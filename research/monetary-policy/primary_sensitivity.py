#!/usr/bin/env python3
"""Primary-issuance sensitivity study for the MBO monetary policy model.

Research driver, not a policy proposal. It answers one question:

    How do long-run supply, dilution and security-budget outcomes change when
    the PRIMARY ISSUANCE schedule changes, while every non-issuance
    assumption is held constant?

This is sensitivity analysis, NOT optimization. Nothing here ranks schedules,
recommends one, or selects an MBO monetary policy.

It imports `model` and reuses its equations; no monetary arithmetic is
re-implemented here.

Two matrices over the SAME 80 schedules
---------------------------------------
A design audit run before this driver existed measured that holding the
documented 31,536,000 cap constant would clamp 52 of the 80 schedules onto an
identical cumulative total, leaving only 36% of the resolution. The cause is
structural: for the documented parameters the infinite geometric limit
I x S / (1 - F) = 3,153,600 x 5 / 0.5 is exactly 31,536,000, so the historical
schedule sits precisely on its own cap. Any larger, slower-decaying or
longer-stepped schedule exceeds it.

Rather than raise the cap, drop schedules, or derive a cap per run - each of
which would either hide the effect or confound the comparison - the study runs
both:

  A. uncapped_primary_mechanics    - no cap; isolates the schedule itself
  B. fixed_supply_cap_interaction  - cap 31,536,000; measures the interaction

Their results are never combined into one range.

Cap semantics, as implemented
-----------------------------
`model.MonetaryPolicy.clamp_to_cap` computes headroom as `cap - supply`, using
CURRENT SUPPLY rather than cumulative issuance. Burns lower supply and
therefore reopen issuance capacity. With the controlled fee split (which burns
20% of the fee pool) a schedule can finish with cumulative issuance ABOVE the
cap while outstanding supply stays at or below it.

That is the model's actual behaviour, and this driver reports it rather than
inventing different semantics. Matrix B is named for a SUPPLY cap for that
reason, and reports cumulative issuance and outstanding supply separately.

Usage:
    python3 research/monetary-policy/primary_sensitivity.py --summary
    python3 research/monetary-policy/primary_sensitivity.py --output /tmp/ps.csv
    python3 research/monetary-policy/primary_sensitivity.py --summary --horizon 150

Exit status: 0, or 4 if any run goes beyond what its inputs define (every
observation is still written, and extrapolated runs are excluded from every
range), or 2 for a configuration error.
"""

from __future__ import annotations

import argparse
import copy
import csv
import sys
from dataclasses import dataclass, asdict, fields
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent))

import model  # noqa: E402

STUDY_VERSION = "0.1.0-research"
# The historical study horizon, and the default. 150 and 200 years can be
# requested, but a run is only comparable where its inputs are defined: with
# the shipped scenarios every input ends in year 100, so a longer horizon
# yields extrapolated runs, which are reported and excluded, never ranged.
HORIZON_YEARS = 100
SUPPORTED_HORIZONS = (100, 150, 200)

# ── The grid ────────────────────────────────────────────────────────────
# Four octaves of magnitude, four decay intervals from near-continuous to
# generational, and four decay factors from fast extinction to near-perpetual.
# Values are chosen to span qualitatively different issuance SHAPES, not to
# sit at cosmetic distances around the historical point.
INITIAL_ANNUAL_ISSUANCE = [788_400, 1_576_800, 3_153_600, 6_307_200, 12_614_400]
STEP_YEARS = [2, 5, 10, 20]
STEP_FACTOR = [0.25, 0.50, 0.75, 0.90]

# The documented schedule. Present in the grid exactly once, as a reference
# point only: it is not optimal, recommended, neutral, or a baseline truth.
HISTORICAL_REFERENCE: Tuple[int, int, float] = (3_153_600, 5, 0.50)

# The documented cap. Structurally coupled to the historical schedule:
# 3,153,600 x 5 / (1 - 0.5) = 31,536,000.
HISTORICAL_CAP = 31_536_000

