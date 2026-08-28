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

Exit code `0` means the run completed and every accounting identity held;
`1` means an identity was violated; `2` means the configuration was rejected.

## Tests

```bash
python3 research/monetary-policy/test_model.py
# or
python3 -m unittest discover -s research/monetary-policy -t .
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
  multiplier

Growth paths are piecewise: an initial value plus phases carrying an
`annual_growth` that may be negative. That is how a scenario expresses growth,
maturity, stagnation or contraction without the engine assuming any of them.
Nothing forces or implies perpetual exponential growth.

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
