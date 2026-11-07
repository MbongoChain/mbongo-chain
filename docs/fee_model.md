<!-- Verified against tokenomics.md -->
# Mbongo Chain — Fee Model

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

> **Document Type:** Fee Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [Gas Model Overview](#2-gas-model-overview)
3. [Fee Flow Model](#3-fee-flow-model)
4. [Deterministic Fee Rules](#4-deterministic-fee-rules)
5. [PoS / PoUW Interaction](#5-pos--pouw-interaction)
6. [Anti-Spam & Security](#6-anti-spam--security)
7. [Long-Term Sustainability](#7-long-term-sustainability)
8. [Future Extensions](#8-future-extensions)

---

## 1. Purpose of This Document

This document defines Mbongo Chain's fee model—a predictable, deterministic system designed to align with the network's compute-first architecture and fixed-supply economics.

### 1.1 Why a Deterministic Fee Model

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FEE MODEL DESIGN GOALS                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   PREVENT SPAM                                                                  │  │
│   │   ────────────                                                                  │  │
│   │                                                                                 │  │
│   │   • Every operation has a cost                                                 │  │
│   │   • Spammers must pay proportionally                                           │  │
│   │   • Economic barrier prevents resource exhaustion                              │  │
│   │   • Block space remains accessible for legitimate users                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   PREDICTABLE VALIDATOR & COMPUTE PROVIDER INCENTIVES                          │  │
│   │   ───────────────────────────────────────────────────                           │  │
│   │                                                                                 │  │
│   │   • Fee distribution follows fixed rules                                       │  │
│   │   • Validators know exactly what they earn                                     │  │
│   │   • Compute providers can calculate ROI                                        │  │
│   │   • No surprises or discretionary changes                                      │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ENABLE DETERMINISTIC EXECUTION                                                │  │
│   │   ──────────────────────────────                                                │  │
│   │                                                                                 │  │
│   │   • All nodes calculate identical fees                                         │  │
│   │   • No consensus failures from fee disagreements                               │  │
│   │   • Transaction validity is unambiguous                                        │  │
│   │   • Replay protection includes fee parameters                                  │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   MAINTAIN ECONOMIC EQUILIBRIUM WITH FIXED SUPPLY                              │  │
│   │   ───────────────────────────────────────────────                               │  │
│   │                                                                                 │  │
│   │   • Total supply: 31,536,000 MBO (immutable)                                   │  │
│   │   • No inflation from fees                                                     │  │
│   │   • Base fees burned → deflationary pressure                                   │  │
│   │   • Sustainable long-term economics                                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Compute-First Alignment

Mbongo Chain's fee model is specifically designed for a compute-first blockchain:

| Aspect | Traditional Chains | Mbongo Chain |
|--------|-------------------|--------------|
| **Primary Resource** | State storage | GPU compute |
| **Fee Focus** | Execution gas only | Execution + Compute + Storage |
| **Reward Split** | 100% to validators | 50% PoS + 50% PoUW |
| **Compute Pricing** | Generic gas | Specialized compute gas |
| **Long-term Model** | Inflation-based | Fee-burning + fixed supply |

---

## 2. Gas Model Overview

### 2.1 Gas Categories

Mbongo Chain uses four distinct gas categories to accurately price different resource types:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GAS CATEGORIES                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   1. BASE GAS (Execution Cost)                                                  │  │
│   │   ════════════════════════════                                                  │  │
│   │                                                                                 │  │
│   │   Purpose: Price CPU computation and state access                              │  │
│   │                                                                                 │  │
│   │   Covers:                                                                       │  │
│   │   • Arithmetic operations                                                      │  │
│   │   • Cryptographic verification (signatures, hashes)                            │  │
│   │   • State reads and writes                                                     │  │
│   │   • Memory allocation                                                          │  │
│   │   • Control flow operations                                                    │  │
│   │                                                                                 │  │
│   │   Base Unit: 1 gas = 1 computational step                                      │  │
│   │   Price: Variable (AIDA-regulated within bounds)                               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   2. COMPUTE GAS (GPU/PoUW Tasks)                                               │  │
│   │   ═══════════════════════════════                                               │  │
│   │                                                                                 │  │
│   │   Purpose: Price off-chain GPU computation                                     │  │
│   │                                                                                 │  │
│   │   Covers:                                                                       │  │
│   │   • AI/ML inference tasks                                                      │  │
│   │   • Training job chunks                                                        │  │
│   │   • Rendering workloads                                                        │  │
│   │   • ZK proof generation                                                        │  │
│   │   • Batch computation                                                          │  │
│   │                                                                                 │  │
│   │   Base Unit: 1 compute_gas = 1 GPU work unit                                   │  │
│   │   Price: Market-determined + AIDA bounds                                       │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   3. STORAGE GAS (Per-Byte, Long-Term)                                          │  │
│   │   ════════════════════════════════════                                          │  │
│   │                                                                                 │  │
│   │   Purpose: Price permanent state storage                                       │  │
│   │                                                                                 │  │
│   │   Covers:                                                                       │  │
│   │   • Account state                                                              │  │
│   │   • Contract storage (future)                                                  │  │
│   │   • Oracle data persistence                                                    │  │
│   │   • Compute receipts                                                           │  │
│   │                                                                                 │  │
│   │   Base Unit: 1 storage_gas = 1 byte stored                                     │  │
│   │   Price: Fixed per byte + storage rent                                         │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   4. NETWORK GAS (Message Propagation)                                          │  │
│   │   ════════════════════════════════════                                          │  │
│   │                                                                                 │  │
│   │   Purpose: Price bandwidth and propagation overhead                            │  │
│   │                                                                                 │  │
│   │   Covers:                                                                       │  │
│   │   • Transaction size (bytes transmitted)                                       │  │
│   │   • Gossip propagation weight                                                  │  │
│   │   • Cross-shard messaging (future)                                             │  │
│   │   • Oracle message relay                                                       │  │
│   │                                                                                 │  │
│   │   Base Unit: 1 network_gas = 1 byte transmitted                                │  │
│   │   Price: Fixed base + congestion adjustment                                    │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Gas Schedule Table

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                              GAS SCHEDULE                                                           │
├─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                                     │
│   CATEGORY         │ OPERATION                      │ GAS COST        │ UNIT           │ NOTES                     │
│   ─────────────────┼────────────────────────────────┼─────────────────┼────────────────┼───────────────────────────│
│                    │                                │                 │                │                           │
│   BASE GAS         │ Simple transfer                │ 21,000          │ gas            │ Minimum transaction       │
│                    │ State read (32 bytes)          │ 200             │ gas            │ Per slot                  │
│                    │ State write (32 bytes)         │ 5,000           │ gas            │ Per slot (new)            │
│                    │ State write (update)           │ 2,900           │ gas            │ Per slot (existing)       │
│                    │ Signature verification         │ 3,000           │ gas            │ Ed25519                   │
│                    │ Hash (keccak256)               │ 30 + 6/word     │ gas            │ Per 32-byte word          │
│                    │ Memory expansion               │ 3               │ gas/byte       │ Linear cost               │
│                    │                                │                 │                │                           │
│   ─────────────────┼────────────────────────────────┼─────────────────┼────────────────┼───────────────────────────│
│                    │                                │                 │                │                           │
│   COMPUTE GAS      │ Compute task registration      │ 50,000          │ compute_gas    │ Task setup                │
│                    │ GPU work unit                  │ 1,000           │ compute_gas    │ Per work unit             │
│                    │ Result verification            │ 10,000          │ compute_gas    │ Per receipt               │
│                    │ AI inference (small)           │ 100,000         │ compute_gas    │ ~1M parameters            │
│                    │ AI inference (medium)          │ 500,000         │ compute_gas    │ ~100M parameters          │
│                    │ AI inference (large)           │ 2,000,000       │ compute_gas    │ ~1B parameters            │
│                    │                                │                 │                │                           │
│   ─────────────────┼────────────────────────────────┼─────────────────┼────────────────┼───────────────────────────│
│                    │                                │                 │                │                           │
│   STORAGE GAS      │ State storage (new)            │ 20,000          │ storage_gas    │ Per 32-byte slot          │
│                    │ State storage (per byte)       │ 625             │ storage_gas    │ Per byte                  │
│                    │ Storage rent (annual)          │ 100             │ storage_gas    │ Per byte/year             │
│                    │ Compute receipt storage        │ 5,000           │ storage_gas    │ Per receipt               │
│                    │                                │                 │                │                           │
│   ─────────────────┼────────────────────────────────┼─────────────────┼────────────────┼───────────────────────────│
│                    │                                │                 │                │                           │
│   NETWORK GAS      │ Transaction base               │ 100             │ network_gas    │ Fixed overhead            │
│                    │ Transaction data               │ 16              │ network_gas    │ Per non-zero byte         │
│                    │ Transaction zero-byte          │ 4               │ network_gas    │ Per zero byte             │
│                    │ Oracle message                 │ 500             │ network_gas    │ Base + per attester       │
│                    │                                │                 │                │                           │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Total Gas Calculation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TOTAL GAS FORMULA                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TRANSACTION GAS                                                                       │
│   ═══════════════                                                                       │
│                                                                                         │
│   total_gas = base_gas + compute_gas + storage_gas + network_gas                       │
│                                                                                         │
│   Where:                                                                                │
│   • base_gas = Σ (operation_cost) for all execution steps                              │
│   • compute_gas = Σ (gpu_work_units × unit_cost) for PoUW tasks                        │
│   • storage_gas = Σ (bytes_stored × storage_cost)                                      │
│   • network_gas = tx_overhead + (data_bytes × per_byte_cost)                           │
│                                                                                         │
│                                                                                         │
│   FEE CALCULATION                                                                       │
│   ═══════════════                                                                       │
│                                                                                         │
│   total_fee = total_gas × gas_price                                                    │
│                                                                                         │
│   gas_price = base_fee + priority_fee                                                  │
│                                                                                         │
│   Where:                                                                                │
│   • base_fee: Protocol-determined (AIDA-regulated)                                     │
│   • priority_fee: User-determined (tip for faster inclusion)                           │
│                                                                                         │
│                                                                                         │
│   EXAMPLE                                                                               │
│   ═══════                                                                               │
│                                                                                         │
│   Simple MBO Transfer:                                                                  │
│   • base_gas: 21,000                                                                   │
│   • compute_gas: 0                                                                     │
│   • storage_gas: 0 (existing accounts)                                                 │
│   • network_gas: 100 + (100 × 16) = 1,700                                              │
│   • total_gas: 22,700                                                                  │
│   • gas_price: 0.000001 MBO                                                            │
│   • total_fee: 0.0000227 MBO                                                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Fee Flow Model

### 3.1 Fee Distribution Overview

Fees in Mbongo Chain follow a deterministic flow with clear destinations for each component:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FEE FLOW MODEL                                                  │
└─────────────────────────────────────────────────────────────────────────────────────────┘


   ┌─────────────────────────────────────────────────────────────────────────────────────┐
   │                                USER TRANSACTION                                     │
   │                                                                                     │
   │   tx_fee = gas_used × (base_fee + priority_fee)                                    │
   │                                                                                     │
   └─────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         │
                                         ▼
   ┌─────────────────────────────────────────────────────────────────────────────────────┐
   │                                EXECUTION LAYER                                      │
   │                                                                                     │
   │   1. Validate transaction                                                           │
   │   2. Execute operations                                                             │
   │   3. Calculate gas_used                                                             │
   │   4. Deduct fee from sender                                                         │
   │                                                                                     │
   └─────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         │
                    ┌────────────────────┴────────────────────┐
                    │                                         │
                    ▼                                         ▼
   ┌─────────────────────────────────┐       ┌─────────────────────────────────┐
   │                                 │       │                                 │
   │        BASE FEE COMPONENT       │       │     PRIORITY FEE COMPONENT      │
   │                                 │       │                                 │
   │   gas_used × base_fee           │       │   gas_used × priority_fee       │
   │                                 │       │                                 │
   └────────────────┬────────────────┘       └────────────────┬────────────────┘
                    │                                         │
                    │                                         │
                    ▼                                         ▼
   ┌─────────────────────────────────┐       ┌─────────────────────────────────┐
   │                                 │       │                                 │
   │           BURNED                │       │    RECIPIENT (by tx type)       │
   │                                 │       │                                 │
   │   • Sent to null address        │       │   • Standard tx → Validator     │
   │   • Permanently destroyed       │       │   • Compute tx → GPU Provider   │
   │   • Reduces circulating supply  │       │   • Oracle tx → Attesters       │
   │   • 100% of base fee            │       │                                 │
   │                                 │       │                                 │
   └─────────────────────────────────┘       └─────────────────────────────────┘
                    │                                         │
                    │                                         │
                    ▼                                         ▼
   ┌─────────────────────────────────┐       ┌─────────────────────────────────┐
   │                                 │       │                                 │
   │    SUPPLY DECREASES             │       │    PARTICIPANT RECEIVES         │
   │                                 │       │                                 │
   │    Deflationary pressure        │       │    Incentive for service        │
   │                                 │       │                                 │
   └─────────────────────────────────┘       └─────────────────────────────────┘
```

### 3.2 Fee Flow Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FEE PROCESSING PIPELINE                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│                                                                                         │
│   ┌───────────┐                                                                        │
│   │  USER TX  │                                                                        │
│   │           │                                                                        │
│   │ • sender  │                                                                        │
│   │ • gas_lim │                                                                        │
│   │ • gas_prc │                                                                        │
│   └─────┬─────┘                                                                        │
│         │                                                                              │
│         ▼                                                                              │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │   VALIDATION    │────▶│  Insufficient   │────▶ REJECT                              │
│   │                 │     │  balance/gas?   │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐                                                                  │
│   │   EXECUTION     │                                                                  │
│   │                 │                                                                  │
│   │ • Run ops       │                                                                  │
│   │ • Track gas     │                                                                  │
│   │ • Apply state   │                                                                  │
│   └────────┬────────┘                                                                  │
│            │                                                                            │
│            ▼                                                                            │
│   ┌─────────────────┐                                                                  │
│   │   FEE CALC      │                                                                  │
│   │                 │                                                                  │
│   │ actual_fee =    │                                                                  │
│   │ gas_used ×      │                                                                  │
│   │ gas_price       │                                                                  │
│   └────────┬────────┘                                                                  │
│            │                                                                            │
│            ▼                                                                            │
│   ┌─────────────────┐                                                                  │
│   │   FEE SPLIT     │                                                                  │
│   │                 │                                                                  │
│   │ base_portion =  │                                                                  │
│   │ gas_used ×      │                                                                  │
│   │ base_fee        │                                                                  │
│   │                 │                                                                  │
│   │ priority_port = │                                                                  │
│   │ gas_used ×      │                                                                  │
│   │ priority_fee    │                                                                  │
│   └────────┬────────┘                                                                  │
│            │                                                                            │
│      ┌─────┴─────┐                                                                     │
│      │           │                                                                     │
│      ▼           ▼                                                                     │
│   ┌──────┐   ┌──────────┐                                                              │
│   │ BURN │   │ REWARD   │                                                              │
│   │      │   │          │                                                              │
│   │ base │   │ priority │                                                              │
│   └──────┘   └──────────┘                                                              │
│                                                                                         │
│                                                                                         │
│   FINAL STATE                                                                           │
│   ───────────                                                                           │
│   • Sender balance: reduced by actual_fee                                              │
│   • Burn address: increased by base_portion (then destroyed)                           │
│   • Recipient: increased by priority_portion                                           │
│   • Unused gas: refunded to sender (gas_limit - gas_used) × gas_price                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Receipt Handling

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DETERMINISTIC RECEIPT HANDLING                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Every transaction produces a receipt with fee details:                               │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   TRANSACTION RECEIPT                                                           │  │
│   │   ═══════════════════                                                           │  │
│   │                                                                                 │  │
│   │   {                                                                             │  │
│   │     "tx_hash": "0x...",                                                         │  │
│   │     "status": "SUCCESS" | "REVERTED",                                           │  │
│   │     "gas_used": 45000,                                                          │  │
│   │     "gas_price": {                                                              │  │
│   │       "base_fee": 1000000,      // in smallest unit                             │  │
│   │       "priority_fee": 100000    // in smallest unit                             │  │
│   │     },                                                                          │  │
│   │     "fee_breakdown": {                                                          │  │
│   │       "total_fee": "0.0000495 MBO",                                             │  │
│   │       "burned": "0.000045 MBO",                                                 │  │
│   │       "to_recipient": "0.0000045 MBO"                                           │  │
│   │     },                                                                          │  │
│   │     "recipient_type": "VALIDATOR" | "GPU_PROVIDER" | "ATTESTER",                │  │
│   │     "recipient_address": "0x..."                                                │  │
│   │   }                                                                             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   RECEIPT GUARANTEES                                                                    │
│   ══════════════════                                                                    │
│   • Deterministic: Same tx → same receipt on all nodes                                 │
│   • Verifiable: Anyone can recalculate from block data                                 │
│   • Immutable: Stored in block, cannot be altered                                      │
│   • Complete: All fee components recorded                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Deterministic Fee Rules

### 4.1 Core Determinism Requirements

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DETERMINISTIC FEE RULES                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   RULE 1: NO FLOATING-POINT ARITHMETIC                                          ║  │
│   ║   ════════════════════════════════════                                          ║  │
│   ║                                                                                 ║  │
│   ║   • All calculations use integer arithmetic                                    ║  │
│   ║   • No IEEE 754 floating-point numbers                                         ║  │
│   ║   • Division rounds down (floor)                                               ║  │
│   ║   • Prevents cross-platform discrepancies                                      ║  │
│   ║                                                                                 ║  │
│   ║   Example:                                                                      ║  │
│   ║   ✗ WRONG:  fee = 45000 × 0.000001                                             ║  │
│   ║   ✓ RIGHT:  fee = 45000 × 1000000 / 1000000000000                              ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   RULE 2: ALL GAS CALCULATIONS ARE INTEGER-BASED                                ║  │
│   ║   ══════════════════════════════════════════════                                ║  │
│   ║                                                                                 ║  │
│   ║   • Gas costs are positive integers                                            ║  │
│   ║   • Gas prices in smallest unit (1 MBO = 10^18 units)                          ║  │
│   ║   • Multiplication before division                                             ║  │
│   ║   • Overflow protection (256-bit integers)                                     ║  │
│   ║                                                                                 ║  │
│   ║   Gas Price Representation:                                                     ║  │
│   ║   1 MBO = 1,000,000,000,000,000,000 smallest_units (10^18)                     ║  │
│   ║   Typical gas price: ~1,000,000 smallest_units = 0.000000000001 MBO            ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   RULE 3: FEES MUST MATCH CANONICAL GAS SCHEDULE                                ║  │
│   ║   ══════════════════════════════════════════════                                ║  │
│   ║                                                                                 ║  │
│   ║   • Gas costs defined in protocol specification                                ║  │
│   ║   • No per-node customization                                                  ║  │
│   ║   • Schedule changes require governance                                        ║  │
│   ║   • All nodes use identical schedule                                           ║  │
│   ║                                                                                 ║  │
│   ║   Verification:                                                                 ║  │
│   ║   For each operation: actual_cost == schedule[operation_code]                  ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   RULE 4: VALIDATORS MUST REJECT UNDERPRICED TRANSACTIONS                       ║  │
│   ║   ═══════════════════════════════════════════════════════                       ║  │
│   ║                                                                                 ║  │
│   ║   • Transaction gas_price MUST be ≥ block base_fee                             ║  │
│   ║   • Underpriced transactions are INVALID                                       ║  │
│   ║   • Cannot be included in blocks                                               ║  │
│   ║   • Consensus rejects blocks with underpriced txs                              ║  │
│   ║                                                                                 ║  │
│   ║   Validation:                                                                   ║  │
│   ║   if (tx.gas_price < block.base_fee) { REJECT("UNDERPRICED") }                 ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   RULE 5: NO DISCRETIONARY OVERRIDES                                            ║  │
│   ║   ══════════════════════════════════                                            ║  │
│   ║                                                                                 ║  │
│   ║   • No manual fee adjustments                                                  ║  │
│   ║   • No validator-specific pricing                                              ║  │
│   ║   • No governance exceptions for specific transactions                         ║  │
│   ║   • No Foundation overrides                                                    ║  │
│   ║                                                                                 ║  │
│   ║   Fee calculation is PURELY algorithmic:                                        ║  │
│   ║   fee = f(gas_used, base_fee, priority_fee)                                    ║  │
│   ║   No external inputs allowed                                                    ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Arithmetic Safety

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ARITHMETIC SAFETY RULES                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   OVERFLOW PROTECTION                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   • Use 256-bit integers for all fee calculations                                      │
│   • Check for overflow before multiplication                                           │
│   • Saturate at MAX_U256 if overflow would occur                                       │
│   • Reject transaction if fee exceeds sender balance                                   │
│                                                                                         │
│   ROUNDING RULES                                                                        │
│   ══════════════                                                                        │
│                                                                                         │
│   • Division always rounds DOWN (toward zero)                                          │
│   • Ensures fees never exceed expected amount                                          │
│   • Deterministic across all implementations                                           │
│                                                                                         │
│   Example:                                                                              │
│   gas_used = 45001                                                                     │
│   gas_price = 1000000 (smallest units)                                                 │
│   fee = 45001 × 1000000 = 45,001,000,000 smallest units                                │
│                                                                                         │
│   If split 90/10:                                                                       │
│   burn = 45001000000 × 90 / 100 = 40,500,900,000 (rounds down)                         │
│   reward = 45001000000 - 40500900000 = 4,500,100,000                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. PoS / PoUW Interaction

### 5.1 Fee Routing by Transaction Type

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PoS / PoUW FEE INTERACTION                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   FEE ROUTING RULES                                                                     │
│   ══════════════════                                                                    │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   PoS TRANSACTIONS (Standard)                                                   │  │
│   │   ═══════════════════════════                                                   │  │
│   │                                                                                 │  │
│   │   • Simple transfers                                                           │  │
│   │   • State updates                                                              │  │
│   │   • Governance operations                                                      │  │
│   │   • Staking/unstaking                                                          │  │
│   │                                                                                 │  │
│   │   Fee Routing:                                                                  │  │
│   │   • Base fee → BURNED                                                          │  │
│   │   • Priority fee → Block Proposer (Validator)                                  │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   PoUW COMPUTE RECEIPTS                                                         │  │
│   │   ═════════════════════                                                         │  │
│   │                                                                                 │  │
│   │   • GPU task submissions                                                       │  │
│   │   • Compute result attestations                                                │  │
│   │   • AI/ML inference receipts                                                   │  │
│   │                                                                                 │  │
│   │   Fee Routing:                                                                  │  │
│   │   • Base fee → BURNED                                                          │  │
│   │   • Priority fee → Compute Provider (GPU node)                                 │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ORACLE MESSAGES                                                               │  │
│   │   ═══════════════                                                               │  │
│   │                                                                                 │  │
│   │   • Price feed updates                                                         │  │
│   │   • External data attestations                                                 │  │
│   │   • Cross-chain proofs                                                         │  │
│   │                                                                                 │  │
│   │   Fee Routing:                                                                  │  │
│   │   • Base fee → BURNED                                                          │  │
│   │   • Priority fee → Attesters (fee-sharing)                                     │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Fee Routing Table

| Transaction Type | Base Fee | Priority Fee | Recipient |
|-----------------|----------|--------------|-----------|
| **Simple Transfer** | BURNED | To Validator | Block Proposer |
| **State Update** | BURNED | To Validator | Block Proposer |
| **Staking Operation** | BURNED | To Validator | Block Proposer |
| **Governance Vote** | BURNED | To Validator | Block Proposer |
| **Compute Task Submit** | BURNED | To Provider | Assigned GPU Node |
| **Compute Receipt** | BURNED | To Provider | Receipt Creator |
| **Oracle Update** | BURNED | To Attesters | Signing Attesters |
| **Delegation Change** | BURNED | To Validator | Block Proposer |

### 5.3 Block Producer Restrictions

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         BLOCK PRODUCER FEE RESTRICTIONS                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   BLOCK PRODUCERS CANNOT CENSOR OR DIVERT FEE FLOWS                             ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   PROHIBITED ACTIONS                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   ✗ Cannot redirect priority fees to themselves                                        │
│     • Compute receipt fees MUST go to provider                                         │
│     • Oracle fees MUST go to attesters                                                 │
│                                                                                         │
│   ✗ Cannot skip burning base fees                                                      │
│     • Base fee burn is consensus-enforced                                              │
│     • Blocks with incorrect burns are INVALID                                          │
│                                                                                         │
│   ✗ Cannot selectively censor transactions                                             │
│     • Must include highest-paying valid transactions                                   │
│     • Censorship detectable via mempool analysis                                       │
│     • Repeated censorship can trigger slashing                                         │
│                                                                                         │
│   ✗ Cannot modify fee calculations                                                     │
│     • All nodes verify fee calculations                                                │
│     • Mismatches cause block rejection                                                 │
│                                                                                         │
│   ENFORCEMENT                                                                           │
│   ═══════════                                                                           │
│                                                                                         │
│   • Consensus validation checks all fee flows                                          │
│   • Invalid fee distribution = invalid block                                           │
│   • Slashing for detected manipulation                                                 │
│   • Economic incentives align with honest behavior                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Anti-Spam & Security

### 6.1 Spam Prevention Mechanisms

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ANTI-SPAM MECHANISMS                                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   1. MINIMUM GAS PRICE                                                          │  │
│   │   ════════════════════                                                          │  │
│   │                                                                                 │  │
│   │   • Protocol-enforced minimum gas price                                        │  │
│   │   • Currently: 1,000,000 smallest_units (0.000000000001 MBO)                   │  │
│   │   • Transactions below minimum are INVALID                                     │  │
│   │   • Cannot be bypassed by validators                                           │  │
│   │                                                                                 │  │
│   │   Validation:                                                                   │  │
│   │   if (tx.gas_price < MIN_GAS_PRICE) { REJECT("BELOW_MINIMUM") }                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   2. EXPONENTIAL COST FOR REPEATED SPAM ACTIONS                                 │  │
│   │   ═════════════════════════════════════════════                                 │  │
│   │                                                                                 │  │
│   │   Certain repetitive actions incur escalating costs:                           │  │
│   │                                                                                 │  │
│   │   • Account creation: 2x cost after 10 creations/block                         │  │
│   │   • Failed transactions: 1.5x gas after 5 failures/address                     │  │
│   │   • Storage spam: 3x cost after threshold exceeded                             │  │
│   │                                                                                 │  │
│   │   Formula:                                                                      │  │
│   │   adjusted_cost = base_cost × (multiplier ^ excess_count)                      │  │
│   │                                                                                 │  │
│   │   Resets: Per epoch (weekly)                                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   3. STORAGE RENT FOR LONG-TERM OCCUPATION                                      │  │
│   │   ════════════════════════════════════════                                      │  │
│   │                                                                                 │  │
│   │   Permanent state storage incurs ongoing rent:                                 │  │
│   │                                                                                 │  │
│   │   • Rate: 100 storage_gas per byte per year                                    │  │
│   │   • Prepaid: Up to 10 years in advance                                         │  │
│   │   • Expiration: Data marked for cleanup if rent unpaid                         │  │
│   │   • Grace period: 30 days before deletion                                      │  │
│   │                                                                                 │  │
│   │   Purpose:                                                                      │  │
│   │   • Prevents state bloat                                                       │  │
│   │   • Encourages efficient storage use                                           │  │
│   │   • Long-term sustainability                                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   4. GUARANTEED BLOCK SPACE FOR SYSTEM MESSAGES                                 │  │
│   │   ═════════════════════════════════════════════                                 │  │
│   │                                                                                 │  │
│   │   1% of block space reserved for critical operations:                          │  │
│   │                                                                                 │  │
│   │   • Slashing evidence                                                          │  │
│   │   • Governance emergency actions                                               │  │
│   │   • Validator set updates                                                      │  │
│   │   • Oracle heartbeats                                                          │  │
│   │                                                                                 │  │
│   │   Reserved space cannot be claimed by regular transactions,                    │  │
│   │   even at high priority fees.                                                  │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Security Measures Summary

| Mechanism | Threat Mitigated | Implementation |
|-----------|------------------|----------------|
| **Minimum Gas Price** | Free transaction spam | Protocol-enforced floor |
| **Exponential Costs** | Repetitive spam attacks | Per-address rate limiting |
| **Storage Rent** | State bloat attacks | Ongoing storage fees |
| **Reserved Block Space** | Critical operation censorship | 1% guaranteed allocation |
| **Gas Limits** | Resource exhaustion | Per-block and per-tx caps |
| **Balance Checks** | Insufficient fund attacks | Pre-execution validation |

### 6.3 Rate Limiting Table

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         RATE LIMITING PARAMETERS                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Action                    │ Threshold  │ Multiplier │ Reset Period │ Max Multiplier  │
│   ──────────────────────────┼────────────┼────────────┼──────────────┼─────────────────│
│   Account creations/block   │ 10         │ 2x         │ Per block    │ 16x             │
│   Failed txs/address        │ 5/hour     │ 1.5x       │ Hourly       │ 8x              │
│   Storage writes/address    │ 100/epoch  │ 1.25x      │ Weekly       │ 4x              │
│   Oracle messages/provider  │ 60/minute  │ 2x         │ Per minute   │ 8x              │
│   Compute tasks/requester   │ 50/epoch   │ 1.5x       │ Weekly       │ 4x              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Long-Term Sustainability

### 7.1 Fixed Supply + Fee Burning Dynamics

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         LONG-TERM ECONOMIC MODEL                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CORE EQUATION                                                                         │
│   ═════════════                                                                         │
│                                                                                         │
│   Fixed Supply MBO + Fee Burning = Deflationary Pressure                               │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   SUPPLY DYNAMICS                                                               │  │
│   │   ───────────────                                                               │  │
│   │                                                                                 │  │
│   │   Total Supply:     31,536,000 MBO (fixed cap)                                 │  │
│   │   Block Emissions:  Decreasing (halving every 5 years)                         │  │
│   │   Fee Burns:        Increasing with network usage                              │  │
│   │                                                                                 │  │
│   │   Net Supply Change (per block):                                                │  │
│   │   ─────────────────────────────                                                 │  │
│   │   Δ supply = block_reward - fees_burned                                        │  │
│   │                                                                                 │  │
│   │   Early Phase (low usage):                                                      │  │
│   │   block_reward > fees_burned → Δ supply > 0 (inflationary)                     │  │
│   │                                                                                 │  │
│   │   Mature Phase (high usage):                                                    │  │
│   │   block_reward < fees_burned → Δ supply < 0 (deflationary)                     │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│                                                                                         │
│   EQUILIBRIUM PROJECTION                                                                │
│   ══════════════════════                                                                │
│                                                                                         │
│   Year 1-5:   Block reward: 0.1 MBO    │  Likely net: Slightly inflationary           │
│   Year 6-10:  Block reward: 0.05 MBO   │  Likely net: Neutral to deflationary         │
│   Year 11-15: Block reward: 0.025 MBO  │  Likely net: Deflationary                    │
│   Year 20+:   Block reward: ~0.01 MBO  │  Likely net: Strongly deflationary           │
│                                                                                         │
│   Equilibrium Point:                                                                   │
│   When fees_burned = block_reward → constant circulating supply                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Why This Prevents Token Collapse

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TOKEN VALUE STABILITY                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PROBLEM: INFLATIONARY COLLAPSE                                                        │
│   ══════════════════════════════                                                        │
│                                                                                         │
│   Many blockchains suffer from:                                                        │
│   • Perpetual inflation to fund security                                               │
│   • Constant selling pressure from validators                                          │
│   • Token value erosion over time                                                      │
│   • Death spiral: low price → less security → lower price                              │
│                                                                                         │
│                                                                                         │
│   SOLUTION: MBONGO'S MODEL                                                              │
│   ════════════════════════                                                              │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   1. FIXED SUPPLY CAP                                                           │  │
│   │      • Maximum 31,536,000 MBO ever                                             │  │
│   │      • No governance can change this                                           │  │
│   │      • Predictable scarcity                                                    │  │
│   │                                                                                 │  │
│   │   2. FEE BURNING                                                                │  │
│   │      • High usage = more burns                                                 │  │
│   │      • Network success → supply decrease                                       │  │
│   │      • Aligns user interests with holders                                      │  │
│   │                                                                                 │  │
│   │   3. HALVING SCHEDULE                                                           │  │
│   │      • Emissions decrease automatically                                        │  │
│   │      • Transition to fee-based security                                        │  │
│   │      • Multi-decade runway                                                     │  │
│   │                                                                                 │  │
│   │   4. DUAL INCENTIVE (PoS + PoUW)                                                │  │
│   │      • 50/50 split diversifies security                                        │  │
│   │      • Compute provides utility value                                          │  │
│   │      • Multiple revenue streams for participants                               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│                                                                                         │
│   POSITIVE FEEDBACK LOOP                                                                │
│   ══════════════════════                                                                │
│                                                                                         │
│   High Usage → More Fees → More Burns → Lower Supply → Higher Scarcity                 │
│        ↑                                                                      │        │
│        └──────────────── Higher Value ← More Demand ←─────────────────────────┘        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.3 Long-Term Security Without Inflation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SECURITY BUDGET SUSTAINABILITY                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SECURITY BUDGET SOURCES                                                               │
│   ═══════════════════════                                                               │
│                                                                                         │
│   1. Block Rewards (decreasing)                                                        │
│      • Primary source in early years                                                   │
│      • Halves every 5 years                                                            │
│      • Approaches zero asymptotically                                                  │
│                                                                                         │
│   2. Transaction Fees (growing)                                                        │
│      • Priority fees to validators/providers                                           │
│      • Scales with network usage                                                       │
│      • Becomes dominant over time                                                      │
│                                                                                         │
│   3. Compute Marketplace Fees                                                          │
│      • GPU providers earn from compute tasks                                           │
│      • AI/ML demand expected to grow exponentially                                     │
│      • Additional revenue beyond block rewards                                         │
│                                                                                         │
│                                                                                         │
│   TRANSITION TIMELINE                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Security                                                                      │  │
│   │   Budget                                                                        │  │
│   │     │                                                                           │  │
│   │     │   ████████░░░░░░░░░░░░░░░░░░░░░░  Block Rewards (early)                  │  │
│   │     │   ░░░░████████████████████████████  Transaction Fees (growing)           │  │
│   │     │   ░░░░░░░░████████████████████████  Compute Fees (emerging)              │  │
│   │     │                                                                           │  │
│   │     └─────┬─────────┬─────────┬─────────┬─────────▶ Years                      │  │
│   │           5        10        15        20                                       │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   KEY INSIGHT                                                                           │
│   ═══════════                                                                           │
│   Security doesn't require perpetual inflation.                                        │
│   Fee revenue from a thriving network sustains validator/provider incentives.          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Future Extensions

### 8.1 Planned Enhancements

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FUTURE EXTENSIONS [PLACEHOLDER]                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   EIP-1559 STYLE DYNAMIC BASE FEE (Optional)                                    │  │
│   │   ══════════════════════════════════════════                                    │  │
│   │                                                                                 │  │
│   │   Current: Fixed base fee (AIDA-regulated within bounds)                       │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • Base fee adjusts based on block utilization                                │  │
│   │   • Target: 50% block fullness                                                 │  │
│   │   • If >50% full: base fee increases                                           │  │
│   │   • If <50% full: base fee decreases                                           │  │
│   │   • Maximum change: ±12.5% per block                                           │  │
│   │                                                                                 │  │
│   │   Benefits:                                                                     │  │
│   │   • Better fee predictability                                                  │  │
│   │   • Automatic congestion pricing                                               │  │
│   │   • Improved UX for users                                                      │  │
│   │                                                                                 │  │
│   │   Timeline: Governance proposal, Year 2                                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   COMPUTE PRIORITY AUCTIONS                                                     │  │
│   │   ═════════════════════════                                                     │  │
│   │                                                                                 │  │
│   │   Current: First-come-first-served compute scheduling                          │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • Auction mechanism for priority compute slots                               │  │
│   │   • Higher bidders get faster execution                                        │  │
│   │   • Dynamic pricing for GPU resources                                          │  │
│   │   • Reserved capacity for premium tasks                                        │  │
│   │                                                                                 │  │
│   │   Benefits:                                                                     │  │
│   │   • Efficient resource allocation                                              │  │
│   │   • Higher revenue for providers                                               │  │
│   │   • Time-sensitive task support                                                │  │
│   │                                                                                 │  │
│   │   Timeline: Research phase, Year 2-3                                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   OFF-CHAIN FEE ESTIMATORS                                                      │  │
│   │   ════════════════════════                                                      │  │
│   │                                                                                 │  │
│   │   Current: Manual gas estimation                                               │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • Official fee estimation API                                                │  │
│   │   • ML-based prediction models                                                 │  │
│   │   • Confidence intervals for estimates                                         │  │
│   │   • Integration with wallets                                                   │  │
│   │                                                                                 │  │
│   │   Benefits:                                                                     │  │
│   │   • Better UX for users                                                        │  │
│   │   • Fewer failed transactions                                                  │  │
│   │   • Optimized fee spending                                                     │  │
│   │                                                                                 │  │
│   │   Timeline: Development phase, Year 1-2                                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Extension Roadmap

| Extension | Status | Target | Dependencies |
|-----------|--------|--------|--------------|
| **Dynamic Base Fee** | Design | Year 2 | Governance approval |
| **Compute Auctions** | Research | Year 2-3 | Marketplace maturity |
| **Fee Estimators** | Development | Year 1-2 | API infrastructure |
| **Cross-Shard Fees** | Exploration | Year 3+ | Sharding implementation |
| **ZK Fee Proofs** | Research | Year 3+ | ZK infrastructure |

---

## Appendix: Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FEE MODEL QUICK REFERENCE                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   GAS CATEGORIES                                                                        │
│   ──────────────                                                                        │
│   Base Gas:      Execution cost (CPU, state access)                                    │
│   Compute Gas:   GPU/PoUW task cost                                                    │
│   Storage Gas:   Per-byte long-term storage                                            │
│   Network Gas:   Message propagation                                                   │
│                                                                                         │
│   FEE STRUCTURE                                                                         │
│   ─────────────                                                                         │
│   Total Fee:     gas_used × (base_fee + priority_fee)                                  │
│   Base Fee:      BURNED (deflationary)                                                 │
│   Priority Fee:  To recipient (validator/provider/attester)                            │
│                                                                                         │
│   DETERMINISTIC RULES                                                                   │
│   ──────────────────                                                                    │
│   • No floating-point arithmetic                                                       │
│   • Integer-based calculations only                                                    │
│   • Canonical gas schedule                                                             │
│   • No discretionary overrides                                                         │
│   • Validators must reject underpriced txs                                             │
│                                                                                         │
│   FEE ROUTING                                                                           │
│   ───────────                                                                           │
│   Standard tx:    Priority → Validator                                                 │
│   Compute receipt: Priority → GPU Provider                                             │
│   Oracle message: Priority → Attesters                                                 │
│   All types:      Base fee → BURNED                                                    │
│                                                                                         │
│   ANTI-SPAM                                                                             │
│   ─────────                                                                             │
│   • Minimum gas price enforced                                                         │
│   • Exponential cost for spam                                                          │
│   • Storage rent for long-term data                                                    │
│   • 1% block space reserved for system                                                 │
│                                                                                         │
│   SUSTAINABILITY                                                                        │
│   ──────────────                                                                        │
│   • Fixed supply: 31,536,000 MBO                                                       │
│   • Fee burning creates deflationary pressure                                          │
│   • Long-term security via fees, not inflation                                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [tokenomics.md](../spec/tokenomics.md) | Canonical economic specification |
| [incentive_design.md](./incentive_design.md) | Incentive mechanisms |
| [supply_schedule.md](./supply_schedule.md) | Emission schedule |
| [staking_model.md](./staking_model.md) | Staking specification |
| [oracle_model.md](./oracle_model.md) | Oracle fee handling |

---

*This document defines the official fee model for Mbongo Chain. All fee calculations are deterministic and enforced by consensus.*