UNCAPPED = "uncapped_primary_mechanics"
FIXED_CAP = "fixed_supply_cap_interaction"
MATRICES: Dict[str, Optional[int]] = {UNCAPPED: None, FIXED_CAP: HISTORICAL_CAP}

CHECKPOINTS = (1, 5, 10, 20, 25, 30, 50, 75, 100)
LONG_HORIZON_CHECKPOINTS = (150, 200)


def checkpoints_for(horizon: int, base: Tuple[int, ...] = CHECKPOINTS) -> Tuple[int, ...]:
    """The historical checkpoints, plus 150 and 200 where the horizon reaches
    them. For 100 years this is exactly the historical tuple."""
    if horizon not in SUPPORTED_HORIZONS:
        raise model.ConfigError(f"horizon must be one of {SUPPORTED_HORIZONS}, got {horizon}")
    return tuple(y for y in base if y <= horizon) + tuple(
        y for y in LONG_HORIZON_CHECKPOINTS if y <= horizon
    )

# The scenario whose non-issuance assumptions are held constant. Everything
# except `monetary.primary_emission` and `monetary.cap` is inherited from it
# unchanged, so the two matrices differ only in what the study varies.
CONTROLLED_SCENARIO = "base"

# Representative shapes for the stress layer, selected by issuance SHAPE and
# not by economic desirability.
REPRESENTATIVE_SHAPES: Dict[str, Tuple[int, int, float]] = {
    "low_fast_decay": (788_400, 2, 0.25),
    "historical_reference": HISTORICAL_REFERENCE,
    "high_front_loaded": (12_614_400, 2, 0.25),
    "slow_decay_long_duration": (3_153_600, 20, 0.90),
}

# Stress mechanisms are reused from the shipped scenarios rather than invented.
STRESS_SOURCES = ("stress_price_shock", "stress_compute_contraction", "stress_combined")


@dataclass(frozen=True)
class Observation:
    """One (matrix, schedule, checkpoint) observation."""

    matrix: str
    initial_annual_issuance: int
    step_years: int
    step_factor: float
    is_historical_reference: bool
    stress: str
    cap: str
    clamped: bool
    first_clamp_year: str
    year: int
    cumulative_issuance: float
    gross_issuance: float
    burned: float
    ending_supply: float
    gross_inflation_rate: float
    net_inflation_rate: float
    dilution_gross_rate: float
    dilution_net_rate: float
    circulating_supply: float
    staked_supply: float
    validator_issuance_revenue: float
    validator_fee_revenue: float
    security_budget_mbo: float
    security_budget_reference_value: float
    security_value_at_risk: float
    cumulative_issuance_over_cap_ratio: str
    supply_over_cap_ratio: str
    # Fee funding (see model.FEE_FUNDING_*). The first is for the whole run, so
    # every checkpoint of an infeasible run carries it; the second counts the
    # infeasible years up to this checkpoint.
    fee_funding_first_infeasible_year: str
    fee_funding_infeasible_years_to_date: int
    # The run's status from its worst year: feasible, velocity_dependent
    # (conditional, never counted as feasible) or infeasible.
    fee_funding_run_status: str
    # Horizon coverage (see model.HORIZON_*). The first is for this
    # checkpoint's year; the next two are for the whole run, empty when every
    # year is covered. A run with a first extrapolated year is never ranged.
    horizon_status: str
    first_extrapolated_year: str
    extrapolated_inputs: str
    # model.run_validity: accounting, fee funding and coverage combined.
    run_validity: str


CSV_COLUMNS = [f.name for f in fields(Observation)]


def schedule_grid() -> List[Tuple[int, int, float]]:
    """The 80 schedules, in a fixed deterministic order."""
    return [
        (i, s, f)
        for i in INITIAL_ANNUAL_ISSUANCE
        for s in STEP_YEARS
        for f in STEP_FACTOR
    ]


def controlled_scenario(config: Dict[str, Any]) -> Dict[str, Any]:
    """The scenario supplying every non-issuance assumption."""
    return copy.deepcopy(model.get_scenario(config, CONTROLLED_SCENARIO))


