# MBO Monetary Policy Simulation

A deterministic, long-horizon model for comparing candidate MBO monetary
policies. It accompanies
[RFC 0003](../../docs/rfcs/0003-mbo-monetary-policy-security-budget.md).

## Why this exists

Mbongo Chain has to fund network security over decades. The repository today
documents one emission plan and implements none of it (see the baseline table
in the RFC). Before choosing a policy, we need to be able to compare
candidates on identical assumptions and see where each one leads.

Arguments about monetary policy tend to fail because each participant carries
different unstated assumptions about adoption, fees, burn and staking. This
model makes those assumptions explicit, configurable and reviewable, so a
disagreement becomes a disagreement about a specific number in
`scenarios.json` rather than about vibes.

## What this is not

**This model does not predict the market price of MBO.** Price is an exogenous
input that a scenario author chooses. Every price path in `scenarios.json` is
an assumption being tested, never a forecast.

It is also not:

- a policy recommendation — no scenario here is normative;
- a protocol specification — nothing here runs on or reads from the chain;
- a cost-of-attack calculator — see *Security metrics* below;
- a claim that any modelled adoption path will happen.

## Current model scope

The engine can parameterise several monetary-policy families, but **the
scenario set in this PR deliberately does not explore the complete MBO
monetary-policy design space.**

Every scenario shipped here reuses the same primary issuance schedule —
`initial_annual_issuance 3153600`, `step_years 5`, `step_factor 0.5` — taken
from the schedule documented in `docs/supply_schedule.md` (0.1 MBO per block,
halving every five years). The set therefore compares what happens **after**
that schedule: hard-cap termination, fixed tail, percentage tail, an
experimental adaptive tail, and adoption/stress assumptions.

**`base` is not a neutral monetary baseline.** It is an illustrative benchmark
anchored on the documented historical schedule; it varies fees, allocation,
staking and adoption, not the issuance schedule itself.

Future economic work must independently vary: initial issuance magnitude,
primary issuance duration, decay interval, decay factor, issuance curve shape,
hard cap, tail policy, fee structure, burn policy, and security allocation.
This PR assigns no values to any of those.

## Terminology

Three quantities are routinely confused. The model keeps them separate and so
should any document quoting it.

| Term | Meaning |
|---|---|
| **Cumulative issuance** | Total MBO created by protocol issuance over the run |
| **Cumulative issuance cap** | Upper bound on cumulative issuance in a hard-cap model (`monetary.cap`) |
| **Ending supply** | Issued MBO still outstanding after modelled burns |

**A cumulative issuance cap is not an ending supply.** A scenario with a
31,536,000 cumulative issuance cap can end far below 31,536,000 once burns are
applied — `historical_documented_partial` ends near 2.5M MBO for exactly that
reason, because it burns its entire fee pool.

A further property of the documented schedule, as reconstructed here: because
the halving series is geometric, cumulative issuance **approaches the cap
asymptotically without reaching it**. A 100-year run yields 31,535,969.92 MBO,
not 31,536,000. That is a property of the schedule as modelled, not a protocol
guarantee.

## Running it

Python 3.8+ with the standard library. No third-party packages.

```bash
# list the available scenarios
python3 research/monetary-policy/model.py --list

# run one scenario over a century
python3 research/monetary-policy/model.py --scenario base --years 100

# export one row per year
python3 research/monetary-policy/model.py --scenario base --years 100 \
    --output /tmp/base.csv
```

