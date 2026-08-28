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
HORIZON_YEARS = 100

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
) -> List[Observation]:
    cap = scenario["monetary"]["cap"]
    clamp_year = detect_clamp(scenario, rows)
    cumulative = 0.0
    by_year: Dict[int, Tuple[model.YearRow, float]] = {}
    for row in rows:
        cumulative += row.gross_issuance
        by_year[row.year] = (row, cumulative)

    out: List[Observation] = []
    for year in CHECKPOINTS:
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
            )
        )
    return out


def run_core_matrices(config: Dict[str, Any]) -> List[Observation]:
    """Both matrices over the same grid, in deterministic order."""
    base = controlled_scenario(config)
    out: List[Observation] = []
    for matrix, cap in MATRICES.items():
        for schedule in schedule_grid():
            scenario = build_scenario(base, schedule, cap)
            rows = model.simulate(scenario, HORIZON_YEARS)
            problems = model.identity_violations(rows)
            if problems:
                raise RuntimeError(f"{matrix} {schedule}: {problems[0]}")
            out.extend(observations_for(matrix, schedule, scenario, rows, stress="none"))
    return out


def run_stress_layer(config: Dict[str, Any]) -> List[Observation]:
    """Existing stress mechanisms applied to four representative shapes."""
    base = controlled_scenario(config)
    out: List[Observation] = []
    for stress_name in STRESS_SOURCES:
        shocks = model.get_scenario(config, stress_name).get("shocks", [])
        for _, schedule in sorted(REPRESENTATIVE_SHAPES.items()):
            for matrix, cap in MATRICES.items():
                scenario = build_scenario(base, schedule, cap, shocks=shocks)
                rows = model.simulate(scenario, HORIZON_YEARS)
                problems = model.identity_violations(rows)
                if problems:
                    raise RuntimeError(f"{stress_name} {matrix} {schedule}: {problems[0]}")
                out.extend(
                    observations_for(matrix, schedule, scenario, rows, stress=stress_name)
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


def format_summary(observations: List[Observation]) -> str:
    """Factual ranges, reported per matrix and never combined."""
    out: List[str] = []
    out.append("=== MBO primary-issuance sensitivity study ===")
    out.append(
        f"study version: {STUDY_VERSION}   horizon: {HORIZON_YEARS} years   "
        f"grid: {len(schedule_grid())} schedules per matrix"
    )
    out.append(f"controlled scenario: {CONTROLLED_SCENARIO} (only schedule and cap replaced)")
    out.append("SENSITIVITY ANALYSIS, NOT OPTIMIZATION. No schedule is ranked or recommended.")
    out.append("")

    core = [o for o in observations if o.stress == "none"]
    for matrix in MATRICES:
        rows = [o for o in core if o.matrix == matrix]
        final = [o for o in rows if o.year == HORIZON_YEARS]
        out.append(f"--- {matrix} ---")
        out.append(f"  schedules: {len({(o.initial_annual_issuance, o.step_years, o.step_factor) for o in rows})}")
        lo, hi = _range([o.cumulative_issuance for o in final])
        out.append(f"  cumulative issuance @100y : {lo:>18,.0f}  ..  {hi:>18,.0f} MBO")
        lo, hi = _range([o.ending_supply for o in final])
        out.append(f"  ending supply @100y       : {lo:>18,.0f}  ..  {hi:>18,.0f} MBO")
        distinct = len({round(o.cumulative_issuance, 2) for o in final})
        out.append(f"  distinct cumulative totals: {distinct} of {len(final)}")
        y10 = [o for o in rows if o.year == 10]
        lo, hi = _range([o.gross_inflation_rate for o in y10])
        out.append(f"  gross inflation @10y      : {lo:>17.4%}  ..  {hi:>17.4%}")
        lo, hi = _range([o.net_inflation_rate for o in final])
        out.append(f"  net inflation @100y       : {lo:>17.4%}  ..  {hi:>17.4%}")
        lo, hi = _range([o.security_budget_mbo for o in final])
        out.append(f"  security budget @100y     : {lo:>18,.0f}  ..  {hi:>18,.0f} MBO")
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
        out.append("")

    stressed = [o for o in observations if o.stress != "none"]
    if stressed:
        out.append("--- stress layer (representative shapes only) ---")
        out.append(f"  shapes: {', '.join(sorted(REPRESENTATIVE_SHAPES))}")
        out.append(f"  stresses: {', '.join(STRESS_SOURCES)}")
        out.append(f"  observations: {len(stressed)}")
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
    args = parser.parse_args(argv)

    try:
        config = model.load_config(args.config)
        observations = run_core_matrices(config)
        if not args.no_stress:
            observations.extend(run_stress_layer(config))
    except (model.ConfigError, RuntimeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    if args.summary or not args.output:
        print(format_summary(observations))
    if args.output:
        write_csv(observations, args.output)
        print(f"\n{len(observations)} observations written to {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
