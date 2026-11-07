# RFC 0004 — MBO Monetary Policy Decision Framework

**Status:** DRAFT — RESEARCH — NON-NORMATIVE
**Author:** Gilbert Kalombo
**Created:** 2026-08-28
**Protocol version:** none (this RFC changes no protocol version)
**Locked surfaces affected:** none
**Companion to:** [RFC 0003](0003-mbo-monetary-policy-security-budget.md)

> **THIS RFC DOES NOT SELECT MBO MONETARY POLICY.**
>
> It selects no cap, no cap semantics, no issuance schedule, no burn rule and
> no security budget target. It ranks no candidate and declares no preference.
> It defines the criteria, the decision axes and the gates that a future
> proposal must satisfy before any of those questions can be answered.
>
> RFC 0003 established what the repository implements, what it merely
> documents, and how to compare candidates reproducibly. This RFC defines
> what a candidate would have to demonstrate.

---

## 1. Why a framework rather than another simulation

Three studies now exist and are CI-protected: the base model (41 tests), the
primary-issuance sensitivity study (22 tests) and the cap-semantics
comparison (31 tests). Between them they can already produce an unlimited
number of trajectories.

That is the problem this RFC addresses. Adding a fourth arbitrary simulation
would produce more numbers without making any of them decisive, because no
one has written down what a number would have to show in order to settle
anything. **The binding constraint is not simulation capacity. It is the
absence of stated criteria.**

So this RFC adds no simulation. It states the objectives, separates the
properties a policy must never violate from the ones that trade off, lists
the decision gates that are still open, and identifies the minimum set of
further experiments that a gate actually requires. Experiments not tied to a
gate are deliberately excluded.

---

## 2. Evidence classification

Every statement used below is classified. The categories are not
interchangeable, and the classification is the point: this project has
carried documentation that reads as protocol fact for long enough that the
distinction has to be mechanical.

| Class | Meaning |
|---|---|
| `RUNTIME_FACT` | Provable from code in `crates/`. |
| `HISTORICAL_DOCUMENTATION` | Asserted in `docs/`. Not implemented, not verified. |
| `CURRENT_RESEARCH_MODEL` | Behaviour of `research/monetary-policy/`. An implementation choice, not a finding. |
| `EXPERIMENTAL_RESULT` | Measured output of a study in this repository. |
| `OPEN_POLICY_QUESTION` | Not answered anywhere. |

### 2.1 RUNTIME_FACT

Searched across `crates/**/*.rs`:

| Concept | Occurrences | Status |
|---|---|---|
| `issuance` | 0 | not implemented |
| `burn` | 0 | not implemented |
| `block_reward` | 0 | not implemented |
| `max_supply` / `supply_cap` | 0 | not implemented |
| `total_supply` | 0 | not implemented |
| `inflation` | 0 | not implemented |
| `mint` | 3 | all refer to **minting blocks**, not tokens |
| `reward` | 1 | a doc comment on an AIDA coefficient |

What does exist: `Account { address, balance: u128, nonce }` with `credit`,
`debit` and `transfer` (`crates/mbongo-core/src/account.rs`).
`TransactionType::Stake` exists as an enum variant; no staking state
machine, no stake accounting, no reward distribution accompanies it.

One code path creates balance from nothing:
`Backend::ensure_genesis` (`crates/mbongo-node/src/backend.rs:143`) writes a
genesis block and pre-funds a hardcoded development key with
`1_000_000_000` smallest units. It is unconditional, tied to no schedule, no
cap and no denomination, and exists for devnet convenience.

> **RUNTIME_CAP_SEMANTICS = NONE.** The runtime enforces neither reading of
> 31,536,000, because it enforces no monetary rule at all. Nothing in this
> RFC may be phrased as though it did.

### 2.2 HISTORICAL_DOCUMENTATION

The figure 31,536,000 appears in roughly 95 places across `docs/`. It is
asserted in two mutually stronger forms, **in the same file**:

| Reading | Evidence | Language |
|---|---|---|
| Lifetime cumulative issuance | `docs/economic_security.md:137` | "Σ (all MBO **ever created**) ≤ 31,536,000", called "UNCONDITIONALLY TRUE for all time" |
| Outstanding supply at time t | `docs/economic_security.md:1205` | "∀t: Σ(all MBO **at time t**) ≤ 31,536,000", under "CLAIM 1: SUPPLY INVARIANT" |

Reinforced on the cumulative side by `docs/supply_schedule.md:771` and
`docs/reward_mechanics.md:566`; on the outstanding side by
`docs/incentive_design.md:1219` and `docs/token_distribution.md:516`. The
remainder are undifferentiated "Total Supply: 31,536,000 MBO" statements.

`docs/monetary_policy.md:33` asserts the cap "is enforced at the protocol
level". Per §2.1 it is not.