def build_scenario(
    base: Dict[str, Any],
    schedule: Tuple[int, int, float],
    cap: Optional[int],
    shocks: Optional[List[Dict[str, Any]]] = None,
) -> Dict[str, Any]:
    """Controlled scenario with ONLY the primary schedule and cap replaced."""
    initial, step_years, step_factor = schedule
    scenario = copy.deepcopy(base)
    scenario["monetary"]["cap"] = cap
    scenario["monetary"]["primary_emission"] = {
        "initial_annual_issuance": initial,
        "step_years": step_years,
        "step_factor": step_factor,
        "phase_end_year": None,
    }
    if shocks is not None:
        scenario["shocks"] = copy.deepcopy(shocks)
    return scenario


def detect_clamp(scenario: Dict[str, Any], rows: List[model.YearRow]) -> Optional[int]:
    """First year the cap held issuance below what the schedule called for.

    Uses the model's own `primary_issuance`, so the schedule is never
    re-derived here.
    """
    if scenario["monetary"]["cap"] is None:
        return None
    policy = model.build_policy(scenario["monetary"])
    for row in rows:
        if row.year < 1:
            continue
        wanted = policy.primary_issuance(row.year)
        if wanted - row.gross_issuance > 1e-6:
            return row.year
    return None


def observations_for(
    matrix: str,
    schedule: Tuple[int, int, float],
    scenario: Dict[str, Any],
    rows: List[model.YearRow],
    stress: str,
    checkpoints: Tuple[int, ...] = CHECKPOINTS,
) -> List[Observation]:
    cap = scenario["monetary"]["cap"]
    clamp_year = detect_clamp(scenario, rows)
    infeasible_years = model.years_with_fee_funding(rows, model.FEE_FUNDING_INFEASIBLE)
    run_status = model.fee_funding_run_status(rows)
    validity = model.run_validity(rows)
    first_extrapolated = validity.first_extrapolated_year
    cumulative = 0.0
    by_year: Dict[int, Tuple[model.YearRow, float]] = {}
    for row in rows:
        cumulative += row.gross_issuance
        by_year[row.year] = (row, cumulative)

    out: List[Observation] = []
    for year in checkpoints:
        row, cum = by_year[year]
        out.append(
            Observation(
                matrix=matrix,
                initial_annual_issuance=schedule[0],
                step_years=schedule[1],
                step_factor=schedule[2],
                is_historical_reference=(schedule == HISTORICAL_REFERENCE),
                stress=stress,
                cap="" if cap is None else str(cap),
                clamped=clamp_year is not None,
                first_clamp_year="" if clamp_year is None else str(clamp_year),
                year=year,
                cumulative_issuance=cum,
                gross_issuance=row.gross_issuance,
                burned=row.burned,
                ending_supply=row.ending_supply,
                gross_inflation_rate=row.gross_inflation_rate,
                net_inflation_rate=row.net_inflation_rate,
                # Transparent dilution: the share of a year's new supply
                # relative to the supply that existed at its start. This is
                # the dilution borne by a holder who receives none of the
                # issuance. Staker-specific dilution net of staking rewards is
                # NOT_MODELLED: the model allocates issuance to validators as
                # an aggregate, not to stakers pro rata.
                dilution_gross_rate=row.gross_inflation_rate,
                dilution_net_rate=row.net_inflation_rate,
                circulating_supply=row.circulating_supply,
                staked_supply=row.staked_supply,
                validator_issuance_revenue=row.validator_issuance_revenue,
                validator_fee_revenue=row.validator_fee_revenue,
                security_budget_mbo=row.security_budget_mbo,
                security_budget_reference_value=row.security_budget_reference_value,
                security_value_at_risk=row.security_value_at_risk,
                cumulative_issuance_over_cap_ratio=(
                    "" if cap is None else f"{cum / cap:.6f}"
                ),
                supply_over_cap_ratio=(
                    "" if cap is None else f"{row.ending_supply / cap:.6f}"
                ),
                fee_funding_first_infeasible_year=(
                    str(infeasible_years[0]) if infeasible_years else ""
                ),
                fee_funding_infeasible_years_to_date=sum(
                    1 for y in infeasible_years if y <= year
                ),
                fee_funding_run_status=run_status,
                horizon_status=row.horizon_status,
                first_extrapolated_year=(
                    "" if first_extrapolated is None else str(first_extrapolated)
                ),
                extrapolated_inputs=";".join(validity.extrapolated_inputs),
                run_validity=validity.status,
            )
        )
    return out


