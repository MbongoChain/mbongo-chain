#!/usr/bin/env python3
"""Deterministic long-horizon simulation of candidate MBO monetary policies.

This is a research tool. It is NOT part of the protocol, it does not read or
write chain state, and nothing it computes is normative. It exists so that
candidate monetary policies can be compared over a long horizon on identical
assumptions, instead of being argued about with incompatible mental models.

It does not predict the market price of MBO. Price is an exogenous scenario
input, chosen by whoever writes the scenario.

Standard library only, no randomness: the same model version plus the same
scenario plus the same horizon always produces the same output.

Usage:
    python3 model.py --list
    python3 model.py --scenario base --years 100
    python3 model.py --scenario base --years 100 --output /tmp/base.csv
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from dataclasses import dataclass, asdict, fields
from pathlib import Path
from typing import Any, Dict, List, Optional

MODEL_VERSION = "0.1.0-research"
DEFAULT_CONFIG = Path(__file__).with_name("scenarios.json")

# Relative tolerance used when the model checks its own accounting identities.
IDENTITY_TOLERANCE = 1e-9


class ConfigError(ValueError):
    """A scenario configuration is internally inconsistent or out of range."""


# ── Output row ──────────────────────────────────────────────────────────


@dataclass(frozen=True)
class YearRow:
    """One simulated year. Every field is either MBO or a pure ratio, except
    the two fields whose name ends in ``reference_value``/``reference_price``,
    which are denominated in the scenario's arbitrary reference unit."""

    year: int
    starting_supply: float
    gross_issuance: float
    burned: float
    ending_supply: float
    gross_inflation_rate: float
    net_inflation_rate: float
    circulating_supply: float
    staked_supply: float
    validator_issuance_revenue: float
    validator_fee_revenue: float
    security_budget_mbo: float
    mbo_reference_price: float
    security_budget_reference_value: float
    compute_user_spend: float
    worker_revenue: float
    protocol_compute_fee: float
    transaction_fee_revenue: float
    treasury_revenue: float
    security_value_at_risk: float


CSV_COLUMNS = [f.name for f in fields(YearRow)]


# ── Configuration helpers ───────────────────────────────────────────────


def _require(cond: bool, message: str) -> None:
    if not cond:
        raise ConfigError(message)


def _ratio(value: Any, name: str) -> float:
    """A share in [0, 1]."""
    _require(isinstance(value, (int, float)), f"{name} must be a number")
    value = float(value)
    _require(0.0 <= value <= 1.0, f"{name} must be within [0, 1], got {value}")
    return value


def _non_negative(value: Any, name: str) -> float:
    _require(isinstance(value, (int, float)), f"{name} must be a number")
    value = float(value)
    _require(value >= 0.0, f"{name} must be >= 0, got {value}")
    return value


def growth_path(spec: Dict[str, Any], years: int, name: str) -> List[float]:
    """Expand a piecewise-growth specification into one value per year 0..years.

    A path is an initial value plus ordered phases, each with a `until_year`
    and an `annual_growth` that may be negative (contraction) — so a scenario
    can express growth, maturity, stagnation and decline without the engine
    assuming any of them. Years beyond the last phase keep the last phase's
    growth rate.
    """
    initial = _non_negative(spec.get("initial", 0.0), f"{name}.initial")
    phases = spec.get("phases", [])
    _require(isinstance(phases, list), f"{name}.phases must be a list")

    last_until = -1
    for i, phase in enumerate(phases):
        until = phase.get("until_year")
        _require(
            isinstance(until, int) and until >= 0,
            f"{name}.phases[{i}].until_year must be a non-negative integer",
        )
        _require(
            until > last_until,
            f"{name}.phases must have strictly increasing until_year",
        )
        last_until = until
        growth = phase.get("annual_growth", 0.0)
        _require(
            isinstance(growth, (int, float)) and growth > -1.0,
            f"{name}.phases[{i}].annual_growth must be a number > -1",
        )

    def growth_for(year: int) -> float:
        for phase in phases:
            if year <= phase["until_year"]:
                return float(phase.get("annual_growth", 0.0))
        return float(phases[-1].get("annual_growth", 0.0)) if phases else 0.0

    path = [initial]
    for year in range(1, years + 1):
        path.append(path[-1] * (1.0 + growth_for(year)))
    return path


