# RFC 0003 — MBO Monetary Policy & Security Budget

**Status:** DRAFT — RESEARCH — NON-NORMATIVE
**Author:** Gilbert Kalombo
**Created:** 2026-08-28
**Protocol version:** none (this RFC changes no protocol version)
**Locked surfaces affected:** none

> This RFC selects no monetary policy, changes no consensus rule, and modifies
> no runtime behaviour. It establishes what the repository actually implements
> today, separates that from what it merely documents, and provides a
> reproducible way to compare candidate policies. Nothing in it is normative.

---

## 1. Purpose

Mbongo Chain must eventually answer one question:

> How should the network fund long-term security and protocol incentives while
> maintaining a credible monetary policy?

That question cannot be answered by argument alone. Candidate policies differ
over decades, and their differences only become visible when they are run
forward on identical assumptions. This RFC therefore does two things:

1. records the **factual monetary baseline** of the repository, separating
   implemented behaviour from documented intent;
2. introduces a **deterministic 100-year simulation framework** so candidate
   policies can be compared reproducibly.

Choosing a policy is explicitly out of scope. See [§16 Decision Gates](#16-decision-gates)
for what must be true before a recommendation becomes possible.

---

## 2. Status and Non-Normative Scope

This document is research. Concretely:

- No value in this RFC or in `research/monetary-policy/scenarios.json` is a
  proposal, a target, or a protocol parameter.
- The simulation does **not** predict the market price of MBO. Price is an
  exogenous scenario input.
- Where the repository's documentation and its code disagree, this RFC records
  both and does not resolve the disagreement.
- The scenario set holds the documented primary issuance schedule constant, so
  it compares tail behaviour rather than the full design space, and `base` is
  **not** a neutral baseline. See [§11 Current scenario-set scope](#current-scenario-set-scope).
- `historical_documented_partial` is a **partial** reconstruction: faithful on
  the issuance schedule and cap, deliberately incomplete on fees. The
  documented fee system has two channels (base fee burned, priority fee to
  validators/providers) whose ratio the documentation never states, and this
  simulator has one aggregate fee pool. That scenario therefore models the
  base-fee burn only, gives validators zero fee revenue by construction, and
  **understates the documented security budget**. No base/priority ratio has
  been invented to close the gap.
- Statements are labelled by status: **IMPLEMENTED** (present in `crates/`),
  **DOCUMENTED** (written in `docs/`, not enforced by the runtime),
  **PROVISIONAL** (explicitly marked as not final by its own source),
  **NOT_FOUND** (no evidence in the repository).

---

## 3. Current Repository Baseline

Audited on `dev` at `e80791c9a1fb5cd719944750c164dd8c892fa301`. Every row was
checked against the file cited; no row is inferred from history or intent.

### 3.1 What the runtime implements

| Property | Status | Evidence | Notes |
|---|---|---|---|
| Native balances | **IMPLEMENTED** | `crates/mbongo-core/src/account.rs` — `Account { address, balance: u128, nonce }` | Balance is documented in-code as "the smallest unit" |
| Signed transfers | **IMPLEMENTED** | `crates/mbongo-node/src/backend.rs` — `Account::transfer` in `apply_block` | Sender signature, nonce and sufficient balance are all enforced |
| Replay protection | **IMPLEMENTED** | `backend.rs` — `validate_and_increment_nonce` | Per-account nonce, consumed by every transaction |
| Genesis allocation | **IMPLEMENTED** | `backend.rs` — `ensure_genesis`, `dev_account.balance = 1_000_000_000` | A single well-known development account. This is devnet scaffolding, not a distribution model |
| `Stake` transaction type | **IMPLEMENTED (encoding only)** | `crates/mbongo-core/src/primitives.rs` — `TransactionType::Stake` (index `0x02`) | `apply_block` deliberately routes it through the plain transfer path (see the RFC 0002 non-goals comment). No lock, no reward, no slashing |
| Named unit "MBO" | **NOT_FOUND** in code | Only a doc comment in `primitives.rs:153` ("MBO or compute units") | No symbol, denomination constant or decimals exist in `crates/` |
| Minimum denomination | **NOT_FOUND** | — | `balance: u128` is described as a smallest unit, but no conversion constant is defined |
| Supply cap enforcement | **NOT_FOUND** | `git grep max_supply -- crates/` → 0 matches | Nothing in the runtime tracks or bounds total supply |
| Issuance / emission | **NOT_FOUND** | `git grep -iE 'issuance\|emission' -- crates/` → 0 matches | `produce_block` mints no value; blocks carry no coinbase |
| Block reward | **NOT_FOUND** | `git grep block_reward -- crates/` → 0 matches | — |
| Validator rewards | **NOT_FOUND** | `crates/mbongo-node/src/backend.rs` — `list_validators` returns empty ("no validator tracking yet") | — |
| Staking mechanism | **NOT_FOUND** | `crates/mbongo-consensus/src/lib.rs` — modules commented out, "to be implemented" | The PoX/AIDA formulas in that crate are documentation in a stub, not code |
| Transaction fees | **NOT_FOUND** | `git grep -i fee -- crates/` → 0 matches | Transactions are free today |
| Burn | **NOT_FOUND** | `git grep -i burn -- crates/` → 0 matches | — |
| Treasury | **NOT_FOUND** | `git grep -i treasury -- crates/` → 0 matches | — |
| Compute ↔ MBO economic link | **NOT_FOUND** in code | — | `TransactionType::ComputeTask` exists but, like `Stake`, follows the transfer path |

**Consensus-critical monetary properties today** are therefore exactly three:
conservation of balances across a transfer, sufficiency of the sender's
balance, and nonce monotonicity — all enforced in `apply_block` and re-executed
by every node. Supply is changed by nothing except genesis. There is no
issuance to be consensus-critical about.

### 3.2 What the documentation describes

| Property | Status | Evidence | Notes |
|---|---|---|---|
| Max supply 31,536,000 MBO | **DOCUMENTED** | `docs/supply_schedule.md`, `docs/monetary_policy.md`, `docs/economic_security.md`, `docs/economic_summary.md`, `docs/token_distribution.md` | Described as "mathematically capped" and "enforced at the protocol level". **The runtime enforces nothing of the kind** |
| Emission via block rewards only | **DOCUMENTED** | `docs/supply_schedule.md` §2.1 | "All 31,536,000 MBO are created exclusively through block rewards" |
| Initial block reward 0.1 MBO | **DOCUMENTED** | `docs/supply_schedule.md` §3.1 | Years 1–5 |
| Halving every 5 years | **DOCUMENTED** | `docs/supply_schedule.md` §1.1, §3.2 | "Block rewards decrease by 50% every 5 years"; the 25-year table is fully tabulated |
| 31,536,000 blocks per year | **DOCUMENTED** | `docs/supply_schedule.md` §3.1 | Implies a 1-second block time. The runtime default is 5 s (`--block-time`), and `docs/ALIGNMENT_AUDIT_2026-02.md` already calls 1 s "aspirational, not implemented" |
| Allocation 40/20/15/10/10/5 | **DOCUMENTED** | `docs/token_distribution.md` §2.1 | PoS validators 40%, PoUW compute 20%, ecosystem 15%, foundation 10%, community 10%, early contributors 5% |
| Base fee burned, priority fee to validators/providers | **DOCUMENTED** | `docs/economic_summary.md:42,95-96` | Two channels. The **ratio between them is not stated**. Also: slashed stake burned, invalid-compute penalties burned |
| Post-cap security via fees | **DOCUMENTED** | `docs/monetary_policy.md` §5 | "Validator and compute rewards transition to transaction fee distribution" |
| "No inflation — ever" | **DOCUMENTED** | `docs/monetary_policy.md` §2 | A strong claim with no implementation behind it |
| `reward_split` 70/20/10 | **PROVISIONAL** | `docs/specs/COMPUTE_INTERFACE_v0.1.md:152` | The source itself marks it "Proposed, not final" |
| 50,000 MBO minimum stake | **PROVISIONAL** | `docs/ALIGNMENT_AUDIT_2026-02.md:89` | Recorded there as a placeholder pending a staking RFC |
| 1,000 MBO slashing | **PROVISIONAL** | `docs/ALIGNMENT_AUDIT_2026-02.md:90` | That audit calls the figure "**Premature**" |

### 3.3 Hypotheses checked and not found

These were specifically searched for, to avoid carrying an assumption forward
as fact:

| Hypothesis | Result |
|---|---|
| Year 1 issuance = 10% of supply, reduced 20% per year, over ~10 years | **NOT_FOUND.** No such schedule exists anywhere in the repository. What is documented is a 5-year halving of a per-block reward |
| 2-second block time | **NOT_FOUND.** Documentation implies 1 s; the runtime default is 5 s |
| 70/20/10 split | **FOUND but PROVISIONAL** — see above |
| 31,536,000 as a settled hard cap | **FOUND in documentation only.** It is not implemented, not enforced, and not reachable by any code path in `crates/` |

### 3.4 The gap this RFC starts from

The repository contains a detailed, internally consistent monetary design in
`docs/` and a transfer-only runtime in `crates/`. The documentation is not
wrong so much as **unbuilt**, and parts of it are already flagged as premature
by the project's own alignment audit.

That gap is an opportunity rather than a defect: because no monetary rule is
implemented, none has to be preserved for compatibility. The design space is
genuinely open, and this is the cheapest moment in the project's life to
examine it.

One numerical coincidence is worth recording so nobody rediscovers it as a
mystery: **31,536,000 is the number of seconds in a 365-day year.** The same
figure is used both as the maximum supply and as the number of blocks per year
at a 1-second block time, which is how "0.1 MBO per block for five years"
produces exactly half the cap. This is an elegant construction, and also a
reminder that the cap's magnitude was derived from block timing rather than
from a security-budget requirement.

---

## 4. Monetary Policy Objectives

Objectives, not parameters. Each is a property a candidate policy can be
evaluated against; none prescribes a number.

1. **Long-term security sustainability.** The network should be able to pay for
   its own security indefinitely, without assuming that token price rises
   forever.
2. **Credible, predictable rules.** Holders and validators should be able to
   reason about future supply without trusting discretionary intervention.
3. **Bounded dilution.** If any issuance persists, its ceiling should be
   explicit and small enough to be defensible to a long-term holder.
4. **Economic utility over scarcity theatre.** Scarcity is a means to fund
   security, not an end in itself.
5. **Ability to compensate security providers** in the currency the protocol
   controls, rather than relying entirely on a fee market that may not exist
   for years.
6. **Legibility.** A policy nobody can explain in a paragraph will not survive
   contact with governance.

These objectives are in tension. A hard cap maximises (2) and (3) and puts all
weight on the fee market for (1). A tail emission does the reverse. That
tension is exactly what the simulation exists to make visible.

---

## 5. Economic Actors

| Actor | Status in the codebase | Role in the model |
|---|---|---|
| **Users** | Implemented (accounts, transfers) | Pay transaction fees and compute spend |
| **Validators** | Not implemented (no validator set, no rewards) | Receive issuance share and fee share; provide security |
| **Compute workers** | Not implemented (compute is a transaction type only) | Receive compute revenue from users |
| **Stakers** | Not implemented (`Stake` is encoding only) | Lock supply; determine `security_value_at_risk` |
| **Treasury / protocol** | Not implemented | Receives a fee share; funds ecosystem work |

Only users exist today. Every other actor is modelled prospectively, and the
model does not assume any of them will materialise.

---

## 6. Security Budget

The **security budget** is what the protocol pays, per year, to the parties
that secure it. In the model:

```
security_budget_mbo = validator_issuance_revenue + validator_fee_revenue
```

Reported both in MBO and, multiplied by the scenario's exogenous reference
price, in reference value. Keeping the two separate matters: a policy can hold
its MBO budget flat while its reference value collapses, and a reader who sees
only one of the two will draw the wrong conclusion.

The essential long-run question is whether, as issuance decays, fee revenue
grows enough to replace it — and if not, what the network intends to do about
that. No policy family answers this by construction; the simulation shows the
shape of the answer under stated assumptions.

---

## 7. Compute Economics Separation

The model deliberately does **not** treat compute worker revenue as monetary
issuance. They are different flows with different funding sources:

- **Compute market revenue** originates from users buying compute. It splits
  into worker revenue and a protocol fee. It is a market, not a subsidy.
- **Monetary issuance** originates from the protocol creating supply. It
  dilutes holders and is bounded by policy.

Conflating them produces two classic errors: assuming issuance must scale with
compute demand, and assuming compute demand can substitute for a security
budget. The model keeps them separate and lets a scenario connect them through
one explicit channel — the protocol's fee on compute, which flows into the same
fee pool as transaction fees and is therefore available to security.

This separation is a modelling decision, not a protocol decision.

---

## 8. Monetary Policy Families

All four are candidates. None is recommended.

| Family | Definition | Principal open risk |
|---|---|---|
| **Hard cap** | Primary emission decays to zero; cumulative issuance never exceeds a configured cap (ending supply may be lower after burns) | Security depends entirely on fee revenue once emission ends |
| **Fixed tail** | After the primary phase, a constant number of MBO per year | Dilution falls asymptotically toward zero as supply grows, but never stops |
| **Percentage tail** | After the primary phase, a constant fraction of supply per year | Perpetual, constant-rate dilution; the supply curve never flattens |
| **Adaptive bounded** | Issuance moves within an explicit `[min_rate, max_rate]` band in response to security coverage | Requires an on-chain measurement and a controller, neither of which is designed; introduces governance surface |

The adaptive family is included as a **research comparator only**, and its
current implementation is narrower than its name suggests. As implemented it is
a **two-state (bang-bang) controller**: below `target_security_ratio` it issues
at `max_rate`, otherwise at `min_rate`, and never at any intermediate rate.

Three properties must be stated plainly before anyone cites it:

- `target_security_ratio` is a **free illustrative parameter**. It is not
  derived from any Mbongo threat or security model, because none exists yet.
- The controller reads the previous year's values, so it lags by one period.
- The ratio it evaluates compares two reference-value quantities, so the MBO
  reference price appears in both numerator and denominator and **cancels**.
  The controller is therefore **price-invariant by construction**: a price
  collapse changes its issuance by exactly nothing. This was verified by running
  the scenario with and without a 90% price shock, and a test now asserts it.

**This scenario must not be interpreted as a proposed Mbongo adaptive monetary
policy.** It exists only to exercise the simulation framework. This RFC proposes
no controller design, no on-chain measurement, and no governance mechanism.

---

## 9. 100-Year Simulation Methodology

- **Horizon.** Years 0–100 inclusive. Year 0 records the starting state and
  issues nothing, so a run produces 101 observations. Checkpoints are reported
  at years 1, 5, 10, 20, 30, 50, 75 and 100.
- **A 100-year horizon is a modelling choice, not a protocol commitment.** It
  is long enough for a decaying emission schedule to fully play out. Nothing
  here suggests the protocol should encode an immutable century-long rule.
- **Units.** MBO quantities and reference-value quantities are separate columns
  and are never added together.
- **Price is exogenous.** Each scenario declares a price path. The model does
  not forecast price and must never be cited as if it did.
- **Determinism.** No randomness, no clock dependence. Same version plus same
  scenario plus same horizon produces byte-identical output.
- **Scenarios are data.** Adding a scenario means editing
  `research/monetary-policy/scenarios.json`, never `model.py`. A test asserts
  that no shipped scenario is marked normative.

---

## 10. Accounting Identities

Enforced by `identity_violations()` and by the test suite, on every scenario
and every year:

| Identity | Purpose |
|---|---|
| `ending_supply = starting_supply + gross_issuance − burned` | Supply conservation |
| `worker_revenue + protocol_compute_fee = compute_user_spend` | Compute spend has exactly two destinations |
| `validator_fee_revenue + treasury_revenue + burned = fee_pool` | The fee pool is split once, three ways |
| `security_budget_mbo = validator_issuance_revenue + validator_fee_revenue` | The budget is exactly its components |
| `staked_supply ≤ circulating_supply ≤ ending_supply` | Supply layers are ordered |
| `starting_supply(y) = ending_supply(y−1)` | Years chain without leakage |
| every value is finite | No NaN or infinity escapes |

Burn is subtracted in exactly one place, so it cannot be double-counted, and
the fee pool is formed once from transaction fees plus the protocol's compute
fee, so no revenue is counted twice.

### Issuance, cap and supply are three different things

| Term | Meaning |
|---|---|
| **Cumulative issuance** | Total MBO created by protocol issuance over the run |
| **Cumulative issuance cap** | Upper bound on cumulative issuance in a hard-cap model |
| **Ending / outstanding supply** | Issued MBO still outstanding after modelled burns |

**A cumulative issuance cap is not an ending supply**, and the two must not be
used interchangeably in this RFC, in `scenarios.json`, or in any future
whitepaper drawing on them. A scenario capped at 31,536,000 can end far below
that figure once burns apply: `historical_documented_partial` ends near 2.5M
MBO precisely because it burns its entire fee pool.

A further property of the documented schedule **as reconstructed by this
model**: the halving series is geometric, so cumulative issuance approaches the
cap asymptotically without reaching it. A 100-year run yields **31,535,969.92
MBO**, not 31,536,000. This is a property of the modelled schedule, not a
protocol guarantee.

**Source-document arithmetic inconsistency.** `docs/supply_schedule.md` derives
31,536,000 from "365.25 days × 24 × 60 × 60", but that product is 31,557,600.
The figure 31,536,000 corresponds to a **365-day** year. This RFC records the
discrepancy and does not resolve it; correcting the source document is left to
a later documentation or tokenomics decision.

---

## 11. Adoption Scenarios

Four configurations — `pessimistic`, `conservative`, `base`, `high_adoption` —
sharing one engine. Each expresses activity as a piecewise path with an initial
value and phases carrying an annual growth rate that **may be negative**, which
is how the framework represents growth, maturity, stagnation and contraction
without assuming any of them.

No scenario compounds indefinitely: every growth path flattens, and the
pessimistic path declines. Perpetual exponential adoption is not modelled
because it is not a defensible assumption over a century.

### Current scenario-set scope

The engine can parameterise several policy families, but **the scenario set in
this RFC deliberately does not explore the complete MBO monetary-policy design
space.**

Every shipped scenario reuses the same primary issuance schedule —
`initial_annual_issuance 3153600`, `step_years 5`, `step_factor 0.5` — which is
the schedule documented in `docs/supply_schedule.md`. The set therefore
compares what happens **after** that schedule (hard-cap termination, fixed
tail, percentage tail, experimental adaptive tail) and how adoption and stress
assumptions interact with it. It does not compare alternative primary-issuance
designs, because none is configured.

**The `base` scenario is therefore not a neutral monetary baseline.** It is an
illustrative benchmark anchored on the documented historical schedule, varying
fees, allocation, staking and adoption while holding issuance fixed.

Future economic experiments must independently vary: initial issuance
magnitude, primary issuance duration, decay interval, decay factor, issuance
curve shape, hard cap, tail policy, fee structure, burn policy, and security
allocation. This RFC assigns no values to any of them.

---

### Primary-issuance sensitivity study

The first experiment run on this framework varies the primary issuance
schedule itself, across 80 combinations of initial annual issuance, decay
interval and decay factor, holding every non-issuance assumption constant.
It is sensitivity analysis, not optimization, and it selects no policy. See
[research/monetary-policy/README.md](../../research/monetary-policy/README.md)
for the grid, controls and reproduction command.

Two findings from that study belong in this RFC because they bear on the
baseline recorded in §3, not on any policy choice:

**The documented cap and the documented schedule are structurally coupled.**
For the documented parameters the infinite geometric issuance limit is
`I × S / (1 − F)` = `3,153,600 × 5 / 0.5` = **31,536,000** — exactly the
documented cap. The historical schedule therefore sits precisely on its own
cap, and any larger, slower-decaying or longer-stepped schedule exceeds it.
Measured consequence: holding that cap constant clamps 52 of 80 schedules
onto an identical total, which is why the study runs an uncapped matrix and a
fixed-cap matrix separately rather than one blended matrix. This is an
observation about the documented figures, **not** an argument for keeping or
removing the cap.

**The model implements a supply cap, not a cumulative issuance cap.**
`clamp_to_cap` derives headroom from current supply, so burns reopen issuance
capacity. Under the controlled fee split, cumulative issuance reaches up to
1.117× the cap while outstanding supply stays at or below it. This RFC
records the behaviour as implemented; whether a future policy should cap
cumulative issuance, outstanding supply, or both is an open question, added
to §15.

### Cap semantics comparative study

The ambiguity recorded above is not an artefact of the model. It is present
in the historical documentation, in the same file:

| Reading | Evidence | Language |
|---|---|---|
| Lifetime cumulative issuance | `docs/economic_security.md:137` | "Σ (all MBO **ever created**) ≤ 31,536,000", called "UNCONDITIONALLY TRUE for all time" |
| Outstanding supply at time t | `docs/economic_security.md:1205` | "∀t: Σ(all MBO **at time t**) ≤ 31,536,000", under "CLAIM 1: SUPPLY INVARIANT" |

Both readings are reinforced elsewhere — cumulative in
`docs/supply_schedule.md:771` ("Sum all block rewards ≤ 31,536,000") and
`docs/reward_mechanics.md:566`; outstanding in `docs/incentive_design.md:1219`
("Compare total_supply to 31,536,000") and `docs/token_distribution.md:516`.
Most of the remaining ~95 occurrences are simply "Total Supply: 31,536,000
MBO" and settle nothing. **No document poses the question, so none resolves
it.** These documents are evidence of the ambiguity and are deliberately
left unedited by this RFC.

Burn permanence *is* documented — `docs/economic_summary.md:276` ("ALL BURNS
PERMANENTLY REDUCE SUPPLY"), `docs/incentive_design.md:1114`,
`docs/economic_security.md:1029` ("No mechanism to recover burned MBO").
Whether a burn reopens *issuance capacity* is documented nowhere, in either
direction: a repository-wide search for reissue, remint, recycle, reopen or
replenish returns nothing. Recorded as **NOT_SPECIFIED**, not filled in by
convention.

`cap_semantics.py` therefore runs the same 80 schedules under all three
readings, with every non-cap assumption held constant, and with a no-burn
control. It selects none of them. Three results bear on the baseline in §3:

**Divergence between the readings is entirely burn-driven in this model.**
Without burns, all three families produce identical trajectories across all
80 schedules. With the controlled fee split, the lifetime and outstanding
readings differ on 52 of 80 schedules, with a maximum year-100 gap of
3,690,211 MBO in cumulative issuance and 63,901 MBO in security budget.

**The documented schedule is unaffected by the choice.** `3,153,600 / 5 /
0.50` produces byte-identical 101-row trajectories under all three families
and clamps under none, converging to 31,535,969.92 — just under the cap,
never touching it. The semantic question is therefore live for the space of
*candidate* schedules, not for the documented one.

**A lifetime cap and a dual cap are not the same constraint**, although they
coincide here. The identity is exact:
`outstanding_headroom − lifetime_headroom = cumulative_burn − initial_supply`.
With `initial_supply = 0` the lifetime bound always binds, so dual collapses
onto lifetime. Given a non-zero genesis allocation they separate, and a
lifetime cap then permits outstanding supply above the cap, because it
bounds what is created rather than what exists. Recorded as a property of
the families; this RFC does not say which property is desirable.

## 12. Stress Scenarios

Shocks are applied inside an explicit, inclusive year window and nowhere else —
a property the tests verify directly. Supported shock metrics:
`mbo_reference_price`, `compute_user_spend_mbo`, `transaction_fee_revenue_mbo`,
`staking_ratio`.

The framework supports, and ships illustrative configurations for, a sustained
price collapse, a multi-year compute contraction, a fee-revenue collapse, a
larger illiquid share of supply, reduced staking participation, and a combined
shock. **The years and magnitudes in the shipped scenarios are illustrative
only** and carry no claim that such an event will occur in that year.

---

## 13. Security Metrics and Limitations

The model reports `security_value_at_risk = staked_supply × reference_price`.

**This is not a cost of attack, and must not be quoted as one.** The chain has
no staking, no validator set, no slashing and no finality rule implemented, so
no rigorous attack cost can be derived from this repository. The metric is a
conservative proxy: the reference value of stake in the modelled world.

Further limitations, stated plainly:

- Staking ratio and circulating fraction are exogenous assumptions, not
  behavioural responses to yield.
- Fee revenue is an assumed path, not the output of a fee market model.
- Arithmetic is floating point; identities are checked to 1e-9 relative
  tolerance. Results are indicative at that precision.
- The model says nothing about token distribution, vesting or governance
  capture, all of which affect real security.

---

## 14. Governance Questions

Open questions only. This RFC designs no governance.

- Which monetary rules should be immutable, and which should be changeable?
- If any parameter can change, what process changes it, and on what notice?
- Should bounds (for example a maximum issuance rate) be constitutional even
  if the value inside them is adjustable?
- Should a monetary change require delayed activation, and how long?
- What prevents a security-budget emergency from becoming a pretext for
  unbounded issuance?
- How is a monetary rule credibly committed to, given that the chain is young
  and the validator set does not yet exist?

---

## 15. Open Questions

The criteria and decision gates these questions must pass through are defined
in [RFC 0004](0004-mbo-monetary-decision-framework.md), which selects no
policy either.

- Hard cap or not?
- If capped, at what magnitude — and derived from what, given that 31,536,000
  came from seconds-per-year rather than from a security requirement?
- How large should the primary emission phase be, and over how long?
- Tail emission at all? If so, fixed amount or percentage of supply?
- Should fees be burned, redistributed, or split — and in what proportion?
- Should a cap, if any, bound cumulative issuance, outstanding supply, or
  both? Historical documentation asserts both and settles neither; the model
  currently caps supply, so burns reopen issuance capacity. Measured: the
  readings diverge on 52 of 80 candidate schedules with burns and on none
  without, and not at all for the documented schedule.
- Should any issuance be allocated to a treasury?
- What is the validator security floor, if any?
- Should compute carry a protocol fee, and should it fund security?
- What is the genesis distribution, and does the documented 40/20/15/10/10/5
  allocation survive contact with a real launch?
- Vesting for any allocated supply?
- What, precisely, is the economic relationship between MBO and compute?
- Does the documented "no inflation — ever" claim remain the project's
  position once the security budget is modelled?

---

## 16. Decision Gates

A monetary policy recommendation becomes possible only when all of these hold.
None of them holds today.

1. **A staking and validator-reward design exists**, at least at RFC level, so
   "who is paid, for what" has an answer.
2. **A fee model exists**, so fee revenue is a modelled mechanism rather than
   an assumed path.
3. **A security requirement is stated** — what the network must be able to pay
   for, under what threat assumption — so a budget can be judged adequate or
   not.
4. **The genesis distribution question is settled**, since it determines
   circulating supply, which determines everything downstream.
5. **The simulation has been run against those mechanisms**, not against
   assumed paths, and the results have been reviewed by someone who did not
   write them.
6. **The documentation/implementation gap in §3 has been resolved or
   explicitly acknowledged**, so the project stops carrying two incompatible
   monetary stories.

---

## 17. Implementation Boundary

**This RFC does not modify consensus or runtime.**

It adds no crate, no dependency, no protocol version, no transaction type, no
storage change, no RPC change, and no CI change. It touches only
`docs/rfcs/` and `research/`. The simulation is off-chain, standard-library
Python, and has no connection to the node.

Nothing in `crates/` changes, and no locked surface from
[PROTOCOL_LOCK_v0.3.md](../specs/PROTOCOL_LOCK_v0.3.md) is affected.

---

## 18. Reproducibility

```bash
# tests
python3 research/monetary-policy/test_model.py

# a century under the base scenario
python3 research/monetary-policy/model.py --scenario base --years 100

# every available scenario
python3 research/monetary-policy/model.py --list

# per-year CSV
python3 research/monetary-policy/model.py --scenario base --years 100 \
    --output /tmp/base.csv
```

Python 3.8+ standard library only. See
[research/monetary-policy/README.md](../../research/monetary-policy/README.md)
for the model's structure, its metrics and how to add a scenario.

---

## 19. What This RFC Concludes

That candidate monetary policies can now be compared reproducibly, on stated
assumptions, over a century — and that the repository's monetary baseline is
documentation rather than implementation.

**It does not conclude which policy Mbongo Chain should adopt.** That decision
is deferred to a future RFC, gated on §16.
