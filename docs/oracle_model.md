<!-- Verified against tokenomics.md -->
# Mbongo Chain — Oracle Model

> **Document Type:** Oracle Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [Oracle Roles](#2-oracle-roles)
3. [Message Format (Deterministic)](#3-message-format-deterministic)
4. [Validation Rules](#4-validation-rules)
5. [Slashing Rules for Oracle Misbehavior](#5-slashing-rules-for-oracle-misbehavior)
6. [Oracle Economic Model](#6-oracle-economic-model)
7. [Integration With PoUW](#7-integration-with-pouw)
8. [Future Extensions](#8-future-extensions)

---

## 1. Purpose of This Document

### 1.1 Compute-First Architecture

Mbongo Chain is designed as a **compute-first blockchain** with a hybrid PoS + PoUW consensus mechanism. The protocol operates on deterministic, on-chain rules where every state transition can be independently verified and reproduced by any node.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBONGO CHAIN — COMPUTE-FIRST DESIGN                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CORE PRINCIPLES                                                                       │
│   ───────────────                                                                       │
│                                                                                         │
│   1. DETERMINISTIC EXECUTION                                                            │
│      • Every node computes identical results                                           │
│      • State transitions are reproducible                                              │
│      • No randomness or external dependencies in core logic                            │
│                                                                                         │
│   2. ON-CHAIN VERIFICATION                                                              │
│      • All validation rules enforced by consensus                                      │
│      • No trusted third parties                                                        │
│      • Cryptographic proofs for all claims                                             │
│                                                                                         │
│   3. HYBRID PoS + PoUW                                                                  │
│      • 50% security from stake (PoS)                                                   │
│      • 50% utility from compute (PoUW)                                                 │
│      • Balanced incentive structure                                                    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 The Oracle Problem

While Mbongo Chain maintains strict on-chain determinism, many real-world applications require **external data**:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         WHY ORACLES ARE NEEDED                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   EXTERNAL DATA REQUIREMENTS                                                            │
│   ──────────────────────────                                                            │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   PRICE DATA                                                                    │  │
│   │   • MBO/USD exchange rates                                                      │  │
│   │   • Asset prices for DeFi applications                                          │  │
│   │   • Compute pricing for marketplace                                             │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   COMPUTE PROOFS                                                                │  │
│   │   • Compute execution results from off-chain accelerators (GPU/TPU/CPU/FPGA/ASIC)│  │
│   │   • AI/ML inference outputs                                                     │  │
│   │   • Verification hashes for PoUW                                                │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   EXTERNAL EVENTS                                                               │  │
│   │   • Cross-chain state (bridges)                                                 │  │
│   │   • Real-world event outcomes                                                   │  │
│   │   • API responses from external services                                        │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   THE CHALLENGE                                                                         │
│   ─────────────                                                                         │
│   How do we introduce external data WITHOUT breaking:                                  │
│   • Consensus determinism                                                              │
│   • Decentralization                                                                   │
│   • Security guarantees                                                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Document Scope

This document defines:
- **Oracle roles** and their responsibilities
- **Message format** for deterministic oracle data
- **Validation rules** enforced by consensus
- **Slashing conditions** for oracle misbehavior
- **Economic model** (no MBO minting)
- **Integration** with PoUW compute layer

---

## 2. Oracle Roles

### 2.1 Role Overview

The Mbongo oracle system defines three distinct roles with clear separation of responsibilities:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ORACLE DATA FLOW                                                │
└─────────────────────────────────────────────────────────────────────────────────────────┘

    ┌─────────────────────────────────────────────────────────────────────────────────────┐
    │                              EXTERNAL WORLD                                         │
    │                                                                                     │
    │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
    │   │  Exchange A  │  │  Exchange B  │  │   API Feed   │  │  Compute Job │           │
    │   └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘           │
    │          │                 │                 │                 │                   │
    └──────────┼─────────────────┼─────────────────┼─────────────────┼───────────────────┘
               │                 │                 │                 │
               ▼                 ▼                 ▼                 ▼
    ┌─────────────────────────────────────────────────────────────────────────────────────┐
    │                                                                                     │
    │                            DATA PROVIDERS                                           │
    │                         (Off-chain Sources)                                         │
    │                                                                                     │
    │   • Aggregate data from multiple sources                                            │
    │   • Format according to oracle specification                                        │
    │   • Submit to attester network                                                      │
    │                                                                                     │
    └─────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         │  Raw data
                                         ▼
    ┌─────────────────────────────────────────────────────────────────────────────────────┐
    │                                                                                     │
    │                              ATTESTERS                                              │
    │                         (Sign Oracle Updates)                                       │
    │                                                                                     │
    │   ┌───────────────┐  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐       │
    │   │  Attester 1   │  │  Attester 2   │  │  Attester 3   │  │  Attester N   │       │
    │   │               │  │               │  │               │  │               │       │
    │   │  ✓ Verify     │  │  ✓ Verify     │  │  ✓ Verify     │  │  ✓ Verify     │       │
    │   │  ✓ Sign       │  │  ✓ Sign       │  │  ✓ Sign       │  │  ✓ Sign       │       │
    │   └───────┬───────┘  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘       │
    │           │                  │                  │                  │               │
    │           └──────────────────┴─────────┬────────┴──────────────────┘               │
    │                                        │                                           │
    │                              Aggregate signatures                                   │
    │                                                                                     │
    └─────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         │  Signed oracle message
                                         ▼
    ┌─────────────────────────────────────────────────────────────────────────────────────┐
    │                                                                                     │
    │                             MBONGO CHAIN                                            │
    │                                                                                     │
    │   ┌─────────────────────────────────────────────────────────────────────────────┐   │
    │   │                          VALIDATORS                                         │   │
    │   │                   (Verify & Enforce Rules)                                  │   │
    │   │                                                                             │   │
    │   │   • Verify attester signatures                                              │   │
    │   │   • Check timestamp drift                                                   │   │
    │   │   • Validate message format                                                 │   │
    │   │   • Enforce nonce uniqueness                                                │   │
    │   │   • Include in block if valid                                               │   │
    │   │   • Slash for misbehavior                                                   │   │
    │   └─────────────────────────────────────────────────────────────────────────────┘   │
    │                                        │                                           │
    │                                        ▼                                           │
    │   ┌─────────────────────────────────────────────────────────────────────────────┐   │
    │   │                        EXECUTION LAYER                                      │   │
    │   │                                                                             │   │
    │   │   Oracle data available to:                                                 │   │
    │   │   • Smart contracts (future)                                                │   │
    │   │   • PoUW compute verification                                               │   │
    │   │   • Runtime modules                                                         │   │
    │   └─────────────────────────────────────────────────────────────────────────────┘   │
    │                                                                                     │
    └─────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Data Providers

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DATA PROVIDERS                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEFINITION                                                                            │
│   ──────────                                                                            │
│   Data providers are off-chain entities that source, aggregate, and format             │
│   external data for submission to the oracle network.                                  │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Source data from reliable external systems                                         │
│   • Aggregate multiple data points for accuracy                                        │
│   • Format data according to oracle message specification                              │
│   • Submit data to attester network for signing                                        │
│   • Maintain uptime and data freshness                                                 │
│                                                                                         │
│   REQUIREMENTS                                                                          │
│   ────────────                                                                          │
│   • Registered in on-chain provider registry                                           │
│   • Collateral posted (anti-spam, slashing eligibility)                                │
│   • Unique provider ID                                                                 │
│   • Public endpoint for attester queries                                               │
│                                                                                         │
│   EXAMPLES                                                                              │
│   ────────                                                                              │
│   • Price feed aggregators (exchanges, DEXs)                                           │
│   • Compute result providers (GPU/TPU/FPGA/ASIC farms, CPU clusters)                   │
│   • Cross-chain bridges (external chain state)                                         │
│   • API integrators (weather, sports, etc.)                                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Attesters

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ATTESTERS                                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEFINITION                                                                            │
│   ──────────                                                                            │
│   Attesters are registered entities that cryptographically sign oracle updates         │
│   after verifying the data from providers. They provide the trust bridge between       │
│   off-chain data and on-chain consumption.                                             │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Receive data from providers                                                        │
│   • Independently verify data accuracy (cross-reference sources)                       │
│   • Sign valid oracle messages with registered key                                     │
│   • Refuse to sign suspicious or invalid data                                          │
│   • Maintain signing key security                                                      │
│                                                                                         │
│   REQUIREMENTS                                                                          │
│   ────────────                                                                          │
│   • Registered in on-chain attester registry                                           │
│   • Minimum stake: 10,000 MBO (slashing collateral)                                    │
│   • Ed25519 or secp256k1 signing key registered                                        │
│   • Uptime requirements (>95%)                                                         │
│   • Not currently slashed or jailed                                                    │
│                                                                                         │
│   SIGNING PROCESS                                                                       │
│   ───────────────                                                                       │
│   1. Receive data payload from provider                                                │
│   2. Verify data against independent sources                                           │
│   3. Check format compliance                                                           │
│   4. Sign message: signature = sign(attester_key, message_hash)                        │
│   5. Return signed message to aggregator                                               │
│                                                                                         │
│   ECONOMIC INCENTIVE                                                                    │
│   ──────────────────                                                                    │
│   • Micro-fees from oracle message gas                                                 │
│   • Reputation score affects selection probability                                     │
│   • No MBO minting (fees only)                                                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Validators

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VALIDATORS (Oracle Context)                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEFINITION                                                                            │
│   ──────────                                                                            │
│   Validators in Mbongo Chain verify oracle messages and enforce validation rules       │
│   as part of block production. They are the final gatekeepers ensuring only            │
│   valid oracle data enters the blockchain state.                                       │
│                                                                                         │
│   ORACLE-SPECIFIC RESPONSIBILITIES                                                      │
│   ────────────────────────────────                                                      │
│   • Verify attester signatures against registry                                        │
│   • Check timestamp within acceptable drift                                            │
│   • Validate message format and fields                                                 │
│   • Enforce nonce uniqueness (replay protection)                                       │
│   • Include valid oracle transactions in blocks                                        │
│   • Reject invalid or malformed oracle data                                            │
│   • Submit slashing evidence for misbehavior                                           │
│                                                                                         │
│   VALIDATION INTEGRATION                                                                │
│   ──────────────────────                                                                │
│   Oracle message validation is part of the standard transaction validation             │
│   pipeline. Invalid oracle messages cause transaction rejection, not block             │
│   rejection (isolated failure).                                                        │
│                                                                                         │
│   ENFORCEMENT                                                                           │
│   ───────────                                                                           │
│   Validators enforce oracle rules through:                                             │
│   • Transaction-level validation (reject bad messages)                                 │
│   • Evidence submission (trigger slashing)                                             │
│   • Consensus-level verification (all validators agree)                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.5 Role Summary Table

| Role | Location | Stake Required | Primary Function | Slashable |
|------|----------|----------------|------------------|-----------|
| **Data Provider** | Off-chain | Collateral | Source & format data | Yes (collateral) |
| **Attester** | Hybrid | 10,000 MBO | Verify & sign | Yes (stake) |
| **Validator** | On-chain | 50,000 MBO | Enforce rules | Yes (consensus) |

---

## 3. Message Format (Deterministic)

### 3.1 Oracle Message Structure

All oracle messages must follow a strict, deterministic format to ensure consistent validation across all nodes.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ORACLE MESSAGE FORMAT                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   MANDATORY FIELDS                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Field            │ Type        │ Size     │ Description                       │  │
│   │   ─────────────────┼─────────────┼──────────┼───────────────────────────────────│  │
│   │   version          │ uint8       │ 1 byte   │ Message format version            │  │
│   │   data_type        │ uint16      │ 2 bytes  │ Type of oracle data               │  │
│   │   timestamp        │ uint64      │ 8 bytes  │ Unix timestamp (seconds)          │  │
│   │   provider_id      │ bytes32     │ 32 bytes │ Registered provider identifier    │  │
│   │   nonce            │ uint64      │ 8 bytes  │ Unique message nonce              │  │
│   │   payload_length   │ uint32      │ 4 bytes  │ Length of data payload            │  │
│   │   payload          │ bytes       │ variable │ Actual oracle data                │  │
│   │   attester_count   │ uint8       │ 1 byte   │ Number of attester signatures     │  │
│   │   signatures       │ bytes[]     │ variable │ Attester signatures               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   TOTAL HEADER SIZE: 56 bytes (fixed) + variable payload + signatures                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Field Specifications

#### 3.2.1 Data Type

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DATA TYPE CODES                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Code     │ Type                │ Description                                         │
│   ─────────┼─────────────────────┼─────────────────────────────────────────────────────│
│   0x0001   │ PRICE_FEED          │ Asset price data (MBO/USD, etc.)                    │
│   0x0002   │ COMPUTE_RECEIPT     │ PoUW computation result hash                        │
│   0x0003   │ EVENT_HASH          │ External event attestation                          │
│   0x0004   │ CROSS_CHAIN_STATE   │ State proof from another blockchain                 │
│   0x0005   │ API_RESPONSE        │ Generic API data attestation                        │
│   0x0006   │ RANDOM_BEACON       │ Verifiable random value                             │
│   0x0100+  │ CUSTOM              │ Application-specific types                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 3.2.2 Timestamp

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TIMESTAMP SPECIFICATION                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   FORMAT                                                                                │
│   ──────                                                                                │
│   • Unix timestamp in seconds (UTC)                                                    │
│   • 64-bit unsigned integer                                                            │
│   • No millisecond precision (determinism)                                             │
│                                                                                         │
│   VALIDATION                                                                            │
│   ──────────                                                                            │
│   • Must be within ±10 seconds of block timestamp                                      │
│   • Future timestamps rejected                                                         │
│   • Excessively old timestamps rejected                                                │
│                                                                                         │
│   EXAMPLE                                                                               │
│   ───────                                                                               │
│   Block timestamp: 1732924800                                                          │
│   Valid range: 1732924790 to 1732924810                                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 3.2.3 Provider ID

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PROVIDER ID                                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   FORMAT                                                                                │
│   ──────                                                                                │
│   • 32-byte identifier                                                                 │
│   • Derived from: keccak256(provider_pubkey || registration_block)                     │
│   • Unique per registered provider                                                     │
│                                                                                         │
│   REQUIREMENTS                                                                          │
│   ────────────                                                                          │
│   • Must exist in on-chain provider registry                                           │
│   • Must not be in slashed/banned state                                                │
│   • Must have active collateral                                                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 3.2.4 Attester Signature

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ATTESTER SIGNATURE                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SUPPORTED SCHEMES                                                                     │
│   ─────────────────                                                                     │
│   • Ed25519 (64 bytes) — Primary, recommended                                          │
│   • secp256k1 (65 bytes) — ECDSA, Ethereum-compatible                                  │
│                                                                                         │
│   SIGNATURE FORMAT                                                                      │
│   ────────────────                                                                      │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   Field          │ Size      │ Description                                      │  │
│   │   ───────────────┼───────────┼──────────────────────────────────────────────────│  │
│   │   attester_id    │ 32 bytes  │ Registered attester identifier                   │  │
│   │   sig_type       │ 1 byte    │ 0x01 = Ed25519, 0x02 = secp256k1                 │  │
│   │   signature      │ 64-65 b   │ Cryptographic signature                          │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   SIGNED MESSAGE                                                                        │
│   ──────────────                                                                        │
│   message_hash = keccak256(version || data_type || timestamp ||                        │
│                            provider_id || nonce || payload_hash)                       │
│                                                                                         │
│   signature = sign(attester_private_key, message_hash)                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 3.2.5 Nonce

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         NONCE SPECIFICATION                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PURPOSE                                                                               │
│   ───────                                                                               │
│   • Replay protection                                                                  │
│   • Message uniqueness                                                                 │
│   • Ordering within provider                                                           │
│                                                                                         │
│   FORMAT                                                                                │
│   ──────                                                                                │
│   • 64-bit unsigned integer                                                            │
│   • Monotonically increasing per provider                                              │
│   • Tracked on-chain per provider_id                                                   │
│                                                                                         │
│   VALIDATION                                                                            │
│   ──────────                                                                            │
│   • Must be > last_seen_nonce[provider_id]                                             │
│   • Gaps allowed (non-strict increment)                                                │
│   • Duplicate nonces rejected                                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 3.2.6 Version

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VERSION SPECIFICATION                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CURRENT VERSION: 0x01                                                                 │
│                                                                                         │
│   PURPOSE                                                                               │
│   ───────                                                                               │
│   • Forward compatibility                                                              │
│   • Format evolution support                                                           │
│   • Graceful upgrades                                                                  │
│                                                                                         │
│   VALIDATION                                                                            │
│   ──────────                                                                            │
│   • Must be supported version                                                          │
│   • Unknown versions rejected                                                          │
│   • Deprecation notices via governance                                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Example Oracle Message

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         EXAMPLE: PRICE FEED MESSAGE                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   {                                                                                     │
│     "version": 1,                                                                       │
│     "data_type": "PRICE_FEED",        // 0x0001                                        │
│     "timestamp": 1764460800,          // 2025-11-30 00:00:00 UTC                        │
│     "provider_id": "0xabc123...",     // 32 bytes                                      │
│     "nonce": 15847,                                                                     │
│     "payload": {                                                                        │
│       "pair": "MBO/USD",                                                               │
│       "price": "1.2345",              // 4 decimal precision                           │
│       "volume_24h": "5000000"                                                          │
│     },                                                                                  │
│     "signatures": [                                                                     │
│       {                                                                                 │
│         "attester_id": "0xdef456...",                                                  │
│         "sig_type": "Ed25519",                                                         │
│         "signature": "0x789abc..."                                                     │
│       },                                                                                │
│       {                                                                                 │
│         "attester_id": "0xghi789...",                                                  │
│         "sig_type": "Ed25519",                                                         │
│         "signature": "0xjkl012..."                                                     │
│       }                                                                                 │
│     ]                                                                                   │
│   }                                                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.4 Reference to Consensus Validation

Oracle message validation integrates with the broader consensus validation pipeline as defined in:
- `docs/consensus_validation_summary.md` — Full validation pipeline
- `docs/consensus_integrity_checks.md` — Integrity verification rules
- `docs/spec_validation_summary.md` — Transaction validation

Oracle-specific validation is a **sub-module** of transaction validation, executed after signature and format checks but before state application.

---

## 4. Validation Rules

### 4.1 Strict Verification Requirements

All oracle messages undergo strict verification before acceptance. **Any failure results in immediate rejection.**

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VALIDATION RULES                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   RULE 1: SIGNATURE MUST MATCH ATTESTER REGISTRY                               │  │
│   │   ══════════════════════════════════════════════                                │  │
│   │                                                                                 │  │
│   │   • Each signature's attester_id MUST exist in registry                        │  │
│   │   • Signature MUST verify against registered public key                        │  │
│   │   • Attester MUST be in ACTIVE state (not jailed/slashed)                      │  │
│   │   • Minimum attester threshold MUST be met (e.g., 3 of 5)                       │  │
│   │                                                                                 │  │
│   │   Failure: INVALID_SIGNATURE                                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   RULE 2: TIMESTAMP MUST BE WITHIN DRIFT WINDOW                                │  │
│   │   ═════════════════════════════════════════════                                 │  │
│   │                                                                                 │  │
│   │   • Message timestamp MUST be within ±10 seconds of block timestamp            │  │
│   │   • Formula: |message.timestamp - block.timestamp| ≤ 10                        │  │
│   │   • Future timestamps beyond window: REJECTED                                  │  │
│   │   • Past timestamps beyond window: REJECTED                                    │  │
│   │                                                                                 │  │
│   │   Failure: TIMESTAMP_DRIFT_EXCEEDED                                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   RULE 3: MESSAGE MUST BE UNIQUE PER NONCE                                     │  │
│   │   ════════════════════════════════════════                                      │  │
│   │                                                                                 │  │
│   │   • Nonce MUST be greater than last seen for provider_id                       │  │
│   │   • Duplicate nonces are REJECTED (replay protection)                          │  │
│   │   • State: last_nonce[provider_id] updated on acceptance                       │  │
│   │                                                                                 │  │
│   │   Failure: DUPLICATE_NONCE                                                     │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   RULE 4: PROVIDER MUST BE REGISTERED AND NOT SLASHED                          │  │
│   │   ═══════════════════════════════════════════════════                           │  │
│   │                                                                                 │  │
│   │   • provider_id MUST exist in provider registry                                │  │
│   │   • Provider MUST be in ACTIVE state                                           │  │
│   │   • Provider MUST have sufficient collateral                                   │  │
│   │   • Provider MUST NOT be in cooldown/banned state                              │  │
│   │                                                                                 │  │
│   │   Failure: INVALID_PROVIDER                                                    │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   RULE 5: DATA MUST MATCH FORMAT RULES                                         │  │
│   │   ════════════════════════════════════                                          │  │
│   │                                                                                 │  │
│   │   • Payload MUST conform to data_type schema                                   │  │
│   │   • Required fields MUST be present                                            │  │
│   │   • Field types MUST match specification                                       │  │
│   │   • Payload size MUST be within limits                                         │  │
│   │   • Malformed payloads: REJECTED                                               │  │
│   │                                                                                 │  │
│   │   Failure: MALFORMED_PAYLOAD                                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Validation Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ORACLE VALIDATION PIPELINE                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│                                                                                         │
│   ┌─────────────────┐                                                                  │
│   │ ORACLE MESSAGE  │                                                                  │
│   │   (Received)    │                                                                  │
│   └────────┬────────┘                                                                  │
│            │                                                                            │
│            ▼                                                                            │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │  1. VERSION     │────▶│  Unsupported?   │────▶ REJECT: UNSUPPORTED_VERSION         │
│   │     CHECK       │     │                 │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │  2. FORMAT      │────▶│  Malformed?     │────▶ REJECT: MALFORMED_PAYLOAD           │
│   │     CHECK       │     │                 │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │  3. PROVIDER    │────▶│  Not registered │────▶ REJECT: INVALID_PROVIDER            │
│   │     LOOKUP      │     │  or slashed?    │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │  4. NONCE       │────▶│  Duplicate?     │────▶ REJECT: DUPLICATE_NONCE             │
│   │     CHECK       │     │                 │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │  5. TIMESTAMP   │────▶│  Outside ±10s?  │────▶ REJECT: TIMESTAMP_DRIFT_EXCEEDED    │
│   │     CHECK       │     │                 │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │  6. ATTESTER    │────▶│  Not registered │────▶ REJECT: INVALID_ATTESTER            │
│   │     LOOKUP      │     │  or inactive?   │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │  7. SIGNATURE   │────▶│  Invalid sig?   │────▶ REJECT: INVALID_SIGNATURE           │
│   │     VERIFY      │     │                 │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐     ┌─────────────────┐                                          │
│   │  8. THRESHOLD   │────▶│  < min sigs?    │────▶ REJECT: INSUFFICIENT_ATTESTATIONS   │
│   │     CHECK       │     │                 │                                          │
│   └────────┬────────┘     └─────────────────┘                                          │
│            │ OK                                                                         │
│            ▼                                                                            │
│   ┌─────────────────┐                                                                  │
│   │    ACCEPTED     │                                                                  │
│   │                 │                                                                  │
│   │  • Update nonce │                                                                  │
│   │  • Store data   │                                                                  │
│   │  • Emit event   │                                                                  │
│   └─────────────────┘                                                                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Validation Summary Table

| Check | Condition | Error Code | Severity |
|-------|-----------|------------|----------|
| **Version** | Must be supported | UNSUPPORTED_VERSION | Reject |
| **Format** | Must match schema | MALFORMED_PAYLOAD | Reject |
| **Provider** | Must be registered & active | INVALID_PROVIDER | Reject |
| **Nonce** | Must be > last_seen | DUPLICATE_NONCE | Reject |
| **Timestamp** | Must be within ±10s | TIMESTAMP_DRIFT_EXCEEDED | Reject |
| **Attester** | Must be registered & active | INVALID_ATTESTER | Reject |
| **Signature** | Must verify correctly | INVALID_SIGNATURE | Reject |
| **Threshold** | Must meet minimum | INSUFFICIENT_ATTESTATIONS | Reject |

---

## 5. Slashing Rules for Oracle Misbehavior

### 5.1 Slashing Overview

Oracle participants (providers and attesters) are subject to slashing for misbehavior. All slashing is **deterministic** and **on-chain**, triggered by verifiable evidence.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ORACLE SLASHING MODEL                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PRINCIPLES                                                                            │
│   ──────────                                                                            │
│   • Slashing requires cryptographic evidence                                           │
│   • Anyone can submit evidence                                                         │
│   • Automated execution (no human judgment)                                            │
│   • Slashed stake is BURNED (not redistributed)                                        │
│   • Repeated offenses escalate penalties                                               │
│                                                                                         │
│   WHO CAN BE SLASHED                                                                    │
│   ──────────────────                                                                    │
│   • Attesters: For signing invalid/manipulated data                                    │
│   • Providers: For submitting false data (collateral slashed)                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Slashing Conditions

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SLASHING CONDITIONS                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   WRONG DATA / PROVABLE MANIPULATION                                            │  │
│   │   ══════════════════════════════════                                            │  │
│   │                                                                                 │  │
│   │   Penalty: -5% attester stake                                                  │  │
│   │                                                                                 │  │
│   │   Trigger:                                                                      │  │
│   │   • Attester signed data provably different from actual value                  │  │
│   │   • Evidence: signed message + proof of true value                             │  │
│   │   • Example: Price feed showing $1.50 when actual was $1.00                    │  │
│   │                                                                                 │  │
│   │   Evidence Requirements:                                                        │  │
│   │   • Attester's signature on incorrect data                                     │  │
│   │   • Proof of correct value at that timestamp                                   │  │
│   │   • Multiple independent source verification                                   │  │
│   │                                                                                 │  │
│   │   Additional Consequences:                                                      │  │
│   │   • Attester jailed for 7 days                                                 │  │
│   │   • Reputation score reduced                                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   TIMESTAMP MANIPULATION                                                        │  │
│   │   ══════════════════════                                                        │  │
│   │                                                                                 │  │
│   │   Penalty: -2% attester stake                                                  │  │
│   │                                                                                 │  │
│   │   Trigger:                                                                      │  │
│   │   • Attester signed message with deliberately wrong timestamp                  │  │
│   │   • Timestamp designed to exploit time-sensitive applications                  │  │
│   │   • Backdating or future-dating beyond normal drift                            │  │
│   │                                                                                 │  │
│   │   Evidence Requirements:                                                        │  │
│   │   • Signed message with suspicious timestamp                                   │  │
│   │   • Proof that timestamp was intentionally manipulated                         │  │
│   │   • Pattern of timestamp anomalies                                             │  │
│   │                                                                                 │  │
│   │   Additional Consequences:                                                      │  │
│   │   • Attester jailed for 3 days                                                 │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   MALFORMED MESSAGES                                                            │  │
│   │   ══════════════════                                                            │  │
│   │                                                                                 │  │
│   │   Penalty: -0.5% attester stake                                                │  │
│   │                                                                                 │  │
│   │   Trigger:                                                                      │  │
│   │   • Attester signed malformed or invalid message                               │  │
│   │   • Message fails format validation                                            │  │
│   │   • Repeated submission of invalid data                                        │  │
│   │                                                                                 │  │
│   │   Evidence Requirements:                                                        │  │
│   │   • Signed malformed message                                                   │  │
│   │   • Validation failure proof                                                   │  │
│   │                                                                                 │  │
│   │   Additional Consequences:                                                      │  │
│   │   • Warning issued (first offense)                                             │  │
│   │   • Jail on repeated offenses                                                  │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   REPEATED OFFENSES                                                             │  │
│   │   ═════════════════                                                             │  │
│   │                                                                                 │  │
│   │   Penalty: FORCED EJECTION                                                     │  │
│   │                                                                                 │  │
│   │   Trigger:                                                                      │  │
│   │   • 3+ slashing events within 30 days                                          │  │
│   │   • Cumulative slash exceeds 15% of stake                                      │  │
│   │   • Pattern of malicious behavior                                              │  │
│   │                                                                                 │  │
│   │   Consequences:                                                                 │  │
│   │   • Remaining stake fully unbonded (21-day period)                             │  │
│   │   • Permanent ban from attester registry                                       │  │
│   │   • Cannot re-register with same identity                                      │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.3 Slashing Summary Table

| Offense | Penalty | Jail Duration | Escalation |
|---------|---------|---------------|------------|
| **Wrong Data / Manipulation** | -5% stake | 7 days | → Ejection |
| **Timestamp Manipulation** | -2% stake | 3 days | → Ejection |
| **Malformed Messages** | -0.5% stake | Warning / 1 day | → Higher penalties |
| **Repeated Offenses (3+)** | Ejection | Permanent ban | N/A |

### 5.4 Slashing Properties

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SLASHING PROPERTIES                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ✓ ALL SLASHING IS DETERMINISTIC                                                      │
│   ────────────────────────────────                                                      │
│   • Evidence-based (cryptographic proof)                                               │
│   • Same evidence → same outcome on all nodes                                          │
│   • No subjective judgment                                                             │
│                                                                                         │
│   ✓ ALL SLASHING IS ON-CHAIN                                                           │
│   ──────────────────────────────                                                        │
│   • Slashing transactions recorded in blocks                                           │
│   • Full audit trail                                                                   │
│   • Transparent enforcement                                                            │
│                                                                                         │
│   ✓ SLASHED MBO IS BURNED                                                              │
│   ────────────────────────                                                              │
│   • Not redistributed to reporters                                                     │
│   • Not sent to treasury                                                               │
│   • Removed from circulation                                                           │
│   • Deflationary effect                                                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Oracle Economic Model

### 6.1 No MBO Minting

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ORACLE ECONOMIC MODEL                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   ORACLE UPDATES DO NOT MINT MBO                                                ║  │
│   ║   ══════════════════════════════                                                ║  │
│   ║                                                                                 ║  │
│   ║   • Oracle operations are fee-based only                                       ║  │
│   ║   • No new MBO created for oracle activities                                   ║  │
│   ║   • Preserves fixed supply guarantee (31,536,000 MBO)                          ║  │
│   ║   • No inflation from oracle usage                                             ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│                                                                                         │
│   FEE STRUCTURE                                                                         │
│   ═════════════                                                                         │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ORACLE MESSAGE FEES                                                           │  │
│   │   ───────────────────                                                           │  │
│   │   • Paid by: Contract callers / data consumers                                 │  │
│   │   • Covers: Message validation, storage, execution gas                         │  │
│   │   • Destination: Base fee burned, priority to proposer                         │  │
│   │                                                                                 │  │
│   │   Fee Calculation:                                                              │  │
│   │   oracle_fee = base_gas_cost + (payload_size × per_byte_cost)                  │  │
│   │              + signature_verification_cost × attester_count                    │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ATTESTER COMPENSATION                                                         │  │
│   │   ────────────────────                                                          │  │
│   │   • Source: Micro-fees from message gas                                        │  │
│   │   • Model: Fee-sharing from priority fees                                      │  │
│   │   • No block rewards for oracle work                                           │  │
│   │   • Reputation affects fee allocation                                          │  │
│   │                                                                                 │  │
│   │   Attester Revenue:                                                             │  │
│   │   attester_fee = priority_fee × attester_share × reputation_multiplier         │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   PROVIDER COMPENSATION                                                         │  │
│   │   ─────────────────────                                                         │  │
│   │   • Source: Service fees from data consumers                                   │  │
│   │   • Model: Off-chain agreements + on-chain verification                        │  │
│   │   • Optional: Subscription models for frequent data                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Economic Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ORACLE ECONOMIC FLOW                                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│                                                                                         │
│   ┌─────────────────┐                                                                  │
│   │  DATA CONSUMER  │                                                                  │
│   │  (Application)  │                                                                  │
│   └────────┬────────┘                                                                  │
│            │                                                                            │
│            │  Pays oracle fee (MBO)                                                    │
│            │                                                                            │
│            ▼                                                                            │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │                            ORACLE FEE POOL                                      │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│            │                                                                            │
│            ├───────────────────────┬───────────────────────┬────────────────────────┐  │
│            │                       │                       │                        │  │
│            ▼                       ▼                       ▼                        │  │
│   ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐              │  │
│   │   BASE FEE      │     │  PRIORITY FEE   │     │  ATTESTER FEE   │              │  │
│   │                 │     │                 │     │                 │              │  │
│   │   BURNED        │     │  Block Proposer │     │  Attesters      │              │  │
│   │                 │     │                 │     │  (fee-sharing)  │              │  │
│   │   ~60%          │     │   ~30%          │     │   ~10%          │              │  │
│   └─────────────────┘     └─────────────────┘     └─────────────────┘              │  │
│            │                                                                        │  │
│            ▼                                                                        │  │
│   ┌─────────────────┐                                                              │  │
│   │  SUPPLY ↓       │     NO NEW MBO CREATED                                       │  │
│   │  (Deflationary) │     ═══════════════════                                      │  │
│   └─────────────────┘                                                              │  │
│                                                                                         │
│                                                                                         │
│   KEY POINTS                                                                            │
│   ──────────                                                                            │
│   • No inflation from oracles                                                          │
│   • No reward pool for oracle work                                                     │
│   • Fee-only compensation model                                                        │
│   • Base fees burned (deflationary)                                                    │
│   • Sustainable without MBO minting                                                    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Economic Summary

| Aspect | Implementation | MBO Impact |
|--------|----------------|------------|
| **Oracle Fees** | Paid by consumers | Transfer (no mint) |
| **Base Fee** | Burned | Supply decrease |
| **Priority Fee** | To block proposer | Transfer (no mint) |
| **Attester Compensation** | Fee-sharing | Transfer (no mint) |
| **Provider Compensation** | Off-chain + fees | Transfer (no mint) |
| **Slashing** | Stake burned | Supply decrease |
| **Total MBO Created** | 0 | No inflation |

---

## 7. Integration With PoUW

### 7.1 Compute Receipt Flow

The oracle layer plays a critical role in Mbongo's PoUW (Proof-of-Useful-Work) system by providing a trusted channel for compute results.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ORACLE ↔ PoUW INTEGRATION                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   COMPUTE RECEIPT FLOW                                                                  │
│   ════════════════════                                                                  │
│                                                                                         │
│   ┌─────────────────┐                                                                  │
│   │  TASK REQUESTER │                                                                  │
│   │  (User/App)     │                                                                  │
│   └────────┬────────┘                                                                  │
│            │                                                                            │
│            │  1. Submit compute task                                                   │
│            ▼                                                                            │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           MBONGO CHAIN                                          │  │
│   │                                                                                 │  │
│   │   • Task registered on-chain                                                    │  │
│   │   • Task ID assigned                                                            │  │
│   │   • Compute provider selected (GPU/TPU/CPU/FPGA/ASIC)                           │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│            │                                                                            │
│            │  2. Task assignment                                                       │
│            ▼                                                                            │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                         COMPUTE PROVIDER                                        │  │
│   │                                                                                 │  │
│   │   • Execute compute task (off-chain)                                            │  │
│   │   • Generate result hash                                                        │  │
│   │   • Create compute proof                                                        │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│            │                                                                            │
│            │  3. Submit result as oracle message                                       │
│            ▼                                                                            │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           ORACLE LAYER                                          │  │
│   │                                                                                 │  │
│   │   ┌───────────────────────────────────────────────────────────────────────────┐ │  │
│   │   │                      COMPUTE RECEIPT MESSAGE                              │ │  │
│   │   │                                                                           │ │  │
│   │   │   data_type:    COMPUTE_RECEIPT (0x0002)                                  │ │  │
│   │   │   provider_id:  Compute provider identifier                               │ │  │
│   │   │   payload:      {                                                         │ │  │
│   │   │                   task_id: "...",                                         │ │  │
│   │   │                   result_hash: "0x...",                                   │ │  │
│   │   │                   compute_units: 1000,                                    │ │  │
│   │   │                   execution_time_ms: 5000                                 │ │  │
│   │   │                 }                                                         │ │  │
│   │   │   signatures:   [attester_1, attester_2, ...]                             │ │  │
│   │   └───────────────────────────────────────────────────────────────────────────┘ │  │
│   │                                                                                 │  │
│   │   • Attesters verify compute proof                                              │  │
│   │   • Multiple attesters sign receipt                                             │  │
│   │   • Submit to chain                                                             │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│            │                                                                            │
│            │  4. Validated receipt on-chain                                            │
│            ▼                                                                            │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           EXECUTION LAYER                                       │  │
│   │                                                                                 │  │
│   │   • Compute receipt validated                                                   │  │
│   │   • PoUW score calculated                                                       │  │
│   │   • Rewards distributed (50% of block reward to PoUW)                           │  │
│   │   • Result available to applications                                            │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Verification Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE RECEIPT VERIFICATION                                    │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   VERIFICATION METHODS                                                                  │
│   ════════════════════                                                                  │
│                                                                                         │
│   1. REPLICATED COMPUTE                                                                 │
│      • Multiple compute providers execute same task                                    │
│      • Results must match (deterministic tasks)                                        │
│      • Majority consensus on result hash                                               │
│                                                                                         │
│   2. PROBABILISTIC SAMPLING                                                             │
│      • Random subset of results re-verified                                            │
│      • Statistical detection of fraud                                                  │
│      • Cost-efficient for large workloads                                              │
│                                                                                         │
│   3. CRYPTOGRAPHIC PROOFS (Future)                                                     │
│      • ZK proofs for computation                                                       │
│      • Succinct verification on-chain                                                  │
│      • No re-execution required                                                        │
│                                                                                         │
│                                                                                         │
│   ORACLE ROLE IN VERIFICATION                                                           │
│   ═══════════════════════════                                                           │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   Attesters verify compute receipts by:                                         │  │
│   │                                                                                 │  │
│   │   • Checking task_id exists and is assigned to provider                        │  │
│   │   • Verifying result_hash format                                               │  │
│   │   • Validating compute_units against task specification                        │  │
│   │   • Cross-referencing with other providers (if replicated)                     │  │
│   │   • Checking execution_time within reasonable bounds                           │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.3 PoUW Reward Integration

| Component | Source | Verification | Reward |
|-----------|--------|--------------|--------|
| **Compute Task** | User submission | On-chain validation | N/A |
| **Compute Receipt** | Compute provider via oracle | Attester signatures | 50% of block reward |
| **Receipt Validation** | Consensus layer | All validators | N/A |
| **Reward Distribution** | Block finalization | Deterministic | To verified providers |

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
│   │   ZK ORACLE PROOFS                                                              │  │
│   │   ════════════════                                                              │  │
│   │                                                                                 │  │
│   │   Current: Attesters sign data after manual verification                       │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • ZK proofs for data authenticity                                            │  │
│   │   • Prove data from source without revealing source                            │  │
│   │   • Succinct on-chain verification                                             │  │
│   │   • Reduced trust assumptions                                                  │  │
│   │                                                                                 │  │
│   │   Benefits:                                                                     │  │
│   │   • Privacy-preserving oracles                                                 │  │
│   │   • Cheaper verification                                                       │  │
│   │   • Stronger guarantees                                                        │  │
│   │                                                                                 │  │
│   │   Timeline: Research phase, Year 2-3                                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   CROSS-CHAIN ORACLE BRIDGES                                                    │  │
│   │   ══════════════════════════                                                    │  │
│   │                                                                                 │  │
│   │   Current: Single-chain oracle data                                            │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • Bridge state from other blockchains                                        │  │
│   │   • Ethereum, Bitcoin, Solana state proofs                                     │  │
│   │   • Cross-chain asset verification                                             │  │
│   │   • Multi-chain compute coordination                                           │  │
│   │                                                                                 │  │
│   │   Benefits:                                                                     │  │
│   │   • Interoperability                                                           │  │
│   │   • Cross-chain DeFi                                                           │  │
│   │   • Unified compute marketplace                                                │  │
│   │                                                                                 │  │
│   │   Timeline: Development phase, Year 2-3                                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   DECENTRALIZED ATTESTER MARKETPLACE                                            │  │
│   │   ══════════════════════════════════                                            │  │
│   │                                                                                 │  │
│   │   Current: Fixed attester set with manual registration                         │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • Open attester marketplace                                                  │  │
│   │   • Dynamic attester selection                                                 │  │
│   │   • Reputation-based pricing                                                   │  │
│   │   • Specialized attesters for data types                                       │  │
│   │                                                                                 │  │
│   │   Benefits:                                                                     │  │
│   │   • More decentralization                                                      │  │
│   │   • Competitive pricing                                                        │  │
│   │   • Better coverage                                                            │  │
│   │   • Permissionless participation                                               │  │
│   │                                                                                 │  │
│   │   Timeline: Design phase, Year 3-4                                             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Extension Roadmap

| Extension | Status | Target | Dependencies |
|-----------|--------|--------|--------------|
| **ZK Oracle Proofs** | Research | Year 2-3 | ZK infrastructure |
| **Cross-Chain Bridges** | Development | Year 2-3 | Bridge protocol |
| **Attester Marketplace** | Design | Year 3-4 | Governance approval |
| **Real-time Streaming** | Exploration | Year 3+ | Network upgrades |

---

## Appendix: Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ORACLE MODEL QUICK REFERENCE                                    │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLES                                                                                 │
│   ─────                                                                                 │
│   Data Providers:  Off-chain data sources (collateral required)                        │
│   Attesters:       Sign oracle updates (10,000 MBO stake)                              │
│   Validators:      Verify and enforce rules (consensus)                                │
│                                                                                         │
│   MESSAGE FORMAT                                                                        │
│   ──────────────                                                                        │
│   Version:         uint8 (current: 0x01)                                               │
│   Data Type:       uint16 (PRICE, COMPUTE_RECEIPT, EVENT, etc.)                        │
│   Timestamp:       uint64 (Unix seconds, ±10s drift allowed)                           │
│   Provider ID:     bytes32 (registered identifier)                                     │
│   Nonce:           uint64 (unique per provider)                                        │
│   Signatures:      Ed25519 or secp256k1                                                │
│                                                                                         │
│   VALIDATION RULES                                                                      │
│   ────────────────                                                                      │
│   • Signature MUST match attester registry                                             │
│   • Timestamp MUST be within ±10 seconds                                               │
│   • Nonce MUST be unique (replay protection)                                           │
│   • Provider MUST be registered and active                                             │
│   • Payload MUST match format specification                                            │
│                                                                                         │
│   SLASHING                                                                              │
│   ────────                                                                              │
│   Wrong Data:      -5% stake, 7-day jail                                               │
│   Timestamp Manipulation: -2% stake, 3-day jail                                        │
│   Malformed Messages: -0.5% stake, warning                                             │
│   Repeated Offenses: Forced ejection                                                   │
│                                                                                         │
│   ECONOMICS                                                                             │
│   ─────────                                                                             │
│   • NO MBO MINTING from oracles                                                        │
│   • Fees paid by data consumers                                                        │
│   • Base fees burned                                                                   │
│   • Attesters earn micro-fees                                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [tokenomics.md](../spec/tokenomics.md) | Canonical economic specification |
| [consensus_validation_summary.md](./consensus_validation_summary.md) | Validation pipeline |
| [compute_engine_overview.md](./compute_engine_overview.md) | PoUW compute system |
| [incentive_design.md](./incentive_design.md) | Incentive mechanisms |
| [staking_model.md](./staking_model.md) | Staking specification |

---

*This document defines the official oracle model for Mbongo Chain. Oracle operations do not mint MBO and are governed by deterministic on-chain rules.*