def apply_shocks(
    path: List[float], shocks: List[Dict[str, Any]], metric: str
) -> List[float]:
    """Multiply a metric's path inside each shock's configured window only.

    Windows are inclusive and validated, so a shock cannot silently leak into
    years it was not configured for.
    """
    out = list(path)
    for i, shock in enumerate(shocks):
        if shock.get("metric") != metric:
            continue
        start = shock.get("start_year")
        end = shock.get("end_year")
        _require(
            isinstance(start, int) and isinstance(end, int),
            f"shocks[{i}] start_year/end_year must be integers",
        )
        _require(start >= 0, f"shocks[{i}].start_year must be >= 0")
        _require(end >= start, f"shocks[{i}].end_year must be >= start_year")
        multiplier = shock.get("multiplier")
        _require(
            isinstance(multiplier, (int, float)) and multiplier >= 0,
            f"shocks[{i}].multiplier must be a number >= 0",
        )
        for year in range(start, min(end, len(out) - 1) + 1):
            out[year] *= float(multiplier)
    return out


# ── Monetary policy families ────────────────────────────────────────────


class MonetaryPolicy:
    """Computes gross issuance for a year. Subclasses differ only in that."""

    def __init__(self, cfg: Dict[str, Any]) -> None:
        self.cfg = cfg
        primary = cfg.get("primary_emission", {})
        self.initial_annual_issuance = _non_negative(
            primary.get("initial_annual_issuance", 0.0),
            "monetary.primary_emission.initial_annual_issuance",
        )
        step_years = primary.get("step_years", 5)
        _require(
            isinstance(step_years, int) and step_years >= 1,
            "monetary.primary_emission.step_years must be an integer >= 1",
        )
        self.step_years = step_years
        self.step_factor = _ratio(
            primary.get("step_factor", 0.5),
            "monetary.primary_emission.step_factor",
        )
        phase_end = primary.get("phase_end_year")
        _require(
            phase_end is None or (isinstance(phase_end, int) and phase_end >= 0),
            "monetary.primary_emission.phase_end_year must be a non-negative integer or null",
        )
        self.phase_end_year: Optional[int] = phase_end

        cap = cfg.get("cap")
        _require(
            cap is None or (isinstance(cap, (int, float)) and cap >= 0),
            "monetary.cap must be a non-negative number or null",
        )
        self.cap: Optional[float] = None if cap is None else float(cap)

    def primary_issuance(self, year: int) -> float:
        """Stepped issuance: the initial amount, multiplied by `step_factor`
        once per completed `step_years` block. With factor 0.5 this is a
        halving schedule; other factors express other decay speeds."""
        if year < 1:
            return 0.0
        if self.phase_end_year is not None and year > self.phase_end_year:
            return 0.0
        step = (year - 1) // self.step_years
        return self.initial_annual_issuance * (self.step_factor ** step)

    def gross_issuance(self, year: int, supply: float, context: Dict[str, float]) -> float:
        raise NotImplementedError

    def clamp_to_cap(self, supply: float, issuance: float) -> float:
        """A configured cap is a hard ceiling on total supply, never exceeded."""
        if self.cap is None:
            return issuance
        headroom = max(0.0, self.cap - supply)
        return min(issuance, headroom)


class HardCapPolicy(MonetaryPolicy):
    """Primary emission only; issuance stops when the phase ends or the cap
    is reached. Nothing is issued afterwards."""

    def gross_issuance(self, year: int, supply: float, context: Dict[str, float]) -> float:
        return self.clamp_to_cap(supply, self.primary_issuance(year))