def _schedule(o: Observation) -> Tuple[int, int, float]:
    return (o.initial_annual_issuance, o.step_years, o.step_factor)


def is_extrapolated_run(o: Observation) -> bool:
    """True for every observation of a run with an extrapolated year, at any
    checkpoint: the run is excluded as a whole, not year by year."""
    return bool(o.first_extrapolated_year)


def comparable(observations: List[Observation]) -> List[Observation]:
    """The observations a comparison range may use: every run whose inputs the
    scenario defines through the horizon. Extrapolated runs are reported, never
    ranged. Infeasible and conditional runs are kept, and flagged, as before."""
    return [o for o in observations if not is_extrapolated_run(o)]


def horizon_lines(observations: List[Observation], label: str) -> List[str]:
    """How many runs are excluded as extrapolated, and from which year. Empty
    when none is."""
    runs = {(o.matrix, _schedule(o), o.stress) for o in observations}
    excluded = {(o.matrix, _schedule(o), o.stress) for o in observations if is_extrapolated_run(o)}
    if not excluded:
        return []
    firsts = sorted({int(o.first_extrapolated_year) for o in observations if is_extrapolated_run(o)})
    seen = {name for o in observations for name in o.extrapolated_inputs.split(";") if name}
    inputs = [m for m in model.TRAJECTORY_METRICS if m in seen]
    return [
        f"  {label:<26}: {len(excluded)} of {len(runs)} runs EXTRAPOLATED, excluded from every range",
        f"    first extrapolated year  : {firsts[0]} .. {firsts[-1]}",
        f"    inputs continued implicitly: {', '.join(inputs)}",
    ]


def fee_funding_lines(
    observations: List[Observation], label: str, horizon: int = HORIZON_YEARS
) -> List[str]:
    """Infeasible and velocity-dependent runs among these observations, and the
    headline ranges again over fully feasible runs only. A velocity-dependent
    run is conditional, so it is excluded too. The ranges over all runs are
    left as they were."""
    runs = {(_schedule(o), o.stress) for o in observations}
    bad = {(_schedule(o), o.stress) for o in observations
           if o.fee_funding_run_status == model.FEE_FUNDING_INFEASIBLE}
    conditional = {(_schedule(o), o.stress) for o in observations
                   if o.fee_funding_run_status == model.FEE_FUNDING_VELOCITY_DEPENDENT}
    out = [
        f"  {label:<26}: {len(bad)} of {len(runs)} runs",
        f"  {'conditional on velocity':<26}: {len(conditional)} of {len(runs)} runs",
    ]
    if not bad and not conditional:
        return out
    firsts = sorted(
        int(o.fee_funding_first_infeasible_year)
        for o in observations
        if o.fee_funding_first_infeasible_year and o.year == horizon
    )
    if firsts:
        out.append(f"    first infeasible year    : {firsts[0]} .. {firsts[-1]}")
    out.append(
        "    Infeasible runs are NOT VALID, conditional runs hold only with recirculation. "
        "The ranges above include both."
    )
    final = [
        o for o in observations
        if o.year == horizon and o.fee_funding_run_status == model.FEE_FUNDING_FEASIBLE
    ]
    if final:
        lo, hi = _range([o.security_budget_mbo for o in final])
        out.append(f"    security budget @{horizon}y, fully feasible runs : {lo:>14,.0f} .. {hi:>14,.0f} MBO")
        lo, hi = _range([o.ending_supply for o in final])
        out.append(f"    ending supply @{horizon}y, fully feasible runs   : {lo:>14,.0f} .. {hi:>14,.0f} MBO")
        over = [
            float(o.cumulative_issuance_over_cap_ratio)
            for o in final
            if o.cumulative_issuance_over_cap_ratio
        ]
        if over:
            out.append(
                f"    cumulative issuance / cap, fully feasible runs: {min(over):.3f}x .. {max(over):.3f}x"
            )
    return out


