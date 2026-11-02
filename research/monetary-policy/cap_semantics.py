#!/usr/bin/env python3
"""Comparative study of three candidate MBO cap semantics.

WHAT THIS IS NOT
================

This selects no cap semantics. It changes no runtime or consensus
behaviour. It does not argue that any of the three families is correct,
safer, fairer or more faithful to intent. It measures how far apart they
are, and where the distance comes from.

WHY THE QUESTION EXISTS
=======================

Historical Mbongo documentation asserts the figure 31,536,000 MBO in two
mutually stronger forms, in the same file:

    docs/economic_security.md:137    Sigma (all MBO ever created) <= 31,536,000
    docs/economic_security.md:1205   for all t: Sigma(all MBO at time t) <= 31,536,000

Once burns exist these are different claims. Burn permanence IS documented
("ALL BURNS PERMANENTLY REDUCE SUPPLY", "No mechanism to recover burned
MBO"). Whether a burn reopens issuance capacity is documented NOWHERE, in
either direction. The runtime implements neither reading: there is no
issuance, mint, burn, cap or supply accounting in `crates/` at all.

The research model introduced in PR #72 had to pick one in order to run,
and picked the outstanding reading (`headroom = cap - supply`). That was an
implementation necessity, not a finding. This study puts the three readings
side by side under identical assumptions.

THE THREE FAMILIES
==================

With C = 31,536,000:

  A. LIFETIME CUMULATIVE ISSUANCE CAP
     invariant : cumulative_gross_issuance(t) <= C
     headroom  : C - cumulative_gross_issuance_before_the_period
     Burn never restores issuance headroom. Burn moves outstanding supply
     only.

  B. OUTSTANDING SUPPLY CAP
     invariant : outstanding_supply(t) <= C
     headroom  : C - outstanding_supply_at_the_start_of_the_period
     Burn lowers outstanding supply and MAY therefore restore issuance
     headroom. This is what the current research model already does.

  C. DUAL CAP
     invariant : both of the above hold simultaneously
     headroom  : min(lifetime_headroom, outstanding_headroom)
     Burn may create outstanding headroom but must never restore lifetime
     headroom.

ACCOUNTING ORDER (audited, not assumed)
=======================================

`model.simulate` runs, per period, in this order:

  1. starting_supply = previous period's ending_supply
  2. gross_issuance  = policy.gross_issuance(year, starting_supply, ctx)
                       -- this is where clamp_to_cap runs
  3. compute/protocol fees split out of activity
  4. fee_pool split into validator / treasury / burned
  5. ending_supply  = starting_supply + gross_issuance - burned
  6. validator issuance revenue, security budget
  7. circulating and staked supply, prices, value at risk

So the cap is evaluated against BEGINNING-OF-PERIOD supply, which is the
previous period's POST-BURN ending supply. A burn in year t therefore
widens the outstanding headroom available in year t+1, never in year t.
This study does not change that ordering.

HOW THE ALTERNATIVE SEMANTICS ARE IMPLEMENTED
=============================================

`clamp_to_cap(supply, issuance)` is called exactly once per period by every
`gross_issuance` implementation, and its return value becomes that period's
gross issuance. Overriding that single method is therefore sufficient to
express all three families, and nothing else in the model has to move.

Lifetime and dual semantics need cumulative gross issuance DURING the run,
which `clamp_to_cap` is not handed, so the subclass accumulates what it
grants. That accumulator is checked against the externally derived
`sum(row.gross_issuance)` in the tests, so the two agree by construction
rather than by assertion.

`model.MonetaryPolicy.clamp_to_cap` is NOT patched, replaced or monkeyed.
The four original policy families keep their exact behaviour, and every
existing scenario keeps its exact output. New family names are ADDED to
`model.POLICY_FAMILIES` so that `model.simulate` can build them; the
original four keys are left pointing at the original classes.

Run:

    python3 research/monetary-policy/cap_semantics.py --summary
    python3 research/monetary-policy/cap_semantics.py --output /tmp/cap.csv
"""

from __future__ import annotations

import argparse
import copy
import csv
import sys
from dataclasses import dataclass, fields
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent))