class TailPolicy(MonetaryPolicy):
    """Shared behaviour for the two tail families: primary emission until the
    tail starts, then a tail defined by the subclass."""

    def __init__(self, cfg: Dict[str, Any]) -> None:
        super().__init__(cfg)
        tail = cfg.get("tail", {})
        start = tail.get("start_year")
        _require(
            isinstance(start, int) and start >= 1,
            "monetary.tail.start_year must be an integer >= 1",
        )
        self.tail_start_year = start
        self.tail_cfg = tail

    def tail_issuance(self, supply: float) -> float:
        raise NotImplementedError

    def gross_issuance(self, year: int, supply: float, context: Dict[str, float]) -> float:
        if year < self.tail_start_year:
            issuance = self.primary_issuance(year)
        else:
            issuance = self.tail_issuance(supply)
        return self.clamp_to_cap(supply, issuance)


class FixedTailPolicy(TailPolicy):
    """A constant number of MBO per year once the tail starts."""

    def __init__(self, cfg: Dict[str, Any]) -> None:
        super().__init__(cfg)
        self.annual_amount = _non_negative(
            self.tail_cfg.get("annual_amount", 0.0), "monetary.tail.annual_amount"
        )

    def tail_issuance(self, supply: float) -> float:
        return self.annual_amount


class PercentageTailPolicy(TailPolicy):
    """A constant fraction of current supply per year once the tail starts."""

    def __init__(self, cfg: Dict[str, Any]) -> None:
        super().__init__(cfg)
        self.annual_rate = _ratio(
            self.tail_cfg.get("annual_rate", 0.0), "monetary.tail.annual_rate"
        )

    def tail_issuance(self, supply: float) -> float:
        return supply * self.annual_rate


class AdaptiveBoundedPolicy(TailPolicy):
    """RESEARCH ONLY. Issuance moves within an explicit [min_rate, max_rate]
    band, increasing when the modelled security budget is below a target
    share of staked value and decreasing when it is above.

    This is a comparison device, not a governance proposal: it defines no
    on-chain measurement, no activation path and no controller design. It
    exists so that a bounded-adaptive family can be plotted against the
    fixed families rather than dismissed without evidence.
    """

    def __init__(self, cfg: Dict[str, Any]) -> None:
        super().__init__(cfg)
        adaptive = cfg.get("adaptive", {})
        self.min_rate = _ratio(adaptive.get("min_rate", 0.0), "monetary.adaptive.min_rate")
        self.max_rate = _ratio(adaptive.get("max_rate", 0.0), "monetary.adaptive.max_rate")
        _require(
            self.min_rate <= self.max_rate,
            "monetary.adaptive.min_rate must be <= max_rate",
        )
        self.target_ratio = _non_negative(
            adaptive.get("target_security_ratio", 0.0),
            "monetary.adaptive.target_security_ratio",
        )

    def tail_issuance(self, supply: float) -> float:
        raise NotImplementedError  # uses the context-aware path below

    def gross_issuance(self, year: int, supply: float, context: Dict[str, float]) -> float:
        if year < self.tail_start_year:
            return self.clamp_to_cap(supply, self.primary_issuance(year))
        staked_value = context.get("previous_staked_value", 0.0)
        budget_value = context.get("previous_security_budget_value", 0.0)
        if staked_value <= 0.0:
            rate = self.max_rate
        else:
            observed = budget_value / staked_value
            rate = self.max_rate if observed < self.target_ratio else self.min_rate
        return self.clamp_to_cap(supply, supply * rate)


POLICY_FAMILIES = {
    "hard_cap": HardCapPolicy,
    "fixed_tail": FixedTailPolicy,
    "percentage_tail": PercentageTailPolicy,
    "adaptive_bounded": AdaptiveBoundedPolicy,
}


def build_policy(monetary_cfg: Dict[str, Any]) -> MonetaryPolicy:
    family = monetary_cfg.get("family")
    _require(
        family in POLICY_FAMILIES,
        f"monetary.family must be one of {sorted(POLICY_FAMILIES)}, got {family!r}",
    )
    return POLICY_FAMILIES[family](monetary_cfg)


# ── Simulation ──────────────────────────────────────────────────────────