def run_core_matrices(config: Dict[str, Any], horizon: int = HORIZON_YEARS) -> List[Observation]:
    """Both matrices over the same grid, in deterministic order."""
    checkpoints = checkpoints_for(horizon)
    base = controlled_scenario(config)
    out: List[Observation] = []
    for matrix, cap in MATRICES.items():
        for schedule in schedule_grid():
            scenario = build_scenario(base, schedule, cap)
            rows = model.simulate(scenario, horizon)
            problems = model.identity_violations(rows)
            if problems:
                raise RuntimeError(f"{matrix} {schedule}: {problems[0]}")
            out.extend(
                observations_for(matrix, schedule, scenario, rows, stress="none",
                                 checkpoints=checkpoints)
            )
    return out


def run_stress_layer(config: Dict[str, Any], horizon: int = HORIZON_YEARS) -> List[Observation]:
    """Existing stress mechanisms applied to four representative shapes."""
    checkpoints = checkpoints_for(horizon)
    base = controlled_scenario(config)
    out: List[Observation] = []
    for stress_name in STRESS_SOURCES:
        shocks = model.get_scenario(config, stress_name).get("shocks", [])
        for _, schedule in sorted(REPRESENTATIVE_SHAPES.items()):
            for matrix, cap in MATRICES.items():
                scenario = build_scenario(base, schedule, cap, shocks=shocks)
                rows = model.simulate(scenario, horizon)
                problems = model.identity_violations(rows)
                if problems:
                    raise RuntimeError(f"{stress_name} {matrix} {schedule}: {problems[0]}")
                out.extend(
                    observations_for(matrix, schedule, scenario, rows, stress=stress_name,
                                     checkpoints=checkpoints)
                )
    return out


def write_csv(observations: List[Observation], path: Path) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_COLUMNS)
        writer.writeheader()
        for obs in observations:
            record = asdict(obs)
            writer.writerow(
                {
                    k: (f"{v:.10f}" if isinstance(v, float) else v)
                    for k, v in record.items()
                }
            )


def _range(values: List[float]) -> Tuple[float, float]:
    return (min(values), max(values))


def extrapolated_run_count(observations: List[Observation]) -> int:
    return len({(o.matrix, _schedule(o), o.stress) for o in observations if is_extrapolated_run(o)})