import model  # noqa: E402
import primary_sensitivity as ps  # noqa: E402


# ── Definitions ─────────────────────────────────────────────────────────

# The documented figure. Reused, not re-derived: this study takes no view
# on whether the magnitude is right.
CAP = ps.HISTORICAL_CAP

LIFETIME = "lifetime_cumulative_issuance_cap"
OUTSTANDING = "outstanding_supply_cap"
DUAL = "dual_cap"
CAP_FAMILIES: Tuple[str, ...] = (LIFETIME, OUTSTANDING, DUAL)

WITH_BURN = "with_burn"
NO_BURN = "no_burn_control"
BURN_ENVIRONMENTS: Tuple[str, ...] = (WITH_BURN, NO_BURN)

HORIZON_YEARS = ps.HORIZON_YEARS
CHECKPOINTS = (1, 5, 10, 20, 30, 50, 75, 100)
CONTROLLED_SCENARIO = ps.CONTROLLED_SCENARIO
HISTORICAL_REFERENCE = ps.HISTORICAL_REFERENCE

# Float comparison floor. Issuance figures here are of order 1e6..1e9, so a
# 1e-6 absolute floor is far below any economically meaningful difference
# and far above accumulated float noise.
EPSILON = 1e-6


# ── Alternative cap semantics ───────────────────────────────────────────


def _make_policy(base: type, semantics: str) -> type:
    """A policy class identical to `base` except for its cap semantics."""

    class _CapSemanticsPolicy(base):  # type: ignore[misc, valid-type]
        cap_semantics = semantics

        def __init__(self, cfg: Dict[str, Any]) -> None:
            super().__init__(cfg)
            # What this policy has actually granted so far. Lifetime and
            # dual headroom are measured against this, never against
            # outstanding supply.
            self.cumulative_granted = 0.0

        def clamp_to_cap(self, supply: float, issuance: float) -> float:
            if self.cap is None:
                self.cumulative_granted += issuance
                return issuance
            rooms: List[float] = []
            if semantics in (LIFETIME, DUAL):
                rooms.append(self.cap - self.cumulative_granted)
            if semantics in (OUTSTANDING, DUAL):
                rooms.append(self.cap - supply)
            granted = min(issuance, max(0.0, min(rooms)))
            self.cumulative_granted += granted
            return granted

    _CapSemanticsPolicy.__name__ = f"{base.__name__}_{semantics}"
    _CapSemanticsPolicy.__qualname__ = _CapSemanticsPolicy.__name__
    return _CapSemanticsPolicy


def family_name(semantics: str) -> str:
    """The `monetary.family` value that selects `semantics`.

    The controlled scenario uses `hard_cap`, so only that base family needs
    a variant. Deriving the name rather than hardcoding it keeps the
    registration and the lookup from drifting apart.
    """
    return f"hard_cap__{semantics}"


def register_families() -> None:
    """Add the three variants to the model's family registry.

    Additive only. The four original keys are asserted untouched, so a
    future rename in the model surfaces here instead of silently changing
    what every existing scenario means.
    """
    original = dict(model.POLICY_FAMILIES)
    for semantics in CAP_FAMILIES:
        model.POLICY_FAMILIES[family_name(semantics)] = _make_policy(
            model.HardCapPolicy, semantics
        )
    for key, cls in original.items():
        assert model.POLICY_FAMILIES[key] is cls, f"clobbered existing family {key!r}"


register_families()


# ── Observations ────────────────────────────────────────────────────────


@dataclass(frozen=True)
class Observation:
    """One (cap family, burn environment, schedule, checkpoint) row.

    `cumulative_gross_issuance` and `outstanding_supply` are named in full
    and reported separately on purpose: conflating them under a single
    "total supply" label is exactly the ambiguity this study exists to
    measure.
    """

    cap_semantics: str
    burn_environment: str
    initial_issuance: int
    step_interval: int
    decay_factor: float
    is_historical_reference: bool
    year: int

    cumulative_gross_issuance: float
    outstanding_supply: float
    cumulative_burn: float

    first_clamp_year: str
    clamped_periods: int

    issuance_headroom_lifetime: float
    issuance_headroom_outstanding: float

    gross_issuance: float
    gross_inflation_rate: float
    net_inflation_rate: float

    validator_issuance_revenue: float
    validator_fee_revenue: float
    security_budget_mbo: float

    staked_supply: float
    security_value_at_risk: float