Exit codes, the first that applies: `1` an accounting identity was violated;
`2` the configuration was rejected; `3` fees are infeasible in a covered year;
`4` the horizon goes beyond what the scenario's inputs define; `0` otherwise.
See [Horizon validity](#horizon-validity) for the order.

## Tests

```bash
python3 research/monetary-policy/test_model.py
python3 research/monetary-policy/test_fee_funding.py
python3 research/monetary-policy/test_horizon.py
python3 research/monetary-policy/test_scenario_validation.py
```

The tests exercise the arithmetic — accounting identities, policy-family
behaviour, shock windowing and input validation — not merely that the files
load.

## Determinism

The same model version, scenario and horizon always produce byte-identical
CSV output. There is no randomness and no dependence on wall-clock time. If a
future version needs Monte Carlo, that will be a separate change with an
explicit seed.

## Model shape

Year 0 records the starting state and issues nothing, so a run over N years
produces N+1 rows. Each year:

1. the policy family computes gross issuance, clamped to any configured cap;
2. compute spend splits into worker revenue and a protocol fee;
3. transaction fees plus that protocol fee form one fee pool, split exactly
   once between validators, treasury and burn;
4. supply moves by `gross_issuance - burned`;
5. the security budget is validator issuance revenue plus validator fee
   revenue, reported in MBO and in the scenario's reference unit.

### Accounting identities

Checked by `identity_violations()` and by the tests, on every scenario:

- `ending_supply == starting_supply + gross_issuance - burned`
- `worker_revenue + protocol_compute_fee == compute_user_spend`
- `validator_fee_revenue + treasury_revenue + burned == fee pool`
- `security_budget_mbo == validator_issuance_revenue + validator_fee_revenue`
- `staked_supply <= circulating_supply <= ending_supply`
- each year's `starting_supply` equals the previous year's `ending_supply`
- every value is finite

Burn is subtracted in exactly one place, and the compute split has exactly two
destinations, so nothing is double-counted.

**Correction, 2026-11-07.** The third identity is not checked by
`identity_violations()`, and it does not hold in every year. When the burn a
fee pool requires exceeds the MBO that exists, the model caps `burned` at
that amount and keeps the validator and treasury shares of the full pool. The
identity then holds with `burn_requested` in place of `burned`, and fails with
`burned` in every year where the burn is capped. Each such year is flagged
infeasible below.

### Fee funding feasibility

The identities say the books balance. They do not say that the fees a
scenario assumes could have been paid. Fee revenue is an exogenous path in
MBO, unrelated to how much MBO exists, so a scenario can balance its books
while paying fees out of a supply of zero. `high_adoption` did, with no
signal, until 2026-11-07.

Each year is now classified, against the most generous bound: all the MBO that
exists during the year, `available_supply = starting_supply + gross_issuance`.

| `fee_funding` | Condition | Treatment |
|---|---|---|
| `feasible` | Fee pool within `available_supply` | Valid |
| `velocity_dependent` | Fee pool above `available_supply`, its burn within it | Flagged. Possible only if MBO changes hands more than once in the year, and the model has no velocity |
| `infeasible` | Required burn above `available_supply`, or fees with no MBO at all | **Not valid.** The burn is capped, and the fee, treasury and security-budget figures of the year could not have been paid |

The classification rests on these assumptions, and on nothing else:

1. **Only MBO that exists can be destroyed.** A year's burn cannot exceed
   `available_supply`. This is the only hard bound, and the only thing that
   makes a year with MBO in it `infeasible`.
2. **Unburned fees are transfers, not destruction.** Validators and the
   treasury can spend what they receive again in the same year, so the same
   MBO can pay fees more than once. A fee pool larger than the supply is
   therefore not impossible: it needs recirculation, at a velocity the model
   does not represent, and is flagged `velocity_dependent`, never rejected.
3. **Transaction volume is never compared with supply.** Transfers can move
   many times the supply in a year. Only fees enter the classification, and
   only their burned part is held to the stock.
4. **With no MBO at all, nothing can be paid even once,** so fees due in such
   a year are `infeasible`.
5. **The bound is the most generous one:** all MBO that exists in the year,
   staked and illiquid included, and the year's issuance counted from its
   start. A bound that took liquidity into account could only flag more
   years, never fewer.

Nothing is clamped or recomputed to make a year feasible. The CSV gains
`available_supply`, `fee_pool`, `burn_requested`, `burn_shortfall` and
`fee_funding` after the existing columns, which are unchanged. `model.py`
exits with status 3 when any year is infeasible, after printing and writing
the results for inspection.

A run takes the status of its worst year. **A `velocity_dependent` run is
conditional, never shown feasible:** `model.py` exits 0 for it, so that it can
be inspected, and its summary marks it CONDITIONAL. Both studies carry the
run's status and first infeasible year on every observation, report
infeasible and conditional runs, and, beside the original ranges, give the
same ranges over fully feasible runs only. Conditional runs are excluded from
those, as infeasible ones are.

At this commit: `high_adoption` is infeasible from year 52 to 100 (49 years),
and velocity-dependent from 48 to 51. The other 10 scenarios are feasible in
every year. In the sensitivity study, 10 of 184 runs are infeasible, all on
the two smallest, fastest-decaying schedules (788,400 / 2 / 0.25 and 0.50). In
the cap semantics study, 6 of 480, all with burn.

### Horizon validity

The simulator accepts any horizon. Until 2026-11-07 it also ran any horizon
without a signal, although every input path of every shipped scenario ends in
year 100. Beyond its last phase a path kept the last phase's growth rate, and a
shock window ending in year 100 implied full recovery in year 101. So
`historical_documented_partial` was reported infeasible from year 104, purely
because the engine kept growing its fees by 2% a year after the scenario
stopped saying anything.

Each year now carries a `horizon_status`:

| `horizon_status` | Condition |
|---|---|
| `covered` | Every input trajectory is defined in that year by the scenario |
| `extrapolated` | Some input is continued implicitly there. `extrapolated_inputs` names which |

An input is defined through:

- **a path** (`transaction_fee_revenue_mbo`, `compute_user_spend_mbo`,
  `price`): the last `until_year` of its phases, or year 0 alone if it has
  none, unless it declares a continuation:
  - `"continuation": {"policy": "constant"}` holds the last value;
  - `"continuation": {"policy": "growth", "annual_growth": g}` grows at the
    stated rate, which the engine never supplies;
  - either may carry `"until_year": N`, and then covers through `N` only.
- **`staking_ratio`**, a single number: every year, since it is time-invariant
  by definition. Only its shocks limit it.
- **a shock** limits its metric:
  - `"end_year": null` is a persistent shock: it applies to the end of the
    run, with no limit;
  - `"after_end": "revert"` declares full recovery the year after `end_year`,
    with no limit;
  - `"after_end": "unspecified"` states nothing after `end_year`, so coverage
    stops there;
  - with no declaration, a window that ends before the last phase of its
    metric returns to that explicitly defined path, which is the scenario's
    own statement. A window ending at or after it is defined through
    `end_year` only. For `staking_ratio`, the last phase is the last year
    every path of the scenario defines explicitly.

Coverage ends once per input, so covered years are always a prefix of the run.
Extrapolated years are computed exactly as before and written, for
inspection, but they are not a forecast and not a valid result. No
continuation and no shock declaration was added to `scenarios.json`: every
shipped scenario is covered through year 100 and no further.

A run's validity combines three separate checks, each still inspectable on its
own (`model.run_validity`). The first that applies decides:

| Status | Condition | Exit |
|---|---|---|
| `accounting_invalid` | An identity fails, in any year. Extrapolation never excuses arithmetic | 1 |
| `infeasible` | Fees are infeasible in a **covered** year. Shown on the scenario's own inputs, so extrapolation cannot hide it | 3 |
| `extrapolated` | Some year is not covered, including when infeasibility appears only in extrapolated years | 4 |
| `conditional` | Covered years need velocity | 0 |
| `valid` | None of the above | 0 |

Fee funding is reported both over covered years and over all years, so an
infeasibility created by extrapolation is visible without being reported as
shown.

Both studies take `--horizon 100|150|200`, defaulting to 100, the historical
study. Checkpoints are the historical ones, plus 150 and 200 where reached.
Every observation carries `horizon_status`, the run's `first_extrapolated_year`
and `extrapolated_inputs`, and its `run_validity`. **A run with any
extrapolated year is excluded from every range and every divergence count, at
every checkpoint, and reported as excluded.** When nothing is left, the range
is withheld, not printed. A study exits 4 if any run was extrapolated, and
otherwise 0, as before: a sweep is expected to contain infeasible runs, and
flags them.

With the shipped scenarios, a 150- or 200-year study therefore has no
comparable run: 184 of 184 and 480 of 480 are extrapolated from year 101. That
is the intended result. A longer horizon becomes comparable only once a
scenario states what happens after year 100, which is a research decision this
code does not make.

Output changes: the model CSV gains `horizon_status` and `extrapolated_inputs`,
each study CSV `horizon_status`, `first_extrapolated_year`,
`extrapolated_inputs` and `run_validity`, all after the existing columns, which
are byte-identical for every existing run. Each summary gains a coverage line.
A run over more than 100 years that used to exit 0 now exits 4, and
`historical_documented_partial` over 150 or 200 years exits 4 instead of 3.

### Scenario validation

The loader reads keys with defaults, so until 2026-11-08 a misspelled key was
ignored and the run still reported valid (#243): a misspelled shock metric
disabled the shock, a misspelled `annual_growth` became growth 0, and a shock
with no `end_year` key became persistent, a regression from the horizon work.

Now, before anything is simulated:

- unknown keys are rejected at the scenario level, in `activity`, in each
  path, each phase and each shock;
- a shock metric must be one of the four shockable metrics;
- every shock must carry `end_year`: an integer, or `null` to declare it
  persistent;
- every shock is validated, whatever its metric.

A rejected configuration exits 2 from `model.py` and from both studies. It
prints no result and writes no CSV, so it is never classified valid,
conditional, infeasible or extrapolated. An **absent** path is not an error:
it states nothing beyond year 0, and is reported as extrapolated.

Only the sections that hold trajectories are checked for unknown keys.
`monetary`, `fees`, `supply_distribution` and `issuance_allocation` are not
yet.

### Security metrics

`security_budget_mbo` is what the model pays security providers in a year.
`security_value_at_risk` is `staked_supply × mbo_reference_price` — a
deliberately conservative, deliberately named quantity.

It is **not** a cost of attack. The chain has no staking or slashing
implementation today, so no rigorous attack cost can be derived from this
repository. Treat the metric as "reference value of stake in the modelled
world", nothing more.

## Policy families

| Family | Behaviour after the primary emission phase |
|---|---|
| `hard_cap` | nothing more is issued; the cumulative issuance cap is never exceeded |
| `fixed_tail` | a constant number of MBO per year |
| `percentage_tail` | a constant fraction of current supply per year |
| `adaptive_bounded` | issuance moves inside an explicit `[min_rate, max_rate]` band |

`adaptive_bounded` is **research only**, and its current implementation is
narrower than the name suggests. It is a **two-state (bang-bang) controller**:
below `target_security_ratio` it issues at `max_rate`, otherwise at `min_rate`.
It never produces an intermediate rate.

`target_security_ratio` is a free illustrative parameter. It is **not derived
from any Mbongo threat or security model**, because none exists yet. And
because the ratio compares two reference-value quantities, the MBO reference
price cancels between numerator and denominator: **the controller is
price-invariant by construction** and does not react to a price shock at all.
A test asserts this, so the property cannot change silently.

This scenario **must not be read as a proposed Mbongo adaptive monetary
policy.** It exists only to exercise the framework.

The primary emission phase itself is a stepped schedule: an initial annual
amount multiplied by `step_factor` every `step_years`. With factor `0.5` that
is a halving schedule.

## Primary-issuance sensitivity study

`primary_sensitivity.py` answers one question: **how do long-run supply,
dilution and security-budget outcomes change when the primary issuance
schedule changes, while every non-issuance assumption is held constant?**

It is sensitivity analysis, **not optimization**. It ranks nothing and
recommends nothing.

```bash
python3 research/monetary-policy/primary_sensitivity.py --summary
python3 research/monetary-policy/primary_sensitivity.py --output /tmp/ps.csv
python3 research/monetary-policy/test_primary_sensitivity.py
```

The generated CSV (~1,650 rows) is deliberately **not committed**: the driver
is deterministic, so the source plus the command above is the durable
evidence.

### The grid

80 schedules, the Cartesian product of three independently varied dimensions:

| Dimension | Values |
|---|---|
| initial annual issuance | 788,400 · 1,576,800 · **3,153,600** · 6,307,200 · 12,614,400 |
| decay/step interval | 2 · **5** · 10 · 20 years |
| decay factor | 0.25 · **0.50** · 0.75 · 0.90 |

Bold marks the documented historical schedule, present exactly once as
`historical_reference`. It is a reference point, **not** an optimum, a
recommendation, or a neutral baseline.

### Why two matrices

A design audit run before the driver existed measured that holding the
documented 31,536,000 cap constant would clamp 52 of the 80 schedules onto an
**identical** cumulative total, leaving 36% of the resolution. The cause is
structural, and is itself a research finding:

> For the documented parameters, the infinite geometric issuance limit
> `I × S / (1 − F)` = `3,153,600 × 5 / 0.5` = **31,536,000** — exactly the
> documented cap. The historical schedule sits precisely on its own cap, so
> any larger, slower-decaying or longer-stepped schedule exceeds it.

This coupling is an observation about the documented figures. It is **not** an
argument for keeping or removing the cap.

Rather than raise the cap, drop the schedules that exceed it, or derive a cap
per run — each of which would hide the effect or confound the comparison —
the study runs the same 80 schedules twice:

| Matrix | Cap | Measures |
|---|---|---|
| `uncapped_primary_mechanics` | none | the schedule itself, at full resolution |
| `fixed_supply_cap_interaction` | 31,536,000 | how arbitrary schedules interact with the documented cap |

**Their results are never combined into one range.** The uncapped matrix does
not propose uncapped MBO issuance; it is a mathematical isolation device.

### Cap semantics, as implemented

`clamp_to_cap` computes headroom as `cap − supply`, using **current supply**
rather than cumulative issuance. Burns lower supply and therefore reopen
issuance capacity. With the controlled fee split, cumulative issuance in the
capped matrix reaches up to **1.117×** the cap while outstanding supply stays
at or below it.

So the model implements a **supply cap, not a cumulative issuance cap**. The
study reports the model as it is rather than inventing different semantics,
which is why matrix B is named for a supply cap and why cumulative issuance
and outstanding supply are reported as separate columns.

### Controlled variables

Everything except `monetary.primary_emission` and `monetary.cap` is inherited
unchanged from the `base` scenario: fee split, burn share, compute
assumptions, staking ratio, circulating fraction, adoption trajectory,
reference-price trajectory and tail behaviour. A test asserts that paired runs
differ in nothing else, so the comparison isolates the schedule.

Price is held at the `base` trajectory across the core matrices and is an
exogenous assumption throughout. No schedule is ranked by assumed price.

### Dilution

Two transparent metrics, both derived from quantities the model already
produces:

- `dilution_gross_rate` = `gross_issuance ÷ starting_supply` — the dilution
  borne by a holder who receives none of the issuance;
- `dilution_net_rate` = `(gross_issuance − burned) ÷ starting_supply`.

Staker-specific dilution net of staking rewards is **NOT_MODELLED**: issuance
is allocated to validators as an aggregate, never to stakers pro rata.

### Security metrics

The study reports validator issuance revenue, validator fee revenue, security
budget in MBO, its reference value, and `security_value_at_risk` separately.
It does **not** claim that more issuance means more security, and the model
still contains no calibrated attack-cost or threat model.

### Stress layer

The existing shock mechanisms from `stress_price_shock`,
`stress_compute_contraction` and `stress_combined` are applied to four
schedules selected by **shape**, not by desirability: `low_fast_decay`,
`historical_reference`, `high_front_loaded`, `slow_decay_long_duration`. The
full 80 × stress product is deliberately not run.

### Limitations

- Sensitivity, not optimization: no schedule is preferred.
- One controlled adoption/fee/price configuration; interactions with varying
  adoption are out of scope.
- Tail policy is fixed across the core matrices, so tails are not compared.
- The cap is a supply cap, as described above.
- Extreme low-issuance corners can burn supply to zero under the controlled
  fee split; that is model-consistent behaviour, not a prediction.
  *Qualified 2026-11-07:* those runs are also infeasible, because they keep
  paying fees once the supply is gone. The summary's lowest ending supply of 0
  comes from them; over feasible runs it is 457,578 MBO.

## Cap semantics comparative study

`cap_semantics.py` runs the same 80 schedules under three readings of the
documented figure 31,536,000, and measures how far apart they end up. It
picks none of them.

The question is not invented here. Historical documentation asserts both
readings, in the same file:

    docs/economic_security.md:137    Sigma (all MBO ever created)   <= 31,536,000
    docs/economic_security.md:1205   for all t: Sigma(all MBO at t) <= 31,536,000

Once burns exist those are different claims. Burn permanence is documented
("ALL BURNS PERMANENTLY REDUCE SUPPLY"); whether a burn reopens issuance
capacity is documented nowhere, in either direction. The runtime implements
neither reading. The model had to pick one to run at all, and picked the
outstanding reading. That was an implementation necessity, not a finding.

### The three families

With `C` = 31,536,000:

| Family | Invariant | Headroom | Burn |
|---|---|---|---|
| `lifetime_cumulative_issuance_cap` | `cumulative_gross_issuance(t) <= C` | `C - cumulative_issued` | never restores headroom |
| `outstanding_supply_cap` | `outstanding_supply(t) <= C` | `C - outstanding_supply` | may restore headroom |
| `dual_cap` | both of the above | `min(both)` | may restore outstanding headroom only |

The outstanding family is the model's existing behaviour, unchanged; a test
asserts its trajectories are identical to what `hard_cap` already produces,
so nothing measured in earlier work is silently redefined.

### How the alternatives are expressed

`clamp_to_cap(supply, issuance)` is called exactly once per period by every
`gross_issuance` implementation, and its return value becomes that period's
gross issuance. Overriding that one method is therefore enough, and no other
part of the model moves.

`model.MonetaryPolicy.clamp_to_cap` is not patched or replaced. Three new
family names are added to `model.POLICY_FAMILIES`; the four original keys
still point at the original classes, and a test asserts it. Lifetime and
dual headroom need cumulative issuance during the run, which `clamp_to_cap`
is not handed, so the subclass accumulates what it grants — and a test
checks that accumulator against the externally summed `gross_issuance`.

### Accounting order

Audited rather than assumed. Per period `simulate` runs: starting supply,
then issuance (where the clamp happens), then fees, then the burn, then the
supply update, then security metrics. The cap is therefore evaluated against
**beginning-of-period supply**, which is the previous period's post-burn
ending supply. A burn in year `t` widens outstanding headroom in year `t+1`,
never in year `t`. This study does not change that ordering.

### The burn control

Every comparison is run twice: once with the controlled fee split, once with
the burn share moved to the treasury. It moves to the treasury and not to
validators specifically so validator fee revenue — and therefore the
security budget — is identical between the two, leaving burn as the only
difference. That makes it a control on burn rather than a second,
confounded experiment.

### Results

Divergence between families is entirely burn-driven in this model. Without
burns all three families produce identical trajectories for all 80
schedules. With burns, lifetime and outstanding differ on 52 of 80.

The documented schedule is unaffected by the choice: `3,153,600 / 5 / 0.50`
produces byte-identical 101-row trajectories under all three families, and
clamps under none of them. Its cumulative issuance converges to
31,535,969.92 — just under `C`, never touching it.

`dual_cap` collapses onto `lifetime_cumulative_issuance_cap` here, and that
is arithmetic rather than coincidence:

    outstanding_headroom - lifetime_headroom = cumulative_burn - initial_supply

With `initial_supply = 0` and non-negative burns the lifetime bound is
always the binding one. A test constructs the opposite case — a non-zero
starting supply with no burns — where dual instead tracks the outstanding
bound and is strictly tighter than lifetime. So the two are not the same
constraint; they coincide under this scenario set's starting conditions.

That same constructed case shows a property worth stating plainly: a
lifetime cap bounds what is created, not what exists. With a pre-existing
supply it permits outstanding supply above `C`. Recorded as a property of
the family, not as a defect and not as a preference.

### Reproduction

    python3 research/monetary-policy/cap_semantics.py --summary
    python3 research/monetary-policy/cap_semantics.py --output /tmp/cap.csv
    python3 research/monetary-policy/cap_semantics.py --summary --horizon 150
    python3 research/monetary-policy/test_cap_semantics.py

The CSV is 3,840 rows and is not committed; the driver is deterministic and
byte-for-byte reproducible, so the file is regenerated rather than stored.

### Limitations

- Three candidate readings, not an exhaustive space of cap designs.
- One controlled scenario supplies every non-cap assumption, so the
  divergence magnitudes are specific to that fee split and activity path.
- The `dual` and `lifetime` coincidence is contingent on `initial_supply = 0`;
  a scenario set with a genesis allocation would separate them.
- Checkpoints are sampled at 1, 5, 10, 20, 30, 50, 75 and 100 years, so
  divergences that open and close entirely between two checkpoints are
  visible in the clamp counters but not in the reported levels.
- Nothing here measures whether a cap should exist, or at what magnitude.

## Adding a scenario

Add an entry to `scenarios.json`; do not edit `model.py`. A scenario needs:

- `description` and `normative: false`
- `monetary`: family, initial supply, optional cap, primary emission, and the
  family's own parameters (`tail`, `adaptive`)
- `supply_distribution`: circulating fraction and staking ratio
- `issuance_allocation`: the validator share of issuance
- `fees`: validator/treasury/burn shares, which must sum to 1
- `activity`: fee and compute-spend paths, worker share, protocol fee rate
- `price`: the exogenous reference-price path
- `shocks`: optional, each with a metric, an inclusive year window and a
  multiplier, and optionally what happens after the window (see
  [Horizon validity](#horizon-validity))

Growth paths are piecewise: an initial value plus phases carrying an
`annual_growth` that may be negative. That is how a scenario expresses growth,
maturity, stagnation or contraction without the engine assuming any of them.
Nothing forces or implies perpetual exponential growth. A path is defined
through its last phase only, unless it declares a `continuation`; a run longer
than that is reported as extrapolated.

Shockable metrics: `transaction_fee_revenue_mbo`, `compute_user_spend_mbo`,
`mbo_reference_price`, `staking_ratio`.

## Illustrative versus normative

Every number in `scenarios.json` is **illustrative**. None of it is a proposal.
A test asserts that no shipped scenario is marked normative, so the distinction
cannot rot silently.

The one scenario with real provenance is `historical_documented_partial`, and
the suffix is load-bearing. It reconstructs the **issuance schedule and cap**
written in `docs/supply_schedule.md` faithfully. It does **not** reconstruct
the documented fee system: `docs/economic_summary.md` describes two channels —
base fee burned, priority fee routed to validators and providers — without
stating the ratio between them, and this simulator has a single aggregate fee
pool. Setting `burn_share` to 1.0 models the base-fee burn channel only, so
validator fee revenue is zero there by construction and the scenario
**understates the documented security budget by the whole priority-fee
channel**. No base/priority ratio was invented to close that gap.

That schedule is *documentation*, not implemented behaviour. The scenario is
included so the documented plan can be compared against alternatives instead
of assumed.

## Units

MBO quantities and reference-value quantities are kept in separate columns and
never added together. Columns ending in `_reference_price` or
`_reference_value` are denominated in the scenario's arbitrary reference unit;
everything else is MBO or a pure ratio.

Arithmetic uses floating point, and identities are checked to a relative
tolerance of 1e-9. Results are indicative at that precision, not exact
settlement arithmetic.

## Where the decision framework lives

These studies measure; they decide nothing. The criteria a candidate policy
would have to satisfy, the hard-versus-soft separation, the open decision
gates and the minimum set of further experiments are in
[RFC 0004](../../docs/rfcs/0004-mbo-monetary-decision-framework.md). Six of
the seven experiments it identifies are blocked on mechanisms that do not
exist yet — a staking design, a fee model, a compute market — rather than on
simulation capacity.