def simulate(scenario: Dict[str, Any], years: int) -> List[YearRow]:
    """Run one scenario for years 0..`years` inclusive.

    Year 0 is the initial observation: it records the starting state and
    issues nothing, so a run over N years produces N+1 rows.
    """
    _require(isinstance(years, int) and years >= 1, f"years must be an integer >= 1, got {years}")

    monetary = scenario.get("monetary", {})
    policy = build_policy(monetary)
    supply = _non_negative(monetary.get("initial_supply", 0.0), "monetary.initial_supply")
    if policy.cap is not None:
        _require(
            supply <= policy.cap,
            "monetary.initial_supply must not exceed monetary.cap",
        )

    dist = scenario.get("supply_distribution", {})
    circulating_fraction = _ratio(
        dist.get("circulating_fraction", 1.0), "supply_distribution.circulating_fraction"
    )
    staking_ratio = _ratio(dist.get("staking_ratio", 0.0), "supply_distribution.staking_ratio")

    issuance_alloc = scenario.get("issuance_allocation", {})
    validator_issuance_share = _ratio(
        issuance_alloc.get("validator_share", 0.0), "issuance_allocation.validator_share"
    )

    fees = scenario.get("fees", {})
    fee_validator = _ratio(fees.get("validator_share", 0.0), "fees.validator_share")
    fee_treasury = _ratio(fees.get("treasury_share", 0.0), "fees.treasury_share")
    fee_burn = _ratio(fees.get("burn_share", 0.0), "fees.burn_share")
    _require(
        math.isclose(fee_validator + fee_treasury + fee_burn, 1.0, rel_tol=IDENTITY_TOLERANCE),
        "fees.validator_share + treasury_share + burn_share must sum to 1",
    )

    activity = scenario.get("activity", {})
    worker_share = _ratio(
        activity.get("worker_revenue_share", 1.0), "activity.worker_revenue_share"
    )

    shocks = scenario.get("shocks", [])
    _require(isinstance(shocks, list), "shocks must be a list")

    tx_fee_path = apply_shocks(
        growth_path(
            activity.get("transaction_fee_revenue_mbo", {}), years, "activity.transaction_fee_revenue_mbo"
        ),
        shocks,
        "transaction_fee_revenue_mbo",
    )
    compute_spend_path = apply_shocks(
        growth_path(
            activity.get("compute_user_spend_mbo", {}), years, "activity.compute_user_spend_mbo"
        ),
        shocks,
        "compute_user_spend_mbo",
    )
    price_path = apply_shocks(
        growth_path(scenario.get("price", {}), years, "price"), shocks, "mbo_reference_price"
    )
    staking_path = apply_shocks([staking_ratio] * (years + 1), shocks, "staking_ratio")

    rows: List[YearRow] = []
    previous_staked_value = 0.0
    previous_budget_value = 0.0

    for year in range(0, years + 1):
        starting_supply = supply

        if year == 0:
            gross_issuance = 0.0
            tx_fee_revenue = 0.0
            compute_user_spend = 0.0
        else:
            context = {
                "previous_staked_value": previous_staked_value,
                "previous_security_budget_value": previous_budget_value,
            }
            gross_issuance = policy.gross_issuance(year, starting_supply, context)
            tx_fee_revenue = tx_fee_path[year]
            compute_user_spend = compute_spend_path[year]

        # Compute market: what users spend is split between workers and the
        # protocol. These are the only two destinations, so they always sum
        # back to the user spend.
        protocol_compute_fee_rate = _ratio(
            activity.get("protocol_compute_fee_rate", 0.0), "activity.protocol_compute_fee_rate"
        )
        protocol_compute_fee = compute_user_spend * protocol_compute_fee_rate
        worker_revenue = compute_user_spend - protocol_compute_fee
        # `worker_revenue_share` lets a scenario model workers keeping less
        # than the whole remainder (the rest is treated as protocol fee too).
        withheld = worker_revenue * (1.0 - worker_share)
        worker_revenue -= withheld
        protocol_compute_fee += withheld

        # Fee pool: transaction fees plus the protocol's cut of compute.
        # Split once, three ways, so nothing is counted twice.
        fee_pool = tx_fee_revenue + protocol_compute_fee
        validator_fee_revenue = fee_pool * fee_validator
        treasury_revenue = fee_pool * fee_treasury
        burned = fee_pool * fee_burn

        ending_supply = starting_supply + gross_issuance - burned
        # Burn is capped by what exists: a scenario cannot burn supply into
        # negative territory.
        if ending_supply < 0.0:
            burned = starting_supply + gross_issuance
            ending_supply = 0.0

        validator_issuance_revenue = gross_issuance * validator_issuance_share
        security_budget_mbo = validator_issuance_revenue + validator_fee_revenue

        circulating_supply = ending_supply * circulating_fraction
        year_staking_ratio = min(max(staking_path[year], 0.0), 1.0)
        staked_supply = circulating_supply * year_staking_ratio

        price = price_path[year]
        security_budget_reference_value = security_budget_mbo * price
        # Conservative, explicitly named: the reference value of stake that
        # could be economically exposed. It is NOT a cost-of-attack figure.
        security_value_at_risk = staked_supply * price

        gross_inflation_rate = gross_issuance / starting_supply if starting_supply > 0 else 0.0
        net_inflation_rate = (
            (gross_issuance - burned) / starting_supply if starting_supply > 0 else 0.0
        )

        rows.append(
            YearRow(
                year=year,
                starting_supply=starting_supply,
                gross_issuance=gross_issuance,
                burned=burned,
                ending_supply=ending_supply,
                gross_inflation_rate=gross_inflation_rate,
                net_inflation_rate=net_inflation_rate,
                circulating_supply=circulating_supply,
                staked_supply=staked_supply,
                validator_issuance_revenue=validator_issuance_revenue,
                validator_fee_revenue=validator_fee_revenue,
                security_budget_mbo=security_budget_mbo,
                mbo_reference_price=price,
                security_budget_reference_value=security_budget_reference_value,
                compute_user_spend=compute_user_spend,
                worker_revenue=worker_revenue,
                protocol_compute_fee=protocol_compute_fee,
                transaction_fee_revenue=tx_fee_revenue,
                treasury_revenue=treasury_revenue,
                security_value_at_risk=security_value_at_risk,
            )
        )

        supply = ending_supply
        previous_staked_value = security_value_at_risk
        previous_budget_value = security_budget_reference_value

    return rows