Burn permanence **is** documented: `docs/economic_summary.md:276` ("ALL
BURNS PERMANENTLY REDUCE SUPPLY"), `docs/incentive_design.md:1114`,
`docs/economic_security.md:1029` ("No mechanism to recover burned MBO").

Whether a burn reopens **issuance capacity** is documented nowhere. A
repository-wide search for reissue, remint, recycle, reopen or replenish
returns nothing. Recorded as `NOT_SPECIFIED`, and deliberately not filled in
by convention from other chains.

> These documents are the evidence of the ambiguity. This RFC does not
> rewrite them.

### 2.3 CURRENT_RESEARCH_MODEL

`model.MonetaryPolicy.clamp_to_cap` derives headroom as `cap − supply`, so
the model implements an **outstanding-supply cap**. It was introduced by PR
#72 (`c0bffdb`) and was not inherited from anything earlier. The model had
to pick one reading in order to run; that was an implementation necessity,
not a finding, and it is not evidence about intent.

Accounting order per period, audited: starting supply → issuance and clamp →
fees → burn → ending supply → security metrics. The cap is therefore
evaluated against **beginning-of-period supply**, so a burn in year *t*
widens outstanding headroom in year *t+1*, never in year *t*.

### 2.4 EXPERIMENTAL_RESULT

| Result | Source |
|---|---|
| The documented cap and schedule are structurally coupled: `I × S / (1 − F)` = `3,153,600 × 5 / 0.5` = 31,536,000 exactly | PR #75 |
| Holding that cap constant clamps 52 of 80 candidate schedules | PR #75 |
| The documented schedule converges to 31,535,969.92 and never clamps over 100 years | PR #75, PR #77 |
| Under outstanding semantics, cumulative issuance reaches up to 1.117× the cap while outstanding supply stays at or below it | PR #75 |
| Lifetime vs outstanding: 52 of 80 schedules diverge **with** burns | PR #77 |
| Lifetime vs outstanding: 0 of 80 diverge **without** burns | PR #77 |
| The documented schedule is unaffected by the choice of semantics over 100 years | PR #77 |
| Dual collapses onto lifetime when `initial_supply = 0`; a constructed non-zero starting supply separates them | PR #77 |
| A lifetime cap alone does not bound outstanding supply when initial supply is non-zero | PR #77 |

### 2.5 OPEN_POLICY_QUESTION

Everything in §9 (Decision Gates). None of it is answered here.

---

## 3. Policy objectives

Candidate objectives, stated so that a proposal can be argued against them.
Listing an objective is not adopting it; §4 separates what must hold from
what trades off, and even that separation is proposed rather than settled.

### A. Monetary scarcity

Four distinguishable claims, routinely collapsed into "fixed supply":

| Claim | Statement | Measurable as |
|---|---|---|
| Lifetime scarcity | a bound on all MBO ever created | cumulative gross issuance |
| Outstanding scarcity | a bound on MBO existing at any instant | ending supply |
| Predictable dilution | issuance is forecastable without governance input | variance of the issuance path |
| Circulating scarcity | a bound on liquid, unlocked supply | circulating supply |

They are not equivalent once burns or locked allocations exist. A policy may
satisfy one and violate another. `EVIDENCE: EXPERIMENTAL_RESULT` — PR #77
measured lifetime and outstanding diverging on 52 of 80 schedules.

### B. Long-term network security

Once primary issuance becomes small, security must be paid for from
somewhere. Candidate sources, kept separate because they fail
independently:

- primary issuance
- transaction fees
- priority fees
- compute-related protocol revenue
- other protocol revenue

**No source is assumed sufficient.** The current model represents fee and
compute revenue as exogenous assumption paths, not as mechanisms, so no
study in this repository yet constitutes evidence about their adequacy.

### C. Compute economy alignment

Mbongo is compute-native, which makes one confusion especially costly:
paying for useful compute is not the same act as issuing money. Kept
separate:

- payment for useful compute (a user buying a service)
- worker/provider compensation (the supply side being paid)
- validator/security compensation (consensus being paid)
- protocol revenue (what the protocol retains)

**Compute payments are not assumed to be inflationary.** Whether any part of
compute settlement should be funded by issuance rather than by users is an
open gate, not a premise.

### D. Holder dilution

Four measurements, not one:

| Measure | Question |
|---|---|
| Gross issuance dilution | how much new supply is created |
| Net dilution after burns | what the supply does once burns are netted |
| Dilution to non-stakers | what a passive holder experiences |
| Dilution to stakers | what a participating holder experiences |

They can point in opposite directions: a policy can be net-deflationary in
aggregate while diluting non-stakers. The current model reports gross and
net inflation and a dilution pair; staker-versus-non-staker dilution depends
on a staking design that does not exist yet.

### E. Predictability

Whether the supply path can be understood and forecast without modelling
governance behaviour. Measurable as: how many future decisions must be
predicted to compute supply at year *t*.

### F. Governance minimization

Four distinguishable levels of monetary authority:

| Level | Description |
|---|---|
| Immutable constants | changeable only by hard fork |
| Bounded parameters | adjustable within a range fixed in code |
| Governance-adjustable parameters | freely adjustable by process |
| Emergency mechanisms | out-of-band intervention |

Historical documentation asserts strong immutability
(`docs/governance_model.md:99`: governance "cannot increase total supply").
`RUNTIME: NONE` — no governance mechanism constrains monetary parameters
because no monetary parameters exist in the runtime.

### G. Protocol simplicity and auditability

Whether supply and issuance invariants can be verified independently from
chain state, by a party that trusts no one. A cap nobody can check is not a
cap. `docs/supply_schedule.md:771` already proposes a verification method
("Sum all block rewards ≤ 31,536,000"), which presumes the lifetime reading.

### H. Economic sustainability

Whether security spending eventually bears a credible relationship to the
economic activity it protects. **Perpetual subsidy is assumed neither
necessary nor unacceptable.** Both are candidate positions.

---

## 4. Hard invariants versus soft objectives

Proposed membership. **Not settled**, and deliberately recorded as a
proposal so that moving an item between categories is a visible decision
rather than a silent one.

### 4.1 Proposed hard invariants

**H1 — Supply is computable from chain state alone.**
*Rationale:* an unverifiable monetary rule is a claim, not a rule.
*Measurement:* an independent implementation can recompute supply at any
height from blocks alone.
*Failure mode:* holders cannot distinguish an honest chain from one that
minted quietly.
*Evidence status:* `OPEN` — no supply accounting exists in the runtime.

**H2 — No unbounded discretionary issuance.**
*Rationale:* discretionary minting makes every other guarantee conditional
on governance behaviour.
*Measurement:* every issuance path is bounded by constants or by a rule
fixed in code.
*Failure mode:* governance capture translates directly into supply capture.
*Evidence status:* `OPEN`.

**H3 — Accounting identities hold exactly.**
*Rationale:* supply, issuance and burn must reconcile, or every downstream
figure is unfalsifiable.
*Measurement:* `ending_supply = starting_supply + gross_issuance − burned`,
checked per period.
*Evidence status:* `SUPPORTED` in research — enforced by the model and
checked in CI on every run.

**H4 — Whatever a cap is stated to bound, it bounds mechanically.**
*Rationale:* the current gap between "enforced at the protocol level"
(`docs/monetary_policy.md:33`) and zero enforcement is precisely the failure
this invariant forbids.
*Measurement:* the runtime rejects any state transition that would violate
the stated bound.
*Failure mode:* the project ships two incompatible monetary stories.
*Evidence status:* `OPEN`, and currently **violated by the documentation**.

**H5 — Documented monetary claims match implemented behaviour.**
*Rationale:* a documentation-only cap is worse than no cap, because it is
relied upon.
*Measurement:* every normative monetary claim in `docs/` maps to code or is
marked aspirational.
*Evidence status:* `OPEN` — RFC 0003 §3 records the current gap.

Note that H4 and H5 are the only two invariants that today's repository
demonstrably fails, and both are documentation-versus-runtime failures
rather than design failures.

### 4.2 Proposed soft objectives

These trade off against each other; a proposal is expected to argue its
position, not to maximise all of them.

| Objective | Measurement | Principal tension |
|---|---|---|
| Scarcity strength | cumulative issuance bound | against long-term security funding |
| Dilution predictability | variance of the issuance path | against adaptive security funding |
| Long-term security flexibility | ability to fund security after primary emission | against scarcity strength |
| Governance minimization | count and range of adjustable parameters | against adaptivity |
| Auditability | independent verifiability of the invariant | against mechanism complexity |
| Compute-economy compatibility | separation of compute payment from issuance | against bootstrap subsidy |
| Simplicity | number of interacting mechanisms | against every adaptive objective |

The central tension, stated plainly: **scarcity strength and long-term
security flexibility pull against each other, and no evidence in this
repository yet establishes where the balance should sit.**

---

## 5. Cap decision axes

Three candidate readings, evaluated separately. **No selection is made.**
`C` denotes whatever magnitude a cap would take; §9 GATE 1 asks what it
means, and RFC 0003 §15 separately asks what magnitude, if any, is right.

### 5.1 Lifetime cumulative issuance cap

*Guarantees:* a bound on all MBO ever created. Verifiable by summing
issuance across all blocks.

*Does not guarantee:* a bound on outstanding supply when a non-zero genesis
allocation exists — measured in PR #77, where a constructed case produced
outstanding supply above `C` while cumulative issuance stayed at `C`.

*Burn interaction:* burns lower outstanding supply and never reopen issuance
headroom. Cumulative issuance is monotonic.

*Future issuance:* once the bound is reached, primary issuance is
permanently exhausted. Any later security funding must come from fees or
protocol revenue.

*Governance:* one constant. Minimal surface.

*Auditability:* strong — a running sum over block rewards, the method
`docs/supply_schedule.md:771` already describes.

*Security budget:* issuance-funded security terminates permanently at the
bound. `TRADE_OFF` against objective B.

### 5.2 Outstanding supply cap

*Guarantees:* a bound on MBO existing at any instant.

*Does not guarantee:* a bound on total MBO ever created. Measured: cumulative
issuance reached **1.117× the cap** under the controlled fee split while
outstanding supply stayed at or below it (PR #75).

*Burn interaction:* burns lower outstanding supply and therefore **may
reopen issuance headroom**, in the following period given the audited
accounting order. This is the model's current behaviour.

*Future issuance:* issuance can resume after burns, so issuance-funded
security has no permanent terminus while burning continues.

*Governance:* one constant, but the effective issuance path depends on burn
volume, which depends on activity — so the supply path is less forecastable
without modelling activity.

*Auditability:* strong for the stated bound; a holder verifying "supply ≤ C"
learns nothing about how much was ever created.

*Security budget:* couples security funding to activity through burns.
`TRADE_OFF` against objective E.

### 5.3 Dual cap

*Guarantees:* both bounds simultaneously; issuance is constrained by the
tighter headroom.

*Does not guarantee:* anything additional beyond the two.

*Burn interaction:* burns may create outstanding headroom but never restore
lifetime headroom.

*Relationship to the other two:* the two constraints are related exactly by

```
outstanding_headroom − lifetime_headroom = cumulative_burn − initial_supply
```

With `initial_supply = 0` the lifetime bound always binds, so dual collapses
onto lifetime — which is what PR #77 measured across all 80 schedules. Given
a non-zero genesis allocation they separate, and dual becomes strictly
tighter than lifetime. **Dual and lifetime are therefore not the same
policy; they coincide under this scenario set's starting conditions only.**

*Governance:* one constant, two checks.

*Auditability:* strong for both bounds.

*Security budget:* the most restrictive of the three on issuance-funded
security.

### 5.4 What the evidence does and does not settle

| Statement | Status |
|---|---|
| The readings diverge on 52 of 80 candidate schedules with burns | `EXPERIMENTAL_RESULT` |
| The readings diverge on 0 of 80 without burns | `EXPERIMENTAL_RESULT` |
| The documented schedule is unaffected over 100 years | `EXPERIMENTAL_RESULT` |
| Dual equals lifetime universally | **FALSE** — contingent on `initial_supply = 0` |
| Which reading Mbongo should adopt | `OPEN_POLICY_QUESTION` — GATE 1 |

Divergence being burn-driven means the cap question and the burn question
are not independent. They are treated as separate axes below because they
can be *decided* separately, not because they interact weakly.

---

## 6. Burn decision axes

Burn is an independent policy dimension, and the documentation answers only
half of it.

| Question | Documentation |
|---|---|
| Are burned tokens permanently destroyed? | **Answered: yes**, explicitly and repeatedly |
| Can burned supply be re-created later? | **Not specified** |
| Does burning create new issuance capacity? | **Not specified** |

The first answer does not imply the other two. "The burned tokens are gone
forever" and "the protocol may issue new tokens because supply fell" are
compatible statements: the original tokens are never recovered, while the
schedule is permitted to issue different ones. Two coherent positions
follow, and the repository endorses neither:

**Position 1 — burn is terminal for issuance.** Destruction reduces supply
and leaves the issuance bound untouched. Cumulative issuance is monotonic
and bounded. Consistent with the lifetime reading.

**Position 2 — burn reopens headroom.** Destruction reduces supply, and the
bound being on supply, capacity to issue returns. Consistent with the
outstanding reading, and what the research model implements today.

One documented statement is worth recording as evidence bearing on this,
without treating it as an answer: `docs/economic_security.md:1032` states
that circulating supply "will be LESS than 31,536,000 MBO. Potentially
significantly less if network is heavily used." Under Position 2, heavier
use means more burn, more headroom and more issuance, which pushes supply
back toward the cap rather than away from it. The documented expectation
reads more naturally under Position 1 — **but it is an expectation stated in
prose, not a rule, and it is not decisive.** GATE 2 remains open.

---

## 7. Security budget framework

Define `SECURITY_BUDGET(t)` as the economic resources available at time *t*
to compensate the parties that secure the network.

```
SECURITY_BUDGET(t) = issuance_funded(t)
                   + fee_funded(t)
                   + protocol_revenue_funded(t)
```

Kept separate because they fail independently: issuance funding is a policy
choice that can be legislated to zero; fee funding depends on demand;
protocol revenue depends on the compute market existing.

**Token issuance is not security.** Issuance is a transfer to validators
denominated in a token whose value is itself endogenous. A schedule that
issues more MBO does not necessarily buy more security.

**Validator revenue is not total network economic security either.** It is
one input. Security also depends on stake distribution, cost of attack
relative to value secured, validator operating costs, and concentration —
none of which the current model represents.

### Limitations, stated rather than buried

- Fee and compute revenue are **exogenous assumption paths** in the current
  model, not mechanisms. Every fee-funded figure produced so far is a
  restatement of an assumption.
- There is no staking design, so staked supply is a scenario parameter and
  staker-specific dilution cannot be computed.
- Token price is an arbitrary reference unit. Reference-value figures are
  ordinal at best.
- No validator cost model exists, so "is this budget adequate" has no
  denominator.
- No attack-cost model exists.

Consequently **no figure in this repository currently establishes that any
policy provides sufficient security.** Sufficiency is GATE 3 and GATE 4.

---

## 8. Security sufficiency, compute revenue, phases and failure modes

### 8.1 How much security budget is enough

No MBO or currency threshold is invented here. The question becomes tractable
only relative to measurable reference quantities:

| Reference quantity | Available today? |
|---|---|
| Value secured by the network | `NO` — no such measurement exists |
| Economic value of transactions | `NO` — modelled as an assumption path |
| Compute settlement value | `NO` — assumption path |
| Stake value | `NO` — no staking mechanism |
| Cost of attack | `NO` — no threat model |
| Validator operating cost | `NO` — no cost model |
| Concentration risk | `NO` — no distribution model |

Seven reference quantities; the repository supplies none of them as
measurements. That is the honest state of the evidence, and it is why §10
proposes mechanism work before more trajectory studies.

### 8.2 Compute revenue

The flow to analyse later, with **no percentages assigned**:

```
user
  → compute payment
      → worker/provider compensation
      → protocol fee
          → security/validator allocation
          → treasury and/or burn
```

Historical documentation proposes specific splits, including a 70/20/10
allocation. Any such figure is `HISTORICAL_DOCUMENTATION`, not a decided
policy, and is not revived as one here.

The structural question is whether compute settlement should touch monetary
issuance at all, or remain entirely a user-funded transfer with a protocol
cut. Both are candidate positions. GATE 5.

### 8.3 Maturity phases

One mechanism can behave differently across network stages. Analytical
phases, **with no dates and no thresholds fixed**:

| Phase | Conditions | Security funding |
|---|---|---|
| Bootstrap | low fees, low compute revenue | may depend heavily on issuance |
| Growth | fees and compute activity increasing | issuance subsidy declining in relative terms |
| Mature | activity is the larger economic flow | activity should carry a larger share **if feasible** |

**Mature issuance is not assumed to be zero.** Whether it should be is GATE
6. Conflating bootstrap and mature economics is a named review failure
(§11.11).

### 8.4 Failure modes

Stress hypotheses to be tested. **None is claimed to occur today**; the
runtime implements no monetary policy, so none can.

| Failure mode | Sketch |
|---|---|
| Security-budget collapse | issuance ends, fees never materialise |
| Excessive dilution | issuance outruns demand |
| Fee shock | fee revenue drops sharply after issuance has decayed |
| Compute-demand collapse | protocol revenue disappears |
| Token-price collapse | budget adequate in MBO, inadequate in value |
| Stake concentration | budget adequate in aggregate, concentrated in practice |
| Validator exit | revenue falls below operating cost |
| Governance capture | adjustable parameters captured |
| Unexpected deflation | burns outrun issuance, harming usability |
| Burn-induced security starvation | under lifetime semantics, burns shrink supply without restoring issuance |
| Unbounded issuance | an adaptive rule fails to converge |
| Hard-cap security starvation | the cap binds before fee revenue is sufficient |

The last two are the endpoints of the central tension in §4.2.

---

## 9. Decision matrix

Candidate families against the axes. **No overall score. No ranking. No
preferred policy.** Cells state what is supported by evidence in this
repository, not what is desirable.

Legend: `SUPPORTED` · `PARTIAL` (partially supported) · `UNKNOWN` ·
`TRADE_OFF`.

### 9.1 Structural properties

| Candidate | Scarcity guarantee | Dilution predictability | Burn interaction | Long-term security flexibility |
|---|---|---|---|---|
| Historical documented schedule | `PARTIAL` — converges to 31,535,969.92, never binds | `SUPPORTED` — fully forecastable | Unaffected by semantics over 100y (measured) | `TRADE_OFF` — emission decays to ~0 |
| Hard lifetime cap | `SUPPORTED` for cumulative; `UNKNOWN` for outstanding with non-zero genesis | `SUPPORTED` | Burns never reopen headroom | `TRADE_OFF` — permanent terminus |
| Hard outstanding cap | `SUPPORTED` for outstanding; **not** for cumulative (1.117× measured) | `PARTIAL` — depends on burn volume | Burns may reopen headroom | `PARTIAL` — issuance can resume |
| Dual cap | `SUPPORTED` for both | `SUPPORTED` | Outstanding headroom only | `TRADE_OFF` — most restrictive |
| Fixed tail issuance | `UNKNOWN` — unbounded cumulative by construction | `SUPPORTED` | Independent | `SUPPORTED` |
| Percentage tail issuance | `UNKNOWN` — unbounded cumulative | `PARTIAL` — proportional to supply | Burns shrink the base | `SUPPORTED` |
| Adaptive security-budget issuance | `UNKNOWN` | `UNKNOWN` — depends on activity | Indirect | `SUPPORTED` by construction |

### 9.2 Governance, auditability and evidence

| Candidate | Governance discretion | Auditability | Compute-economy compatibility | Known failure modes | Evidence strength | Further simulation required |
|---|---|---|---|---|---|---|
| Historical documented schedule | Minimal | `SUPPORTED` | `UNKNOWN` | hard-cap security starvation | Sensitivity + semantics measured | Security-budget stress |
| Hard lifetime cap | Minimal | `SUPPORTED` — running sum | `UNKNOWN` | hard-cap starvation; burn-induced starvation | Semantics measured (#77) | Security-budget stress |
| Hard outstanding cap | Minimal | `PARTIAL` — bound verifiable, creation history not | `UNKNOWN` | unexpected coupling to activity | Semantics measured (#77) | Fee-revenue sensitivity |
| Dual cap | Minimal | `SUPPORTED` | `UNKNOWN` | strictest starvation risk | Measured, incl. separation case | Security-budget stress |
| Fixed tail issuance | Low — one constant | `SUPPORTED` | `UNKNOWN` | excessive dilution | Family exists in model; not compared at equal security | Tail comparison at equal security target |
| Percentage tail issuance | Low | `SUPPORTED` | `UNKNOWN` | excessive dilution; deflation interaction | Family exists; not compared | Tail comparison at equal security target |
| Adaptive security-budget issuance | `TRADE_OFF` — bounded parameters | `PARTIAL` — depends on inputs | `UNKNOWN` | unbounded issuance; governance capture | Family exists; adaptivity untested | Validator-cost floor; stake concentration |

Every row carries `UNKNOWN` for compute-economy compatibility. That is not
an oversight: compute revenue is an assumption path, so the repository has
no evidence to place in that column for any candidate.

---

## 10. Decision gates

These are the questions that must be answered before any MBO monetary policy
can become normative. **None is answered here, and none may be answered by
assumption.**

These layer on top of RFC 0003 §16, which lists six *prerequisite* gates
about mechanisms that must exist (staking design, fee model, security
requirement, genesis distribution, simulation against real mechanisms,
documentation gap resolved). Those remain open and are not restated. The
gates below are the *decisions* that become answerable once they are met.

**GATE 1 — What does 31,536,000 mean?**
Lifetime cumulative issuance, outstanding supply, both, or nothing binding?
`HISTORICAL_CAP_SEMANTICS = AMBIGUOUS`. Blocks: every cap row in §9.

**GATE 2 — Can burns reopen issuance capacity?**
`NOT_SPECIFIED` in documentation. The model says yes; that is an
implementation choice, not evidence. Blocks: GATE 1, and all burn-dependent
security projections.

**GATE 3 — What minimum long-term security properties must issuance
support?**
Requires a threat model and a stated security requirement. Depends on RFC
0003 §16 gate 3. Blocks: every `TRADE_OFF` cell in §9.1.

**GATE 4 — What fraction of security can credibly come from fees?**
Currently unanswerable: fee revenue is an assumption path. Depends on RFC
0003 §16 gate 2. Blocks: GATE 6.

**GATE 5 — What role can compute protocol revenue play?**
Requires the compute market to be a modelled mechanism. Blocks: the
compute-economy column in §9.2, which is `UNKNOWN` for every candidate.

**GATE 6 — Is perpetual issuance acceptable if bounded?**
A values question informed by GATE 3 and GATE 4, not derivable from
simulation alone. Blocks: both tail families and the adaptive family.

**GATE 7 — How much monetary discretion can governance possess?**
Which parameters are immutable, bounded, adjustable, or emergency-only.
Blocks: the adaptive family, and proposed invariant H2.

**GATE 8 — Which invariants must the runtime enforce mechanically?**
The gap between a documented cap and an enforced one is currently total.
Blocks: proposed invariants H1, H4 and H5.

Dependencies, so the ordering is not mistaken for a priority list:

```
GATE 2 ──> GATE 1 ──> GATE 8
GATE 3 ──> GATE 6 <── GATE 4
GATE 5 ──> GATE 4
GATE 7 ──> GATE 8
```

GATE 2 is upstream of the cap question, because what a cap bounds cannot be
settled while it is undecided whether burns relax it.

---

## 11. Required future experiments

Only experiments justified by a gate. Simulation proliferation is itself a
failure mode: more trajectories against assumed inputs would add volume, not
evidence.

| Experiment | Gate | Prerequisite | Justification |
|---|---|---|---|
| Security-budget stress study | GATE 3 | a stated security requirement | The only way to distinguish "issuance ends" from "security ends" |
| Fee-revenue sensitivity | GATE 4 | a fee model (RFC 0003 §16 gate 2) | Fee revenue is currently an assumption; sensitivity over an assumption measures the assumption |
| Compute-revenue sensitivity | GATE 5 | a compute-market mechanism | Same objection; the compute column is `UNKNOWN` for every candidate |
| Token-price shock | GATE 3 | reference-value interpretation | Budgets adequate in MBO may be inadequate in value |
| Validator-cost floor | GATE 3 | a validator cost model | Supplies the missing denominator for adequacy |
| Stake concentration | GATE 3 | a staking design (RFC 0003 §16 gate 1) | Aggregate budget can be adequate while distribution is not |
| Tail-emission comparison at equal security target | GATE 6 | GATE 3 answered | Comparing tails without a common target compares schedules, not policies |

**Deliberately excluded:** further cap-semantics variants (GATE 1 is blocked
on GATE 2, not on more data), more schedule grids (PR #75 already covers the
space), and any study whose inputs are assumption paths standing in for
mechanisms that do not exist. Six of the seven experiments above are blocked
on a mechanism rather than on compute time, which is the finding this
section exists to record.

---

## 12. What this RFC concludes

That the decision is not yet takeable, and precisely why.

- The runtime enforces no monetary rule, so nothing is being changed by
  deferring.
- The documentation asserts two incompatible readings of its central figure
  and settles neither.
- The research model implements one reading out of necessity, and that
  choice is not evidence.
- The measured divergence between readings is entirely burn-driven, which
  makes GATE 2 upstream of GATE 1.
- Every candidate's compute-economy compatibility is `UNKNOWN`, because
  compute revenue is an assumption rather than a mechanism.
- Six of seven required experiments are blocked on missing mechanisms, not
  on missing computation.

```
MBO_CAP_POLICY_DECISION=NOT_MADE
MBO_ISSUANCE_POLICY_DECISION=NOT_MADE
MBO_BURN_REISSUANCE_DECISION=NOT_MADE
MBO_SECURITY_BUDGET_DECISION=NOT_MADE
RUNTIME_CAP_SEMANTICS=NONE
HISTORICAL_CAP_SEMANTICS=AMBIGUOUS
```

---

## 13. Audit addendum, 2026-11-07

A read-only audit of the monetary research, recorded here rather than in a new
document. **It selects no policy.** The gates of §10, their dependency order, and
the decision record of §12 are unchanged.

### 13.1 Runtime facts, re-checked at `dev` `fe6e5e4`

The search of §2.1 still finds no issuance, burn, block reward, supply cap,
total supply, inflation, treasury, vesting or halving in `crates/`. Three
details changed or were not recorded:

- `ensure_genesis` is now at `crates/mbongo-node/src/backend.rs:286`, not `:143`.
- `TransactionType::Stake` follows the transfer path: it moves balance to its
  receiver, and records no stake.
- The producer's block time defaults to **5 seconds** (`--block-time`,
  `crates/mbongo-node/src/main.rs:123`).

### 13.2 Inconsistencies in the historical documents

These are recorded, not resolved. Each document now carries a HISTORICAL,
NON-NORMATIVE banner, and none of its text was changed.

**Line references.** The banner adds 13 lines under each title (15 in
`tokenomics.md`). Every `file:line` reference into these documents, in §2.2
above, in RFC 0003 and in the research code, points to the text as it stood
before the banner, at `fe6e5e4`. At later revisions, add the banner's length.
All 12 distinct references were checked to land on the same text at that
offset.

| # | Inconsistency | Where |
|---|---|---|
| A1 | **Allocation versus emission.** The emission schedule alone, 0.1 MBO per block halving every 5 years at 31,536,000 blocks a year, issues the whole 31,536,000 (§2.4: `I × S / (1 − F)`), and the supply documents say all MBO comes from block rewards, with a genesis supply of 0. The allocation tables put 40% outside emission (ecosystem 15%, foundation 10%, community 10%, early contributors 5%) and only 60% in per-block emission. Both cannot hold: together they reach 140% of the figure | `supply_schedule.md`, `monetary_policy.md` against `token_distribution.md`, `tokenomics.md`, `economic_summary.md` |
| A2 | **PoS and PoUW splits.** Emission is split 50/50 between PoS and PoUW; the allocation gives them 40% and 20%, a 2:1 ratio | `tokenomics.md`, `economic_summary.md` against `token_distribution.md` |
| A3 | **Block-time dependency.** The schedule is per block, and assumes 1-second blocks. At the runtime's default of 5 seconds, the same per-block reward issues one fifth as much per year, and one fifth of the total. Annual issuance therefore depends on a parameter that nothing fixes | `tokenomics.md`, `supply_schedule.md`, `monetary_policy.md`; `main.rs:123` |
| A4 | `tokenomics.md` is marked **Status: ACTIVE**, and says fee transfer and rewards are settled on-chain from v0.4. Protocol v0.4 has neither | `tokenomics.md` |
| A5 | A 10% "Contributor Treasury" does not match any row of the allocation tables (foundation 10%, community 10%, early contributors 5%) | `contributor_compensation.md`, marked **Status: Active** |
| A6 | Nineteen documents carry `<!-- Verified against tokenomics.md -->`, written when each was created on 2025-11-26, while `tokenomics.md` first appears in Git on 2026-02-17. The comment is not evidence of a check, and A1 shows that none held | 13 of the 14 bannered documents, and 6 others |
| A7 | The figure is presented as a fact in documents outside the economic set: `vision.md`, `faq.md`, `glossary.md` and `onboarding_dev.md` (each marked Canonical), `mbongo_whitepaper.md`, `oracle_model.md`, `competitive_analysis.md`, `governance_model.md` (already bannered as aspirational). They are not bannered here. `INDEX.md` now states that no such figure anywhere in `docs/` is implemented or adopted | as listed |

A1 bears on RFC 0003 §16 gate 4, the genesis distribution. A3 adds a question
§10 does not ask: whether issuance is defined per block or per unit of time.

### 13.3 Compatibility with VISION_v2

VISION_v2 (adopted 2026-11-03) postdates this RFC.

| Historical element | Under VISION_v2 |
|---|---|
| PoUW block rewards, 50% of emission; the 20% PoUW allocation; the PoUW half held "reserved, not emitted" until v2 | **Obsolete.** P1 withdraws execution by validators, even optional. The reserved half has no recipient. This addendum does not replace it with another figure |
| PoUW score in leader selection and chain weight (`architecture_master_overview.md`) | **Obsolete.** It would make consensus judge compute work (P1, P3) |
| Compute providers as PoUW earners | **Invalid role.** Providers are off-chain workers paid by their clients |
| Validators paid a verification fee for deterministic receipt checks | Compatible (P3) |
| Executors paid outside the protocol | Compatible, and the current state |
| A protocol fee on compute payments (RFC 0003 §7) | Feasible later, only after the settlement RFC (VISION_v2 §7, step 11). In the model it remains an assumption |
| Issuance for security, transaction fees, burn, treasury, staking | Feasible, each through its own protocol RFC. There is no validator set today: RFC 0006 proposes one authority |
| Separating compute revenue from issuance (RFC 0003 §7) | Compatible |

### 13.4 Research model: fee funding

The model paid fees out of a supply of zero. Fee revenue is an exogenous MBO path,
so in `high_adoption` supply reached zero in year 52, and validator and treasury
fee revenue were still reported for the 49 years that followed, while every
accounting identity passed. PR #239 classifies each year as `feasible`,
`velocity_dependent` or `infeasible` against all the MBO that exists in it,
changes no existing figure, and rejects infeasible runs with their own exit
status.

Every result of §2.4 holds on feasible runs: the coupling of cap and schedule,
the 52 of 80 clamped schedules, the historical schedule's convergence without
clamping, the 1.117× maximum, and the 52 of 80 divergence with burns, none of
which involves an infeasible run. Two published range ends need qualification,
because they come from infeasible runs: the lowest ending supply at 100 years,
0 (457,578 MBO over feasible runs), and the lowest cumulative issuance over cap,
0.067× (0.133×). Any figure from `high_adoption` after year 51 is not valid.

### 13.5 MVP v0.2

No monetary decision is on the MVP v0.2 critical path. RFC 0006, if accepted,
lets only genesis create balances and keeps every block supply-neutral; it adds
no issuance, burn, fee or reward, and leaves this RFC and RFC 0003 non-normative.
The test-network unit convention of S2 is not a denomination. Test genesis
allocations are not a distribution.

### 13.6 Questions this audit adds, outside the gates

Not gates, and not ordered against them. They are recorded so that they are not
answered by default:

- **Denomination:** the unit's name and its number of decimal places. The runtime
  holds integer balances, and nothing converts them.
- **Issuance unit:** per block or per unit of time (A3).
- **Genesis distribution:** which of A1's two readings, if either, survives. This
  is RFC 0003 §16 gate 4.
- **The PoUW share:** what happens to the emission and allocation it held (§13.3).

```
MBO_CAP_POLICY_DECISION=NOT_MADE
MBO_ISSUANCE_POLICY_DECISION=NOT_MADE
MBO_BURN_REISSUANCE_DECISION=NOT_MADE
MBO_SECURITY_BUDGET_DECISION=NOT_MADE
MBO_DENOMINATION_DECISION=NOT_MADE
MBO_GENESIS_DISTRIBUTION_DECISION=NOT_MADE
RUNTIME_CAP_SEMANTICS=NONE
HISTORICAL_CAP_SEMANTICS=AMBIGUOUS
```