CSV_COLUMNS = [f.name for f in fields(Observation)]


# ── Scenario construction ───────────────────────────────────────────────


def controlled_scenario(config: Dict[str, Any]) -> Dict[str, Any]:
    return ps.controlled_scenario(config)


def strip_burn(scenario: Dict[str, Any]) -> Dict[str, Any]:
    """The no-burn control: the burn share moves to the treasury.

    The fee shares must sum to 1, so the burned share has to go somewhere.
    It goes to the treasury rather than to validators precisely so that
    validator fee revenue -- and therefore the security budget -- is
    identical to the burning run. The only thing that changes is whether
    those fees leave the supply. That is what makes this a control on burn
    and not a second, confounded experiment.
    """
    out = copy.deepcopy(scenario)
    fees = out["fees"]
    out["fees"] = {
        "validator_share": fees["validator_share"],
        "treasury_share": fees["treasury_share"] + fees["burn_share"],
        "burn_share": 0.0,
    }
    return out


def build_scenario(
    base: Dict[str, Any],
    schedule: Tuple[int, int, float],
    semantics: str,
    burn_environment: str,
) -> Dict[str, Any]:
    """Controlled scenario with ONLY the schedule, cap family and burn
    environment replaced. Every other assumption is inherited verbatim."""
    scenario = ps.build_scenario(base, schedule, CAP)
    scenario["monetary"]["family"] = family_name(semantics)
    if burn_environment == NO_BURN:
        scenario = strip_burn(scenario)
    return scenario


# ── Measurement ─────────────────────────────────────────────────────────


def clamp_profile(
    scenario: Dict[str, Any], rows: List[model.YearRow]
) -> Tuple[Optional[int], int]:
    """(first clamped year, number of clamped periods).

    The schedule is never re-derived here: `primary_issuance` comes from the
    model, so what the schedule "wanted" is the model's own answer.
    """
    policy = model.build_policy(scenario["monetary"])
    first: Optional[int] = None
    count = 0
    for row in rows:
        if row.year < 1:
            continue
        wanted = policy.primary_issuance(row.year)
        if wanted - row.gross_issuance > EPSILON:
            count += 1
            if first is None:
                first = row.year
    return first, count


def observations_for(
    semantics: str,
    burn_environment: str,
    schedule: Tuple[int, int, float],
    scenario: Dict[str, Any],
    rows: List[model.YearRow],
) -> List[Observation]:
    initial, step_interval, decay_factor = schedule
    first_clamp, clamped_periods = clamp_profile(scenario, rows)
    cumulative_issuance = 0.0
    cumulative_burn = 0.0
    out: List[Observation] = []
    for row in rows:
        cumulative_issuance += row.gross_issuance
        cumulative_burn += row.burned
        if row.year not in CHECKPOINTS:
            continue
        out.append(
            Observation(
                cap_semantics=semantics,
                burn_environment=burn_environment,
                initial_issuance=initial,
                step_interval=step_interval,
                decay_factor=decay_factor,
                is_historical_reference=schedule == HISTORICAL_REFERENCE,
                year=row.year,
                cumulative_gross_issuance=cumulative_issuance,
                outstanding_supply=row.ending_supply,
                cumulative_burn=cumulative_burn,
                first_clamp_year="" if first_clamp is None else str(first_clamp),
                clamped_periods=clamped_periods,
                issuance_headroom_lifetime=max(0.0, CAP - cumulative_issuance),
                issuance_headroom_outstanding=max(0.0, CAP - row.ending_supply),
                gross_issuance=row.gross_issuance,
                gross_inflation_rate=row.gross_inflation_rate,
                net_inflation_rate=row.net_inflation_rate,
                validator_issuance_revenue=row.validator_issuance_revenue,
                validator_fee_revenue=row.validator_fee_revenue,
                security_budget_mbo=row.security_budget_mbo,
                staked_supply=row.staked_supply,
                security_value_at_risk=row.security_value_at_risk,
            )
        )
    return out


