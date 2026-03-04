# Mbongo Chain Tokenomics

**Status:** ACTIVE
**Last updated:** 2026-02-17
**Related:** [VISION_v1.md](VISION_v1.md), [COMPUTE_INTERFACE_v0.1.md](specs/COMPUTE_INTERFACE_v0.1.md), [economic_summary.md](economic_summary.md)

---

## Tokenomics — v1 vs v2 Clarification

The long-term Mbongo economic model defines a 50/50 split between PoS validators and PoUW compute providers. That design is unchanged. However, the two halves activate at different protocol versions.

### v1 (Verification Layer Only)

In v1, Mbongo validators **do not execute AI workloads**. Validators produce blocks, verify deterministic compute receipts, and maintain chain state. All AI inference execution happens off-chain.

| Function | Where it runs | Protocol version |
|----------|--------------|-----------------|
| AI inference execution | Off-chain (external executors) | n/a |
| ComputeReceipt submission | On-chain (transaction) | v0.4+ |
| Deterministic receipt verification | On-chain (apply_block) | v0.4+ |
| Settlement (fee transfer, reward) | On-chain (atomic state transition) | v0.4+ |
| PoUW execution by validators | On-chain (validator node) | **v2+ only** |

### v2+ (PoUW Execution Layer)

In v2+, validators may optionally execute lightweight inference workloads directly, earning PoUW block rewards in addition to PoS rewards. This requires hardware attestation, on-chain execution metering, and a fraud proof system — none of which exist in v1.

**PoUW execution rewards are activated in v2+ only.** No PoUW block rewards are emitted before v2.

---

## Block Reward — v1 Emission Rules

### Long-term constants (unchanged)

| Parameter | Value |
|-----------|-------|
| Total supply | 31,536,000 MBO |
| Inflation | 0% (fixed forever) |
| Block time | 1 second (target) |
| Annual blocks | 31,536,000 |
| Initial block reward (Years 1-5) | 0.1 MBO |
| Halving period | 5 years |
| Long-term PoS share | 50% |
| Long-term PoUW share | 50% |

### v1 emission behavior

During v1, the PoUW half of the block reward is not distributed:

| Component | v1 behavior | v2+ behavior |
|-----------|-------------|--------------|
| PoS share (50% = 0.05 MBO/block) | **Emitted.** Distributed to validators. | Emitted. Distributed to validators. |
| PoUW share (50% = 0.05 MBO/block) | **Reserved. Not emitted.** Held in protocol reserve until v2 activation. | Emitted. Distributed to compute providers. |
| Effective v1 block reward | 0.05 MBO/block | 0.1 MBO/block |

This means:
- v1 emits half the long-term rate. Total supply is reached more slowly.
- The reserved PoUW allocation is not burned, not redistributed, and not accessible. It remains unissued.
- When v2 activates PoUW, the full 0.1 MBO/block emission resumes from the current block forward. No retroactive issuance of reserved tokens.
- The halving schedule applies to the full 0.1 MBO rate. In v1, the effective halving applies to the 0.05 MBO PoS emission only.

---

## Genesis Allocations (unchanged)

| Category | % | MBO Amount | Unlock |
|----------|---|------------|--------|
| PoS Validators/Delegators | 40% | 12,614,400 | Per-block emission |
| PoUW Compute Providers | 20% | 6,307,200 | Per-block emission (v2+ only) |
| Ecosystem Grants | 15% | 4,730,400 | Milestone-based |
| Foundation & Operations | 10% | 3,153,600 | 4-year linear vesting |
| Community & Incentives | 10% | 3,153,600 | Per-epoch stream |
| Early Contributors | 5% | 1,576,800 | 4-year, 1-year cliff |

The PoUW Compute Providers allocation (20%, 6,307,200 MBO) is reserved and begins emission only when v2 PoUW is activated. In v1, this allocation remains unissued.

---

## v1 Verification Fee Model

In v1, compute receipt verification is funded by task submitters, not by block rewards.

### Fee components

Each `ComputeReceipt` submission transaction includes:

| Fee component | Description |
|---------------|-------------|
| `verification_fee` | Paid in MBO by the task submitter. Covers the cost of on-chain deterministic verification. |
| `base_gas_fee` | Standard transaction gas. Burned (same as all transactions). |
| `priority_fee` | Optional. Incentivizes faster block inclusion. |

### Fee distribution in v1

| Destination | Share | Mechanism |
|-------------|-------|-----------|
| Validators | 90% of `verification_fee` | Distributed to the block producer that includes and verifies the receipt. |
| Burn | 10% of `verification_fee` | Burned. Subject to future AIDA bounds adjustment in v2+. |
| Burn | 100% of `base_gas_fee` | Burned (same as all transactions). |
| Block producer | 100% of `priority_fee` | Standard priority fee routing. |

### Executor payment

Executors (off-chain compute providers) are **not paid by the Mbongo protocol in v1**. Payment between task submitters and executors occurs through one of:

- Off-chain bilateral agreements.
- Escrow contracts (future, not in v1 scope).
- External payment rails.

Mbongo v1 verifies that work was done correctly. It does not enforce executor compensation. Executor payment enforcement is a v2+ feature.

---

## v1 Slashing Model (Minimal)

### Slashable offenses in v1

| Offense | Penalty | Destination |
|---------|---------|-------------|
| Double-signing blocks | 5% of stake | Burned |
| Liveness fault (extended downtime) | 0.5% of stake | Burned |
| Signing invalid receipt verification | TBD (requires RFC) | Burned |

### Dispute resolution

- All disputes are resolved **deterministically by replay**. Any node can re-execute the verification logic against the receipt and the chain state at the relevant block height.
- There is no subjective governance, no committee vote, and no manual override in v1 slashing.
- Slashing parameters for invalid receipt verification will be defined in the receipt verification RFC (v0.4 scope). Until that RFC is accepted, only block-level slashing (double-sign, liveness) is active.

### Constants preserved from long-term model

| Parameter | Value | Status |
|-----------|-------|--------|
| Double-signing penalty | 5% of stake | Active in v1 (when PoS activates in v0.3) |
| Downtime penalty | 0.5% of stake | Active in v1 (when PoS activates in v0.3) |
| Invalid compute penalty | 1,000 MBO fixed | **Deferred.** Requires receipt verification RFC. |
| Unbonding period | 21 days | Active in v1 (when PoS activates in v0.3) |
| All slashed MBO | Burned (not redistributed) | Active in all versions |

---

## PoUW Activation Gate

PoUW execution rewards activate when **all** of the following conditions are met:

1. An RFC defining the PoUW execution model is accepted by Core Maintainers.
2. A protocol version bump (v2.0) is tagged.
3. A new PROTOCOL_LOCK document is published.
4. Hardware attestation and execution metering are implemented and audited.
5. The fraud proof / challenge system for compute results is live.

Until these conditions are met, the PoUW allocation remains unissued and the block reward is 50% of the long-term rate.

**This is a protocol-enforced gate, not a governance decision.** No vote, no multisig, and no admin key can activate PoUW emission early.

---

## Version Summary

| Version | Block reward | PoS emission | PoUW emission | Verification fees |
|---------|-------------|--------------|---------------|-------------------|
| **v0.2** (current) | 0 (no consensus) | n/a | n/a | n/a |
| **v0.3** (planned) | 0.05 MBO/block | Active | Reserved | n/a |
| **v0.4** (planned) | 0.05 MBO/block | Active | Reserved | Active |
| **v1.0** (target) | 0.05 MBO/block | Active | Reserved | Active |
| **v2+** (future) | 0.1 MBO/block | Active | Active | Active |

---

## References

| Document | Description |
|----------|-------------|
| [VISION_v1.md](VISION_v1.md) | Strategic vision: verification layer, not execution layer |
| [COMPUTE_INTERFACE_v0.1.md](specs/COMPUTE_INTERFACE_v0.1.md) | Compute types, receipt format, reserved RPC methods |
| [economic_summary.md](economic_summary.md) | Long-term economic model (PoS + PoUW combined) |
| [PROTOCOL_LOCK_v0.2.md](specs/PROTOCOL_LOCK_v0.2.md) | Frozen protocol surfaces |
| [RFC_PROCESS.md](RFC_PROCESS.md) | How to propose economic parameter changes |
