# PoX Consensus Formula - Mathematical Specification

## Overview

This document provides the complete mathematical specification of Mbongo Chain's **Proof of X (PoX)** consensus mechanism. PoX combines **Proof of Stake (PoS)** and **Proof of Useful Work (PoUW)** using dynamic coefficients managed by the AIDA (Adaptive Intelligence for Dynamic Adjustment) system.

**Note**: The native token is **MBO** (Mbongo).

## Table of Contents

1. [Core Formula](#core-formula)
2. [Stake Weight Calculation](#stake-weight-calculation)
3. [Proof of Compute Score](#proof-of-compute-score)
4. [AIDA Dynamic Coefficients](#aida-dynamic-coefficients)
5. [Anti-Centralization Mechanism](#anti-centralization-mechanism)
6. [Practical Examples](#practical-examples)
7. [Parameter Reference](#parameter-reference)

---

## Core Formula

### Total Weight Function

The total weight of a validator, used for block production selection and rewards distribution, is calculated as:

```
total_weight = (stake_weight × C_SR) + (√(poc_score) × C_NL)
```

**Where:**
- `stake_weight` = Validator's stake-based weight (in MBO)
- `C_SR` = AIDA coefficient for Stake Rewards (0.8 - 1.2)
- `poc_score` = Proof of Compute score (dimensionless)
- `C_NL` = AIDA coefficient for Network Load (0.8 - 1.2)
- `√()` = Square root function (anti-centralization mechanism)

### Component Breakdown

```
┌─────────────────────────────────────────────────────────────┐
│                   Total Weight Components                    │
└─────────────────────────────────────────────────────────────┘

                    total_weight
                         │
        ┌────────────────┴────────────────┐
        │                                 │
        ▼                                 ▼
  Stake Component                   Work Component
        │                                 │
  stake_weight × C_SR            √(poc_score) × C_NL
        │                                 │
        ├─ Economic security              ├─ Computational contribution
        ├─ Sybil resistance               ├─ Network utility
        └─ Long-term alignment            └─ Anti-whale mechanism
```

---

## Stake Weight Calculation

### Base Formula

```
stake_weight = staked_amount × time_multiplier × reputation_factor
```

### Time Multiplier (Lock Period Bonus)

To incentivize long-term commitment, validators receive a bonus based on their stake lock period:

```
time_multiplier = 1 + (lock_period_days / MAX_LOCK_DAYS) × LOCK_BONUS_MAX
```

**Where:**
- `lock_period_days` = Number of days tokens are locked
- `MAX_LOCK_DAYS` = 730 days (2 years)
- `LOCK_BONUS_MAX` = 0.5 (50% maximum bonus)

#### Lock Period Table

| Lock Period | Time Multiplier | Effective Stake (per 1000 MBO) |
|-------------|----------------|-------------------------------|
| 0 days (unlocked) | 1.00 | 1,000 MBO |
| 30 days | 1.02 | 1,020 MBO |
| 90 days | 1.06 | 1,060 MBO |
| 180 days (6 months) | 1.12 | 1,120 MBO |
| 365 days (1 year) | 1.25 | 1,250 MBO |
| 730 days (2 years) | 1.50 | 1,500 MBO |

### Reputation Factor

```
reputation_factor = 0.5 + (reputation_score / 2)
```

**Where:**
- `reputation_score` ∈ [0, 1]
- `reputation_factor` ∈ [0.5, 1.0]

**Rationale**: Even validators with poor reputation (0.0) maintain 50% of their stake weight to prevent complete exclusion and allow recovery.

#### Reputation Impact Table

| Reputation Score | Reputation Factor | Effective Stake Multiplier |
|-----------------|-------------------|---------------------------|
| 0.0 (minimum) | 0.50 | 50% |
| 0.2 | 0.60 | 60% |
| 0.5 | 0.75 | 75% |
| 0.8 | 0.90 | 90% |
| 1.0 (maximum) | 1.00 | 100% |

### Complete Stake Weight Formula

```
stake_weight = staked_amount × (1 + (lock_period_days / 730) × 0.5) × (0.5 + reputation_score / 2)
```

### Example Calculation

**Scenario**: Validator with 10,000 MBO staked for 1 year with 0.8 reputation

```
staked_amount = 10,000 MBO
lock_period_days = 365
reputation_score = 0.8

time_multiplier = 1 + (365 / 730) × 0.5
                = 1 + 0.5 × 0.5
                = 1.25

reputation_factor = 0.5 + (0.8 / 2)
                  = 0.5 + 0.4
                  = 0.9

stake_weight = 10,000 × 1.25 × 0.9
             = 11,250 MBO
```

---

## Proof of Compute Score

### Complete Formula

The Proof of Compute (PoC) score measures a validator's contribution through useful work:

```
poc_score = Σ (compute_units × job_value × reliability × validity × decay)
```

**Where the sum is over all completed jobs in the evaluation window.**

### Component Definitions

#### 1. Compute Units

```
compute_units = (CPU_hours × CPU_weight) + (GPU_hours × GPU_weight) + (RAM_GB_hours × RAM_weight)
```

**Standard Weights:**
```
CPU_weight = 1.0
GPU_weight = 10.0  # GPUs are 10× more valuable
RAM_weight = 0.1   # RAM is supplementary
```

**Example:**
```
Job: 2 CPU hours, 0.5 GPU hours, 16 GB-hours RAM

compute_units = (2 × 1.0) + (0.5 × 10.0) + (16 × 0.1)
              = 2 + 5 + 1.6
              = 8.6 units
```

#### 2. Job Value

Job value is determined by the task's complexity and market demand:

```
job_value = base_value × priority_multiplier × market_multiplier
```

**Base Value Categories:**
```
Inference (simple):     1.0
Inference (complex):    2.0
Training (small):       5.0
Training (medium):     10.0
Training (large):      20.0
Custom workloads:     variable
```

**Priority Multiplier:**
```
Low priority:    0.8
Normal priority: 1.0
High priority:   1.5
Urgent:          2.0
```

**Market Multiplier:**
```
market_multiplier = current_demand / average_demand

Clamped to: [0.5, 2.0]
```

#### 3. Reliability Score

Reliability measures the validator's historical performance:

```
reliability = (successful_jobs / total_jobs) × uptime_factor

Where:
uptime_factor = min(1.0, actual_uptime / expected_uptime)
```

**Example:**
```
successful_jobs = 95
total_jobs = 100
actual_uptime = 0.98 (98%)
expected_uptime = 0.95 (95%)

reliability = (95 / 100) × min(1.0, 0.98 / 0.95)
            = 0.95 × 1.0
            = 0.95
```

#### 4. Validity Score

Validity represents verification confidence:

```
validity = verification_confidence × (1 - fraud_penalty)

Where:
verification_confidence ∈ [0, 1]  # From verification layer
fraud_penalty ∈ [0, 1]            # Penalties for disputed results
```

**Verification Confidence:**
```
Phase 1 (Redundant):   1.0 if consensus, 0.0 otherwise
Phase 2 (TEE):         0.95 (TEE) or 1.0 (consensus)
Phase 3 (ZK):          1.0 (cryptographic proof)
```

**Fraud Penalty:**
```
No disputes:           0.0
Challenged (resolved): 0.1 per challenge
Fraud proven:          1.0 (complete invalidation)
```

#### 5. Time Decay

To prevent infinite accumulation and prioritize recent contributions:

```
decay = e^(-λ × days_since_completion)

Where:
λ = ln(2) / half_life
half_life = 30 days (default)
```

**Decay Table:**

| Days Since Completion | Decay Factor | PoC Score % |
|----------------------|--------------|-------------|
| 0 days | 1.000 | 100% |
| 15 days | 0.841 | 84.1% |
| 30 days | 0.707 | 70.7% |
| 60 days | 0.500 | 50.0% |
| 90 days | 0.354 | 35.4% |
| 180 days | 0.125 | 12.5% |

### Complete PoC Score Example

**Scenario**: Validator completed a GPU training job 15 days ago

```
# Job parameters
CPU_hours = 4
GPU_hours = 8
RAM_GB_hours = 128

# Compute units
compute_units = (4 × 1.0) + (8 × 10.0) + (128 × 0.1)
              = 4 + 80 + 12.8
              = 96.8 units

# Job value
base_value = 10.0          # Medium training
priority_multiplier = 1.5   # High priority
market_multiplier = 1.2     # 20% above average demand

job_value = 10.0 × 1.5 × 1.2
          = 18.0

# Reliability
successful_jobs = 190
total_jobs = 200
uptime_factor = 1.0

reliability = 190 / 200 × 1.0
            = 0.95

# Validity
verification_confidence = 1.0
fraud_penalty = 0.0

validity = 1.0 × (1 - 0.0)
         = 1.0

# Decay (15 days ago)
λ = ln(2) / 30 = 0.0231
decay = e^(-0.0231 × 15)
      = e^(-0.347)
      = 0.707

# Final PoC score for this job
job_poc_score = 96.8 × 18.0 × 0.95 × 1.0 × 0.707
              = 1,173.6

# Total PoC score (sum of all jobs)
poc_score = 1,173.6 + [other jobs...]
```

---

## AIDA Dynamic Coefficients

### Overview

AIDA (Adaptive Intelligence for Dynamic Adjustment) dynamically adjusts the coefficients `C_SR` (Stake Rewards) and `C_NL` (Network Load) to maintain network balance.

### Coefficient Bounds

```
C_SR ∈ [0.8, 1.2]  # Stake Rewards coefficient
C_NL ∈ [0.8, 1.2]  # Network Load coefficient
```

**Constraint:**
```
C_SR + C_NL = 2.0  (constant sum)
```

This ensures the total weight scale remains stable while allowing redistribution between stake and work.

### Adjustment Formula

```
C_SR(t+1) = C_SR(t) + α × (target_stake_ratio - current_stake_ratio)

C_NL(t+1) = 2.0 - C_SR(t+1)

Where:
α = learning_rate = 0.01
target_stake_ratio = 0.5 (50% stake, 50% work)
current_stake_ratio = total_stake_weight / (total_stake_weight + total_work_weight)
```

### Adjustment Examples

#### Scenario 1: Too Much Stake Dominance

```
Current state:
total_stake_weight = 1,000,000 MBO
total_work_weight = 400,000 (effective)
current_stake_ratio = 1,000,000 / 1,400,000 = 0.714

Current coefficients:
C_SR = 1.1
C_NL = 0.9

Adjustment:
C_SR_new = 1.1 + 0.01 × (0.5 - 0.714)
         = 1.1 + 0.01 × (-0.214)
         = 1.1 - 0.00214
         = 1.098

C_NL_new = 2.0 - 1.098
         = 0.902

Result: Stake coefficient decreased, work coefficient increased
```

#### Scenario 2: Too Much Work Dominance

```
Current state:
total_stake_weight = 600,000 MBO
total_work_weight = 1,800,000 (effective)
current_stake_ratio = 600,000 / 2,400,000 = 0.25

Current coefficients:
C_SR = 0.9
C_NL = 1.1

Adjustment:
C_SR_new = 0.9 + 0.01 × (0.5 - 0.25)
         = 0.9 + 0.01 × 0.25
         = 0.9 + 0.0025
         = 0.9025

C_NL_new = 2.0 - 0.9025
         = 1.0975

Result: Stake coefficient increased, work coefficient decreased
```

### Coefficient Impact Visualization

```
┌─────────────────────────────────────────────────────────────┐
│          AIDA Coefficient Adjustment Range                   │
└─────────────────────────────────────────────────────────────┘

C_SR: Stake Rewards Coefficient
├────────┼────────┼────────┼────────┼────────┤
0.8     0.9     1.0     1.1     1.2
         ↑               ↑
    Work-favored    Stake-favored


C_NL: Network Load Coefficient
├────────┼────────┼────────┼────────┼────────┤
1.2     1.1     1.0     0.9     0.8
 ↑               ↑
Work-favored    Stake-favored


Balanced state: C_SR = 1.0, C_NL = 1.0
```

---

## Anti-Centralization Mechanism

### Square Root Function Rationale

The PoX formula applies a **square root** to the PoC score:

```
work_component = √(poc_score) × C_NL
```

**Purpose**: Implement **diminishing returns** to prevent large compute providers from dominating the network.

### Mathematical Analysis

#### Linear vs Square Root Comparison

**Without √ (Linear):**
```
Validator A: 100 PoC units   → weight contribution = 100
Validator B: 10,000 PoC units → weight contribution = 10,000

Ratio: 10,000 / 100 = 100:1
```

**With √ (Sublinear):**
```
Validator A: 100 PoC units   → weight contribution = √100 = 10
Validator B: 10,000 PoC units → weight contribution = √10,000 = 100

Ratio: 100 / 10 = 10:1
```

**Result**: The square root reduces the advantage of large validators by 10× in this example.

### Diminishing Returns Curve

```
┌─────────────────────────────────────────────────────────────┐
│         Work Component: Linear vs Square Root               │
└─────────────────────────────────────────────────────────────┘

Weight
  │
  │                                            Linear (y = x)
  │                                         ╱
500 │                                      ╱
  │                                     ╱
  │                                  ╱
  │                               ╱
  │                            ╱
  │                         ╱
  │                      ╱
  │                   ╱            √(x) - Actual
  │                ╱            ╱‾‾‾
  │             ╱            ╱
  │          ╱           ╱
  │       ╱          ╱
  │    ╱        ╱‾‾
  │ ╱     ╱‾‾‾
  ╱╱‾‾‾‾
  └──────────────────────────────────────────────────────────
  0              100,000             200,000         PoC Score

Diminishing returns prevent compute whales from dominating
```

### Practical Impact Examples

| PoC Score | Linear Weight | √ Weight | Efficiency Ratio |
|-----------|--------------|---------|-----------------|
| 100 | 100 | 10 | 10% |
| 1,000 | 1,000 | 31.6 | 3.16% |
| 10,000 | 10,000 | 100 | 1% |
| 100,000 | 100,000 | 316.2 | 0.316% |
| 1,000,000 | 1,000,000 | 1,000 | 0.1% |

**Observation**: As PoC score increases 10×, effective weight only increases ~3.16×, creating strong incentives for decentralization.

### Economic Incentive Analysis

**Scenario**: Large provider deciding whether to run as 1 large node or 10 small nodes

**Option 1: Single Large Node**
```
PoC score = 10,000
Weight contribution = √10,000 × 1.0 = 100
```

**Option 2: Ten Small Nodes**
```
PoC score per node = 1,000
Weight per node = √1,000 × 1.0 = 31.6
Total weight = 31.6 × 10 = 316

Advantage: 316 / 100 = 3.16× more rewards
```

**Conclusion**: The square root function creates strong economic incentives for distributing compute across multiple nodes, naturally promoting decentralization.

---

## Practical Examples

### Example 1: Small Validator (Stake-Focused)

**Profile:**
- Staked: 5,000 MBO
- Lock period: 180 days (6 months)
- Reputation: 0.9
- PoC score: 500 (minimal compute)

**Coefficients (balanced):**
- C_SR = 1.0
- C_NL = 1.0

**Calculation:**

```
# Stake weight
time_multiplier = 1 + (180 / 730) × 0.5
                = 1 + 0.123
                = 1.123

reputation_factor = 0.5 + (0.9 / 2)
                  = 0.95

stake_weight = 5,000 × 1.123 × 0.95
             = 5,334.25 MBO

# Work component
work_component = √500 × 1.0
               = 22.36

# Total weight
total_weight = (5,334.25 × 1.0) + (22.36 × 1.0)
             = 5,334.25 + 22.36
             = 5,356.61

# Weight breakdown
stake_contribution = 5,334.25 / 5,356.61 = 99.6%
work_contribution = 22.36 / 5,356.61 = 0.4%
```

**Result**: Primarily stake-based validator.

---

### Example 2: Medium Validator (Balanced)

**Profile:**
- Staked: 15,000 MBO
- Lock period: 365 days (1 year)
- Reputation: 1.0
- PoC score: 25,000 (active compute provider)

**Coefficients (work-favored):**
- C_SR = 0.9
- C_NL = 1.1

**Calculation:**

```
# Stake weight
time_multiplier = 1 + (365 / 730) × 0.5
                = 1.25

reputation_factor = 0.5 + (1.0 / 2)
                  = 1.0

stake_weight = 15,000 × 1.25 × 1.0
             = 18,750 MBO

# Work component
work_component = √25,000 × 1.1
               = 158.11 × 1.1
               = 173.92

# Total weight
total_weight = (18,750 × 0.9) + (173.92 × 1.1)
             = 16,875 + 191.31
             = 17,066.31

# Weight breakdown
stake_contribution = 16,875 / 17,066.31 = 98.9%
work_contribution = 191.31 / 17,066.31 = 1.1%
```

**Result**: Still stake-dominant, but work contributes meaningfully.

---

### Example 3: Large Compute Provider

**Profile:**
- Staked: 10,000 MBO
- Lock period: 730 days (2 years)
- Reputation: 1.0
- PoC score: 1,000,000 (major GPU farm)

**Coefficients (work-favored):**
- C_SR = 0.85
- C_NL = 1.15

**Calculation:**

```
# Stake weight
time_multiplier = 1 + (730 / 730) × 0.5
                = 1.5

reputation_factor = 0.5 + (1.0 / 2)
                  = 1.0

stake_weight = 10,000 × 1.5 × 1.0
             = 15,000 MBO

# Work component
work_component = √1,000,000 × 1.15
               = 1,000 × 1.15
               = 1,150

# Total weight
total_weight = (15,000 × 0.85) + (1,150 × 1.15)
             = 12,750 + 1,322.5
             = 14,072.5

# Weight breakdown
stake_contribution = 12,750 / 14,072.5 = 90.6%
work_contribution = 1,322.5 / 14,072.5 = 9.4%
```

**Result**: Work contributes ~10%, but stake still dominates. The √ function prevents work from completely overwhelming stake.

---

### Example 4: Comparison - Centralized vs Decentralized

**Scenario**: A provider with 1,000,000 PoC score deciding between 1 large node or 100 small nodes.

**Option A: 1 Large Node**
```
Stake: 100,000 MBO (1 year lock)
PoC: 1,000,000

stake_weight = 100,000 × 1.25 × 1.0 = 125,000
work_component = √1,000,000 × 1.0 = 1,000

total_weight = 125,000 + 1,000 = 126,000
```

**Option B: 100 Small Nodes**
```
Stake per node: 1,000 MBO (1 year lock)
PoC per node: 10,000

stake_weight_per_node = 1,000 × 1.25 × 1.0 = 1,250
work_component_per_node = √10,000 × 1.0 = 100

total_weight_per_node = 1,250 + 100 = 1,350
total_weight_all = 1,350 × 100 = 135,000

Advantage: 135,000 / 126,000 = 1.071 (7.1% more rewards)
```

**Additional Benefits of Decentralization:**
- Network resilience
- Geographic distribution
- Reduced single-point-of-failure risk
- Better community perception

---

### Example 5: AIDA Response to Network Imbalance

**Initial State (Stake-Dominated):**

```
Total network:
- 50 validators
- Total stake: 2,000,000 MBO
- Total PoC: 500,000

Coefficients:
C_SR = 1.15 (stake-favored)
C_NL = 0.85 (work-penalized)

Average validator:
stake_weight = 40,000
work_component = √10,000 × 0.85 = 85

total_weight = 40,000 + 85 = 40,085
stake_ratio = 40,000 / 40,085 = 99.8%
```

**AIDA Adjustment (after 30 epochs):**

```
Target: 50/50 stake-work balance
Current: 99.8% stake

AIDA gradually adjusts:
Epoch 1:  C_SR = 1.15 → 1.14
Epoch 10: C_SR = 1.14 → 1.05
Epoch 30: C_SR = 1.05 → 0.95

New state:
C_SR = 0.95
C_NL = 1.05

Average validator:
stake_component = 40,000 × 0.95 = 38,000
work_component = 100 × 1.05 = 105

total_weight = 38,000 + 105 = 38,105
stake_ratio = 38,000 / 38,105 = 99.7%

Still dominated by stake, but work incentivized
```

**Market Response:**

More validators join to do compute work, eventually balancing the network.

---

## Parameter Reference

### Constants

| Parameter | Value | Unit | Description |
|-----------|-------|------|-------------|
| `MAX_LOCK_DAYS` | 730 | days | Maximum stake lock period |
| `LOCK_BONUS_MAX` | 0.5 | ratio | Maximum time multiplier bonus (50%) |
| `HALF_LIFE` | 30 | days | PoC score decay half-life |
| `CPU_WEIGHT` | 1.0 | - | CPU compute unit weight |
| `GPU_WEIGHT` | 10.0 | - | GPU compute unit weight |
| `RAM_WEIGHT` | 0.1 | - | RAM compute unit weight |
| `MIN_REPUTATION` | 0.0 | - | Minimum reputation score |
| `MAX_REPUTATION` | 1.0 | - | Maximum reputation score |
| `REPUTATION_FLOOR` | 0.5 | - | Minimum reputation factor (50% of stake) |

### AIDA Parameters

| Parameter | Value | Range | Description |
|-----------|-------|-------|-------------|
| `C_SR` | dynamic | [0.8, 1.2] | Stake Rewards coefficient |
| `C_NL` | dynamic | [0.8, 1.2] | Network Load coefficient |
| `C_SR + C_NL` | 2.0 | constant | Sum constraint |
| `α` (learning rate) | 0.01 | - | AIDA adjustment speed |
| `target_stake_ratio` | 0.5 | - | Target 50/50 balance |

### Economic Parameters

| Parameter | Value | Unit | Description |
|-----------|-------|------|-------------|
| `MIN_STAKE` | 1,000 | MBO | Minimum validator stake |
| `OPTIMAL_STAKE` | 10,000 | MBO | Recommended stake amount |
| `MAX_STAKE` | 1,000,000 | MBO | Maximum single validator stake |

---

## Formulas Summary

### Quick Reference

```
1. Total Weight:
   total_weight = (stake_weight × C_SR) + (√(poc_score) × C_NL)

2. Stake Weight:
   stake_weight = staked_amount × time_multiplier × reputation_factor

   time_multiplier = 1 + (lock_days / 730) × 0.5

   reputation_factor = 0.5 + (reputation / 2)

3. PoC Score:
   poc_score = Σ(compute_units × job_value × reliability × validity × decay)

   compute_units = CPU × 1.0 + GPU × 10.0 + RAM × 0.1

   decay = e^(-ln(2) × days / 30)

4. AIDA Adjustment:
   C_SR(t+1) = C_SR(t) + 0.01 × (0.5 - current_stake_ratio)

   C_NL(t+1) = 2.0 - C_SR(t+1)
```

---

## Implementation Notes

### Precision Requirements

- All MBO amounts: Use 18 decimal places (wei-equivalent)
- Coefficients (C_SR, C_NL): Use float64 precision
- PoC scores: Use uint64 or uint128
- Square root: Use high-precision library (e.g., `sqrt` from `num-traits`)

### Overflow Protection

```rust
// Safe total weight calculation
fn calculate_total_weight(
    stake_weight: u128,
    poc_score: u128,
    c_sr: f64,
    c_nl: f64,
) -> Result<u128, Error> {
    let stake_component = stake_weight
        .checked_mul((c_sr * 1e6) as u128)?
        .checked_div(1_000_000)?;

    let poc_sqrt = (poc_score as f64).sqrt();
    let work_component = ((poc_sqrt * c_nl * 1e6) as u128)
        .checked_div(1_000_000)?;

    stake_component.checked_add(work_component)
        .ok_or(Error::Overflow)
}
```

### Gas Optimization

- Cache computed values (time_multiplier, reputation_factor)
- Batch PoC score updates
- Use lookup tables for common decay values
- Pre-compute square roots where possible

---

## References

- [PoUW Consensus Mechanics](./consensus_mechanics.md)
- [Verification Strategy](./verification_strategy.md)
- [AIDA System Specification](./aida_specification.md)
- [Economic Model](./economic_model.md)

## Changelog

- **2025-11-30**: Initial PoX formula specification
  - Complete mathematical formulas
  - AIDA coefficient system
  - Anti-centralization mechanism
  - Practical examples with MBO token
  - Diminishing returns analysis