def run_matrix(
    config: Dict[str, Any], burn_environment: str
) -> List[Observation]:
    """All three cap families over the shared 80-schedule grid."""
    base = controlled_scenario(config)
    out: List[Observation] = []
    for semantics in CAP_FAMILIES:
        for schedule in ps.schedule_grid():
            scenario = build_scenario(base, schedule, semantics, burn_environment)
            rows = model.simulate(scenario, HORIZON_YEARS)
            out.extend(
                observations_for(semantics, burn_environment, schedule, scenario, rows)
            )
    return out


def run_all(config: Dict[str, Any]) -> List[Observation]:
    out: List[Observation] = []
    for burn_environment in BURN_ENVIRONMENTS:
        out.extend(run_matrix(config, burn_environment))
    return out


# ── Divergence ──────────────────────────────────────────────────────────

DIVERGENCE_FIELDS = (
    "cumulative_gross_issuance",
    "outstanding_supply",
    "cumulative_burn",
    "security_budget_mbo",
    "validator_issuance_revenue",
)


def terminal_index(
    observations: List[Observation],
) -> Dict[Tuple[str, str, Tuple[int, int, float]], Observation]:
    """Horizon-year observation, keyed by (burn env, cap family, schedule)."""
    return {
        (o.burn_environment, o.cap_semantics, (o.initial_issuance, o.step_interval, o.decay_factor)): o
        for o in observations
        if o.year == HORIZON_YEARS
    }


def pair_divergence(
    observations: List[Observation], burn_environment: str, left: str, right: str
) -> Dict[str, Any]:
    """How far apart two cap families end up, schedule by schedule."""
    index = terminal_index(observations)
    identical = 0
    different = 0
    deltas: Dict[str, float] = {f: 0.0 for f in DIVERGENCE_FIELDS}
    clamp_year_deltas = 0
    for schedule in ps.schedule_grid():
        a = index[(burn_environment, left, schedule)]
        b = index[(burn_environment, right, schedule)]
        gaps = {f: abs(getattr(a, f) - getattr(b, f)) for f in DIVERGENCE_FIELDS}
        if max(gaps.values()) > EPSILON:
            different += 1
        else:
            identical += 1
        for f, g in gaps.items():
            deltas[f] = max(deltas[f], g)
        if a.first_clamp_year != b.first_clamp_year:
            clamp_year_deltas += 1
    return {
        "burn_environment": burn_environment,
        "left": left,
        "right": right,
        "identical": identical,
        "different": different,
        "max_deltas": deltas,
        "differing_clamp_years": clamp_year_deltas,
    }


def all_divergences(observations: List[Observation]) -> List[Dict[str, Any]]:
    pairs = ((LIFETIME, OUTSTANDING), (LIFETIME, DUAL), (OUTSTANDING, DUAL))
    return [
        pair_divergence(observations, env, left, right)
        for env in BURN_ENVIRONMENTS
        for left, right in pairs
    ]


def historical_reference_rows(
    config: Dict[str, Any], burn_environment: str = WITH_BURN
) -> Dict[str, List[model.YearRow]]:
    """Full 101-row trajectory of the documented schedule, per cap family."""
    base = controlled_scenario(config)
    return {
        semantics: model.simulate(
            build_scenario(base, HISTORICAL_REFERENCE, semantics, burn_environment),
            HORIZON_YEARS,
        )
        for semantics in CAP_FAMILIES
    }


def first_divergence_year(
    left: List[model.YearRow], right: List[model.YearRow]
) -> Optional[int]:
    """Earliest year two trajectories differ in any reported quantity."""
    names = [f.name for f in fields(model.YearRow) if f.name != "year"]
    for a, b in zip(left, right):
        for name in names:
            if abs(getattr(a, name) - getattr(b, name)) > EPSILON:
                return a.year
    return None


# ── Output ──────────────────────────────────────────────────────────────