def format_summary(observations: List[Observation], horizon: int = HORIZON_YEARS) -> str:
    """Factual ranges, reported per matrix and never combined. Ranges use only
    runs whose inputs are defined through the horizon (see `comparable`)."""
    out: List[str] = []
    out.append("=== MBO primary-issuance sensitivity study ===")
    out.append(
        f"study version: {STUDY_VERSION}   horizon: {horizon} years   "
        f"grid: {len(schedule_grid())} schedules per matrix"
    )
    out.append(f"controlled scenario: {CONTROLLED_SCENARIO} (only schedule and cap replaced)")
    out.append("SENSITIVITY ANALYSIS, NOT OPTIMIZATION. No schedule is ranked or recommended.")
    total = len({(o.matrix, _schedule(o), o.stress) for o in observations})
    excluded = extrapolated_run_count(observations)
    out.append(
        f"horizon coverage: {total - excluded} of {total} runs have every input defined "
        f"through year {horizon}; {excluded} extrapolated, excluded from every range"
    )
    out.append("")

    core = [o for o in observations if o.stress == "none"]
    for matrix in MATRICES:
        every = [o for o in core if o.matrix == matrix]
        rows = comparable(every)
        final = [o for o in rows if o.year == horizon]
        out.append(f"--- {matrix} ---")
        out.append(f"  schedules: {len({(o.initial_annual_issuance, o.step_years, o.step_factor) for o in every})}")
        out.extend(horizon_lines(every, "horizon"))
        if not final:
            out.append(f"  no run is defined through year {horizon}: every range is withheld")
            out.append("")
            continue
        lo, hi = _range([o.cumulative_issuance for o in final])
        out.append(f"  cumulative issuance @{horizon}y : {lo:>18,.0f}  ..  {hi:>18,.0f} MBO")
        lo, hi = _range([o.ending_supply for o in final])
        out.append(f"  ending supply @{horizon}y       : {lo:>18,.0f}  ..  {hi:>18,.0f} MBO")
        distinct = len({round(o.cumulative_issuance, 2) for o in final})
        out.append(f"  distinct cumulative totals: {distinct} of {len(final)}")
        y10 = [o for o in rows if o.year == 10]
        lo, hi = _range([o.gross_inflation_rate for o in y10])
        out.append(f"  gross inflation @10y      : {lo:>17.4%}  ..  {hi:>17.4%}")
        lo, hi = _range([o.net_inflation_rate for o in final])
        out.append(f"  net inflation @{horizon}y       : {lo:>17.4%}  ..  {hi:>17.4%}")
        lo, hi = _range([o.security_budget_mbo for o in final])
        out.append(f"  security budget @{horizon}y     : {lo:>18,.0f}  ..  {hi:>18,.0f} MBO")
        clamped = {
            (o.initial_annual_issuance, o.step_years, o.step_factor)
            for o in rows
            if o.clamped
        }
        out.append(f"  schedules clamped         : {len(clamped)}")
        if clamped:
            years = [int(o.first_clamp_year) for o in rows if o.clamped]
            out.append(f"  first clamp year range    : {min(years)} .. {max(years)}")
            over = [
                float(o.cumulative_issuance_over_cap_ratio)
                for o in final
                if o.cumulative_issuance_over_cap_ratio
            ]
            out.append(
                f"  cumulative issuance / cap : {min(over):.3f}x .. {max(over):.3f}x "
                "(above 1.0 means burns reopened headroom)"
            )
        out.extend(fee_funding_lines(rows, "fee funding infeasible", horizon))
        out.append("")

    stressed = [o for o in observations if o.stress != "none"]
    if stressed:
        out.append("--- stress layer (representative shapes only) ---")
        out.append(f"  shapes: {', '.join(sorted(REPRESENTATIVE_SHAPES))}")
        out.append(f"  stresses: {', '.join(STRESS_SOURCES)}")
        out.append(f"  observations: {len(stressed)}")
        for matrix in MATRICES:
            every = [o for o in stressed if o.matrix == matrix]
            out.extend(horizon_lines(every, f"horizon, {matrix}"))
            rows = comparable(every)
            if not rows:
                out.append(f"  {matrix}: no run is defined through year {horizon}, withheld")
                continue
            out.extend(fee_funding_lines(rows, f"fee funding infeasible, {matrix}", horizon))
        out.append("")

    out.append("Observation only. Interpretation, ranking and policy selection are out of scope.")
    return "\n".join(out)


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="MBO primary-issuance sensitivity study (research only)."
    )
    parser.add_argument("--config", type=Path, default=model.DEFAULT_CONFIG)
    parser.add_argument("--output", type=Path, help="write all observations to this CSV")
    parser.add_argument("--summary", action="store_true", help="print the factual summary")
    parser.add_argument("--no-stress", action="store_true", help="skip the stress layer")
    parser.add_argument(
        "--horizon", type=int, choices=SUPPORTED_HORIZONS, default=HORIZON_YEARS,
        help="study horizon in years (default 100, the historical study)",
    )
    args = parser.parse_args(argv)

    try:
        config = model.load_config(args.config)
        observations = run_core_matrices(config, args.horizon)
        if not args.no_stress:
            observations.extend(run_stress_layer(config, args.horizon))
    except (model.ConfigError, RuntimeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    if args.summary or not args.output:
        print(format_summary(observations, args.horizon))
    if args.output:
        write_csv(observations, args.output)
        print(f"\n{len(observations)} observations written to {args.output}")
    # Every observation is written, for inspection. Exit 4 says some run went
    # beyond what its inputs define, as model.py does for a single run.
    # Infeasible runs keep exiting 0 here: a sweep is expected to contain some,
    # and they are flagged in the output.
    if extrapolated_run_count(observations):
        return model.EXIT_CODES[model.RUN_EXTRAPOLATED]
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