# ── Accounting identities ───────────────────────────────────────────────


def identity_violations(rows: List[YearRow]) -> List[str]:
    """Return every accounting identity broken by a run. Empty means clean.

    Kept next to the model rather than only in the tests so a scenario author
    can assert them on their own configuration.
    """
    problems: List[str] = []
    for i, row in enumerate(rows):
        expected_end = row.starting_supply + row.gross_issuance - row.burned
        if not math.isclose(row.ending_supply, expected_end, rel_tol=IDENTITY_TOLERANCE, abs_tol=1e-6):
            problems.append(
                f"year {row.year}: ending_supply {row.ending_supply} != "
                f"starting + issuance - burned ({expected_end})"
            )
        spend = row.worker_revenue + row.protocol_compute_fee
        if not math.isclose(spend, row.compute_user_spend, rel_tol=IDENTITY_TOLERANCE, abs_tol=1e-6):
            problems.append(
                f"year {row.year}: worker_revenue + protocol_compute_fee != compute_user_spend"
            )
        budget = row.validator_issuance_revenue + row.validator_fee_revenue
        if not math.isclose(budget, row.security_budget_mbo, rel_tol=IDENTITY_TOLERANCE, abs_tol=1e-6):
            problems.append(f"year {row.year}: security_budget_mbo does not match its components")
        if row.ending_supply < 0:
            problems.append(f"year {row.year}: negative ending_supply")
        if row.staked_supply > row.circulating_supply + 1e-6:
            problems.append(f"year {row.year}: staked_supply exceeds circulating_supply")
        if row.circulating_supply > row.ending_supply + 1e-6:
            problems.append(f"year {row.year}: circulating_supply exceeds ending_supply")
        if i > 0 and not math.isclose(
            row.starting_supply, rows[i - 1].ending_supply, rel_tol=IDENTITY_TOLERANCE, abs_tol=1e-6
        ):
            problems.append(f"year {row.year}: starting_supply does not continue previous year")
        for name, value in asdict(row).items():
            if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
                problems.append(f"year {row.year}: {name} is not finite")
    return problems


# ── Configuration loading ───────────────────────────────────────────────