def write_csv(observations: List[Observation], path: str) -> None:
    with open(path, "w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_COLUMNS)
        writer.writeheader()
        for o in observations:
            writer.writerow({name: getattr(o, name) for name in CSV_COLUMNS})


def _span(values: List[float]) -> str:
    return f"{min(values):>18,.2f} .. {max(values):>18,.2f}"


def format_summary(observations: List[Observation], config: Dict[str, Any]) -> str:
    lines: List[str] = []
    lines.append("MBO CAP SEMANTICS COMPARISON -- RESEARCH, NOT A DECISION")
    lines.append("Three readings of the same documented figure, measured side by side.")
    lines.append("No family here is preferred, endorsed or recommended.")
    lines.append("")
    lines.append(f"cap                     : {CAP:,}")
    lines.append(f"schedules per family    : {len(ps.schedule_grid())}")
    lines.append(f"cap families            : {len(CAP_FAMILIES)}")
    lines.append(f"horizon                 : {HORIZON_YEARS} years")
    lines.append("")

    for env in BURN_ENVIRONMENTS:
        lines.append(f"=== burn environment: {env} ===")
        for semantics in CAP_FAMILIES:
            final = [
                o
                for o in observations
                if o.burn_environment == env
                and o.cap_semantics == semantics
                and o.year == HORIZON_YEARS
            ]
            clamped = [o for o in final if o.clamped_periods > 0]
            lines.append(f"  [{semantics}]")
            lines.append(
                f"    cumulative gross issuance : {_span([o.cumulative_gross_issuance for o in final])}"
            )
            lines.append(
                f"    outstanding supply        : {_span([o.outstanding_supply for o in final])}"
            )
            lines.append(
                f"    cumulative burn           : {_span([o.cumulative_burn for o in final])}"
            )
            lines.append(
                f"    security budget (MBO)     : {_span([o.security_budget_mbo for o in final])}"
            )
            distinct = len({round(o.cumulative_gross_issuance, 2) for o in final})
            lines.append(
                f"    distinct issuance totals  : {distinct}/{len(final)}"
                f"   clamped schedules: {len(clamped)}/{len(final)}"
            )
            over_cap = [o for o in final if o.cumulative_gross_issuance > CAP + EPSILON]
            supply_over = [o for o in final if o.outstanding_supply > CAP + EPSILON]
            lines.append(
                f"    schedules with cumulative issuance > cap : {len(over_cap)}"
            )
            lines.append(
                f"    schedules with outstanding supply > cap  : {len(supply_over)}"
            )
        lines.append("")

    lines.append("=== pairwise divergence at year 100 (never ranked) ===")
    for d in all_divergences(observations):
        lines.append(
            f"  [{d['burn_environment']}] {d['left']} vs {d['right']}: "
            f"identical {d['identical']}/80, different {d['different']}/80, "
            f"differing first-clamp years {d['differing_clamp_years']}/80"
        )
        for field_name, gap in d["max_deltas"].items():
            lines.append(f"      max delta {field_name:<28}: {gap:>18,.2f}")
    lines.append("")

    lines.append("=== documented historical schedule (3,153,600 / 5 / 0.50) ===")
    traj = historical_reference_rows(config)
    base_rows = traj[LIFETIME]
    for semantics in CAP_FAMILIES[1:]:
        year = first_divergence_year(base_rows, traj[semantics])
        verdict = "identical over 100 years" if year is None else f"first differs in year {year}"
        lines.append(f"  {LIFETIME} vs {semantics}: {verdict}")
    lines.append(
        f"  cumulative gross issuance : {sum(r.gross_issuance for r in base_rows):,.2f}"
    )
    lines.append(f"  outstanding supply        : {base_rows[-1].ending_supply:,.2f}")
    lines.append("")
    lines.append("Historical documentation asserts both readings and settles neither.")
    lines.append("The runtime implements neither. This study settles neither.")
    return "\n".join(lines)


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Compare three candidate MBO cap semantics. Decides nothing."
    )
    parser.add_argument("--config", default=model.DEFAULT_CONFIG)
    parser.add_argument("--output", help="write the full observation CSV here")
    parser.add_argument("--summary", action="store_true", help="print the summary")
    args = parser.parse_args(argv)

    config = model.load_config(args.config)
    observations = run_all(config)

    if args.output:
        write_csv(observations, args.output)
        print(f"CSV written to {args.output}   rows: {len(observations)}")
    if args.summary or not args.output:
        print(format_summary(observations, config))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