def load_config(path: Path) -> Dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    _require("scenarios" in data, "configuration must contain a 'scenarios' object")
    return data


def get_scenario(config: Dict[str, Any], name: str) -> Dict[str, Any]:
    scenarios = config["scenarios"]
    if name not in scenarios:
        raise ConfigError(
            f"unknown scenario {name!r}; available: {', '.join(sorted(scenarios))}"
        )
    return scenarios[name]


# ── Reporting ───────────────────────────────────────────────────────────

SUMMARY_YEARS = (1, 5, 10, 20, 30, 50, 75, 100)


def format_summary(name: str, scenario: Dict[str, Any], rows: List[YearRow]) -> str:
    horizon = rows[-1].year
    out: List[str] = []
    out.append(f"=== MBO monetary policy simulation — scenario '{name}' ===")
    out.append(f"model version: {MODEL_VERSION}   horizon: {horizon} years   rows: {len(rows)}")
    out.append(f"family: {scenario.get('monetary', {}).get('family')}")
    out.append(f"description: {scenario.get('description', '(none)')}")
    out.append("NON-NORMATIVE: illustrative parameters, no policy is recommended.")
    out.append("")
    header = (
        f"{'year':>5} {'ending_supply':>16} {'gross_issuance':>15} {'burned':>12} "
        f"{'net_infl':>10} {'sec_budget_MBO':>15} {'sec_budget_ref':>15}"
    )
    out.append(header)
    out.append("-" * len(header))
    for row in rows:
        if row.year in SUMMARY_YEARS or row.year == horizon:
            out.append(
                f"{row.year:>5} {row.ending_supply:>16,.2f} {row.gross_issuance:>15,.2f} "
                f"{row.burned:>12,.2f} {row.net_inflation_rate:>9.4%} "
                f"{row.security_budget_mbo:>15,.2f} {row.security_budget_reference_value:>15,.2f}"
            )
    final = rows[-1]
    out.append("")
    out.append(f"final ending_supply         : {final.ending_supply:,.2f} MBO")
    out.append(f"cumulative gross issuance   : {sum(r.gross_issuance for r in rows):,.2f} MBO")
    out.append(f"cumulative burned           : {sum(r.burned for r in rows):,.2f} MBO")
    problems = identity_violations(rows)
    out.append(f"accounting identities       : {'OK' if not problems else 'VIOLATED'}")
    for problem in problems[:5]:
        out.append(f"  - {problem}")
    return "\n".join(out)


def write_csv(rows: List[YearRow], path: Path) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_COLUMNS)
        writer.writeheader()
        for row in rows:
            record = asdict(row)
            # Fixed precision keeps a rerun byte-identical rather than
            # depending on float repr.
            writer.writerow(
                {
                    key: (f"{value:.10f}" if isinstance(value, float) else value)
                    for key, value in record.items()
                }
            )


# ── CLI ─────────────────────────────────────────────────────────────────


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Deterministic MBO monetary policy simulation (research only)."
    )
    parser.add_argument("--scenario", default="base", help="scenario id from the config file")
    parser.add_argument("--years", type=int, default=100, help="horizon in years (default 100)")
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG, help="scenario config path")
    parser.add_argument("--output", type=Path, help="write per-year results to this CSV path")
    parser.add_argument("--list", action="store_true", help="list available scenarios and exit")
    args = parser.parse_args(argv)

    try:
        config = load_config(args.config)
    except (OSError, json.JSONDecodeError) as exc:
        print(f"error: cannot read config {args.config}: {exc}", file=sys.stderr)
        return 2

    if args.list:
        for name in sorted(config["scenarios"]):
            scenario = config["scenarios"][name]
            family = scenario.get("monetary", {}).get("family", "?")
            print(f"{name:<24} {family:<18} {scenario.get('description', '')}")
        return 0

    try:
        scenario = get_scenario(config, args.scenario)
        rows = simulate(scenario, args.years)
    except ConfigError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    print(format_summary(args.scenario, scenario, rows))

    if args.output:
        write_csv(rows, args.output)
        print(f"\nCSV written to {args.output}")

    return 1 if identity_violations(rows) else 0


if __name__ == "__main__":
    raise SystemExit(main())
