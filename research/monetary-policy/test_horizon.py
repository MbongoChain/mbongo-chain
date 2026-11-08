#!/usr/bin/env python3
"""Regression tests for horizon validity.

Accounting says the books balance and fee funding that the fees could have been
paid. Horizon coverage says that the inputs producing a year were stated by the
scenario, not continued by the engine. A run can pass the first two and fail
the third: before these checks, every shipped scenario ran to 150 or 200 years
without a warning, and `historical_documented_partial` was reported infeasible
from year 104 purely because the engine kept growing its fees after year 100.

These tests pin the coverage rules, the declarations that extend coverage, the
precedence between invalid states and the exclusion of extrapolated runs from
study ranges. They assert no economic outcome as desirable.

Declarations made in the fixtures below exist only to exercise the mechanism.
They are not research assumptions and not proposed values.

Run from the repository root:

    python3 research/monetary-policy/test_horizon.py
"""

from __future__ import annotations

import contextlib
import copy
import dataclasses
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import model  # noqa: E402
import primary_sensitivity as ps  # noqa: E402
import cap_semantics as cs  # noqa: E402


CONFIG = model.load_config(model.DEFAULT_CONFIG)
SCENARIOS = sorted(CONFIG["scenarios"])
PATH_METRICS = ("transaction_fee_revenue_mbo", "compute_user_spend_mbo", "mbo_reference_price")
STRESS = ("stress_price_shock", "stress_compute_contraction", "stress_combined")

COVERED = model.HORIZON_COVERED
EXTRAPOLATED = model.HORIZON_EXTRAPOLATED


def scenario(name):
    return copy.deepcopy(model.get_scenario(CONFIG, name))


def path_specs(s):
    return (
        s["activity"]["transaction_fee_revenue_mbo"],
        s["activity"]["compute_user_spend_mbo"],
        s["price"],
    )


def with_continuation(s, continuation=None, only=None):
    """TEST FIXTURE: declare a continuation on the paths of a scenario."""
    s = copy.deepcopy(s)
    continuation = continuation or {"policy": model.CONTINUATION_CONSTANT}
    for metric, spec in zip(PATH_METRICS, path_specs(s)):
        if only is None or metric in only:
            spec["continuation"] = dict(continuation)
    return s


def declared_config(shock_after_end=None):
    """TEST FIXTURE: the shipped config, with constant continuations on `base`
    (the controlled scenario of both studies) and, optionally, a declared
    after-end behaviour on every stress shock."""
    cfg = copy.deepcopy(CONFIG)
    cfg["scenarios"]["base"] = with_continuation(cfg["scenarios"]["base"])
    if shock_after_end is not None:
        for name in STRESS:
            for shock in cfg["scenarios"][name]["shocks"]:
                shock["after_end"] = shock_after_end
    return cfg


def velocity_only_scenario():
    """The fixture of test_fee_funding: fees far above supply, none burned."""
    s = scenario("base")
    s["fees"] = {"validator_share": 0.8, "treasury_share": 0.2, "burn_share": 0.0}
    s["activity"]["transaction_fee_revenue_mbo"] = {"initial": 1.0e12, "phases": []}
    return s


def floats(row):
    return {k: v for k, v in dataclasses.asdict(row).items() if isinstance(v, float)}


class ShippedScenarioCoverageTests(unittest.TestCase):
    def test_every_input_of_the_eleven_scenarios_is_defined_through_year_100(self):
        coverage = {name: model.input_coverage(scenario(name)) for name in SCENARIOS}
        self.assertEqual(len(coverage), 11)
        self.assertEqual(sum(len(c) for c in coverage.values()), 44)
        for name, c in coverage.items():
            with self.subTest(scenario=name):
                self.assertEqual(tuple(c), model.TRAJECTORY_METRICS)
                self.assertEqual([c[m] for m in PATH_METRICS], [100, 100, 100])
                # A single number is time-invariant: only a shock limits it.
                self.assertEqual(c["staking_ratio"], 100 if name == "stress_combined" else None)

    def test_hundred_year_runs_are_covered_in_every_year(self):
        rows = [r for name in SCENARIOS for r in model.simulate(scenario(name), 100)]
        self.assertEqual(len(rows), 11 * 101)
        self.assertEqual({r.horizon_status for r in rows}, {COVERED})
        self.assertEqual({r.extrapolated_inputs for r in rows}, {""})

    def test_hundred_year_validity_is_what_it_was(self):
        status = {n: model.run_validity(model.simulate(scenario(n), 100)) for n in SCENARIOS}
        self.assertEqual(
            {n: v.status for n, v in status.items() if v.status != model.RUN_VALID},
            {"high_adoption": model.RUN_INFEASIBLE},
        )
        self.assertEqual(
            {n: v.exit_code for n, v in status.items() if v.exit_code}, {"high_adoption": 3}
        )

    def test_longer_horizons_are_extrapolated_from_year_101(self):
        for years in (150, 200):
            for name in SCENARIOS:
                with self.subTest(years=years, scenario=name):
                    rows = model.simulate(scenario(name), years)
                    v = model.run_validity(rows)
                    self.assertEqual(v.horizon_status, EXTRAPOLATED)
                    self.assertEqual(v.first_extrapolated_year, 101)
                    # Coverage ends once per input, so covered years are a prefix.
                    self.assertEqual([r.year for r in model.covered_rows(rows)], list(range(101)))
                    expected = PATH_METRICS + (("staking_ratio",) if name == "stress_combined" else ())
                    self.assertEqual(
                        set(v.extrapolated_inputs), set(expected)
                    )
                    self.assertEqual(
                        v.status,
                        model.RUN_INFEASIBLE if name == "high_adoption" else model.RUN_EXTRAPOLATED,
                    )


class MissingCoverageTests(unittest.TestCase):
    def test_a_path_without_phases_is_defined_in_year_0_only(self):
        rows = model.simulate(velocity_only_scenario(), 20)
        self.assertEqual(model.input_coverage(velocity_only_scenario())["transaction_fee_revenue_mbo"], 0)
        self.assertEqual(model.first_extrapolated_year(rows), 1)
        self.assertEqual(model.extrapolated_inputs(rows), ("transaction_fee_revenue_mbo",))

    def test_an_absent_path_is_defined_in_year_0_only(self):
        s = scenario("base")
        del s["price"]
        self.assertEqual(model.input_coverage(s)["mbo_reference_price"], 0)
        self.assertEqual(model.first_extrapolated_year(model.simulate(s, 5)), 1)

    def test_a_path_ending_early_flags_only_that_input(self):
        s = with_continuation(scenario("base"))
        price = s["price"]
        del price["continuation"]
        price["phases"] = [{"until_year": 15, "annual_growth": 0.08}, {"until_year": 50, "annual_growth": 0.0}]
        rows = model.simulate(s, 60)
        self.assertEqual(model.first_extrapolated_year(rows), 51)
        self.assertEqual(model.extrapolated_inputs(rows), ("mbo_reference_price",))
        self.assertEqual(rows[50].horizon_status, COVERED)
        self.assertEqual(rows[51].extrapolated_inputs, "mbo_reference_price")

    def test_extrapolated_years_are_still_computed_as_before(self):
        # Outputs are preserved for inspection: the implicit continuation keeps
        # the last phase's growth, exactly as it did before it was flagged.
        rows = model.simulate(scenario("historical_documented_partial"), 150)
        self.assertAlmostEqual(
            rows[101].transaction_fee_revenue, rows[100].transaction_fee_revenue * 1.02, places=6
        )
        self.assertEqual(rows[101].horizon_status, EXTRAPOLATED)


class ContinuationTests(unittest.TestCase):
    def test_constant_continuation_covers_every_year(self):
        s = with_continuation(scenario("base"))
        self.assertEqual(model.input_coverage(s), {m: None for m in model.TRAJECTORY_METRICS})
        rows = model.simulate(s, 200)
        self.assertEqual(model.run_validity(rows).status, model.RUN_VALID)
        for year in (101, 150, 200):
            self.assertEqual(rows[year].transaction_fee_revenue, rows[100].transaction_fee_revenue)
            self.assertEqual(rows[year].mbo_reference_price, rows[100].mbo_reference_price)

    def test_growth_continuation_uses_the_declared_rate(self):
        s = with_continuation(
            scenario("base"), {"policy": model.CONTINUATION_GROWTH, "annual_growth": 0.01}
        )
        rows = model.simulate(s, 150)
        self.assertEqual(model.first_extrapolated_year(rows), None)
        self.assertAlmostEqual(
            rows[101].compute_user_spend, rows[100].compute_user_spend * 1.01, places=6
        )

    def test_a_declaration_replaces_the_implicit_rate_after_its_phases_only(self):
        implicit = model.simulate(scenario("historical_documented_partial"), 150)
        declared = model.simulate(with_continuation(scenario("historical_documented_partial")), 150)
        for a, b in zip(implicit[:101], declared[:101]):
            self.assertEqual(floats(a), floats(b))
        self.assertGreater(implicit[101].transaction_fee_revenue, declared[101].transaction_fee_revenue)
        self.assertEqual(declared[150].transaction_fee_revenue, declared[100].transaction_fee_revenue)

    def test_declarations_change_no_hundred_year_output(self):
        for name in SCENARIOS:
            with self.subTest(scenario=name):
                before = model.simulate(scenario(name), 100)
                after = model.simulate(with_continuation(scenario(name)), 100)
                self.assertEqual(
                    [dataclasses.asdict(r) for r in before], [dataclasses.asdict(r) for r in after]
                )

    def test_a_bounded_continuation_covers_its_years_only(self):
        s = with_continuation(
            scenario("base"), {"policy": model.CONTINUATION_CONSTANT, "until_year": 150}
        )
        self.assertEqual(model.run_validity(model.simulate(s, 150)).status, model.RUN_VALID)
        self.assertEqual(model.first_extrapolated_year(model.simulate(s, 200)), 151)

    def test_declaring_some_paths_leaves_the_others_extrapolated(self):
        s = with_continuation(scenario("base"), only=("mbo_reference_price",))
        rows = model.simulate(s, 150)
        self.assertEqual(
            model.extrapolated_inputs(rows),
            ("transaction_fee_revenue_mbo", "compute_user_spend_mbo"),
        )

    def test_invalid_continuations_are_rejected(self):
        bad = [
            "constant",
            {"policy": "hold_last_growth"},
            {"policy": model.CONTINUATION_GROWTH},
            {"policy": model.CONTINUATION_GROWTH, "annual_growth": -1.0},
            {"policy": model.CONTINUATION_GROWTH, "annual_growth": True},
            {"policy": model.CONTINUATION_CONSTANT, "annual_growth": 0.0},
            {"policy": model.CONTINUATION_CONSTANT, "until_year": 100},
            {"policy": model.CONTINUATION_CONSTANT, "until_year": 120.0},
            {"policy": model.CONTINUATION_CONSTANT, "rate": 0.0},
        ]
        for continuation in bad:
            with self.subTest(continuation=continuation):
                s = scenario("base")
                s["price"]["continuation"] = continuation
                with self.assertRaises(model.ConfigError):
                    model.simulate(s, 150)
                with self.assertRaises(model.ConfigError):
                    model.input_coverage(s)


class ShockExpirationTests(unittest.TestCase):
    def price_shock(self, **shock_fields):
        s = with_continuation(scenario("stress_price_shock"))
        s["shocks"][0].update(shock_fields)
        return s

    def test_a_shock_ending_at_the_last_phase_does_not_imply_recovery(self):
        # The paths are declared to continue; the shock says nothing after 100.
        s = self.price_shock()
        self.assertEqual(model.input_coverage(s)["mbo_reference_price"], 100)
        rows = model.simulate(s, 150)
        self.assertEqual(model.first_extrapolated_year(rows), 101)
        self.assertEqual(model.extrapolated_inputs(rows), ("mbo_reference_price",))
        # Computed as before, a fivefold jump at 101, but flagged.
        self.assertAlmostEqual(rows[101].mbo_reference_price, rows[100].mbo_reference_price * 5, places=9)

    def test_declared_recovery_is_covered(self):
        rows = model.simulate(self.price_shock(after_end=model.SHOCK_REVERT), 150)
        clean = model.simulate(with_continuation(scenario("stress_price_shock")) | {"shocks": []}, 150)
        self.assertEqual(model.first_extrapolated_year(rows), None)
        self.assertAlmostEqual(rows[100].mbo_reference_price, clean[100].mbo_reference_price * 0.2, places=9)
        self.assertEqual(rows[101].mbo_reference_price, clean[101].mbo_reference_price)

    def test_a_persistent_shock_applies_to_the_end_of_the_run(self):
        rows = model.simulate(self.price_shock(end_year=None), 150)
        clean = model.simulate(with_continuation(scenario("stress_price_shock")) | {"shocks": []}, 150)
        self.assertEqual(model.first_extrapolated_year(rows), None)
        for year in (20, 100, 101, 150):
            self.assertAlmostEqual(
                rows[year].mbo_reference_price, clean[year].mbo_reference_price * 0.2, places=9
            )
        self.assertEqual(rows[19].mbo_reference_price, clean[19].mbo_reference_price)

    def test_an_unspecified_end_limits_coverage_to_the_window(self):
        s = scenario("stress_compute_contraction")
        s["shocks"][0]["after_end"] = model.SHOCK_UNSPECIFIED
        self.assertEqual(model.input_coverage(s)["compute_user_spend_mbo"], 32)
        self.assertEqual(model.first_extrapolated_year(model.simulate(s, 100)), 33)

    def test_an_undeclared_window_inside_the_phases_returns_to_the_defined_path(self):
        s = scenario("stress_compute_contraction")
        self.assertEqual(s["shocks"][0]["end_year"], 32)
        self.assertEqual(model.input_coverage(s)["compute_user_spend_mbo"], 100)

    def test_the_staking_shock_ending_in_year_100_is_limited(self):
        s = with_continuation(scenario("stress_combined"))
        self.assertEqual(model.input_coverage(s)["staking_ratio"], 100)
        self.assertEqual(model.input_coverage(s)["mbo_reference_price"], 100)
        for shock in s["shocks"]:
            shock["after_end"] = model.SHOCK_REVERT
        self.assertEqual(model.input_coverage(s), {m: None for m in model.TRAJECTORY_METRICS})
        self.assertEqual(model.run_validity(model.simulate(s, 150)).status, model.RUN_VALID)

    def test_invalid_shock_declarations_are_rejected(self):
        for fields in (
            {"after_end": "recover"},
            {"end_year": None, "after_end": model.SHOCK_REVERT},
            {"end_year": "100"},
        ):
            with self.subTest(fields=fields):
                with self.assertRaises(model.ConfigError):
                    model.simulate(self.price_shock(**fields), 120)


class PrecedenceTests(unittest.TestCase):
    def corrupt(self, rows, year):
        rows = list(rows)
        rows[year] = dataclasses.replace(rows[year], ending_supply=rows[year].ending_supply + 1.0)
        return rows

    def test_order_and_exit_codes(self):
        self.assertEqual(
            model.RUN_PRECEDENCE,
            ("accounting_invalid", "infeasible", "extrapolated", "conditional", "valid"),
        )
        self.assertEqual(
            [model.EXIT_CODES[s] for s in model.RUN_PRECEDENCE], [1, 3, 4, 0, 0]
        )

    def test_accounting_beats_everything_and_the_rest_stays_inspectable(self):
        rows = self.corrupt(model.simulate(scenario("high_adoption"), 150), 120)
        v = model.run_validity(rows)
        self.assertEqual((v.status, v.exit_code), (model.RUN_ACCOUNTING_INVALID, 1))
        self.assertGreater(v.accounting_violations, 0)
        self.assertEqual(v.fee_funding_covered_years, model.FEE_FUNDING_INFEASIBLE)
        self.assertEqual(v.horizon_status, EXTRAPOLATED)

    def test_extrapolation_never_excuses_an_accounting_error(self):
        rows = self.corrupt(model.simulate(scenario("base"), 150), 140)
        self.assertEqual(rows[140].horizon_status, EXTRAPOLATED)
        self.assertEqual(model.run_validity(rows).status, model.RUN_ACCOUNTING_INVALID)

    def test_infeasibility_in_covered_years_is_not_hidden(self):
        v = model.run_validity(model.simulate(scenario("high_adoption"), 150))
        self.assertEqual((v.status, v.exit_code), (model.RUN_INFEASIBLE, 3))
        self.assertEqual(v.horizon_status, EXTRAPOLATED)
        self.assertEqual(v.first_extrapolated_year, 101)

    def test_infeasibility_only_in_extrapolated_years_is_extrapolation(self):
        rows = model.simulate(scenario("historical_documented_partial"), 150)
        infeasible = model.years_with_fee_funding(rows, model.FEE_FUNDING_INFEASIBLE)
        self.assertEqual(infeasible[0], 104)
        self.assertEqual(len(infeasible), 47)
        v = model.run_validity(rows)
        self.assertEqual((v.status, v.exit_code), (model.RUN_EXTRAPOLATED, 4))
        self.assertEqual(v.fee_funding_all_years, model.FEE_FUNDING_INFEASIBLE)
        self.assertEqual(v.fee_funding_covered_years, model.FEE_FUNDING_FEASIBLE)

    def test_extrapolation_beats_velocity(self):
        v = model.run_validity(model.simulate(velocity_only_scenario(), 20))
        self.assertEqual(v.fee_funding_all_years, model.FEE_FUNDING_VELOCITY_DEPENDENT)
        self.assertEqual((v.status, v.exit_code), (model.RUN_EXTRAPOLATED, 4))

    def test_velocity_in_covered_years_is_conditional(self):
        s = velocity_only_scenario()
        s["activity"]["transaction_fee_revenue_mbo"]["continuation"] = {"policy": "constant"}
        v = model.run_validity(model.simulate(s, 20))
        self.assertEqual((v.status, v.exit_code), (model.RUN_CONDITIONAL, 0))

    def test_covered_feasible_run_is_valid(self):
        v = model.run_validity(model.simulate(scenario("base"), 100))
        self.assertEqual((v.status, v.exit_code), (model.RUN_VALID, 0))


class CliTests(unittest.TestCase):
    def run_cli(self, name, years, output=None):
        args = ["--scenario", name, "--years", str(years)]
        if output:
            args += ["--output", str(output)]
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = model.main(args)
        return code, out.getvalue()

    def test_extrapolated_run_exits_4_and_keeps_its_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "base150.csv"
            code, text = self.run_cli("base", 150, path)
            lines = path.read_text(encoding="utf-8").splitlines()
        self.assertEqual(code, 4)
        self.assertEqual(len(lines), 152)
        header = lines[0].split(",")
        self.assertEqual(header[-2:], ["horizon_status", "extrapolated_inputs"])
        self.assertTrue(lines[101].split(",")[-2] == COVERED)
        self.assertTrue(lines[102].split(",")[-2] == EXTRAPOLATED)
        self.assertIn("EXTRAPOLATED from year 101", text)
        self.assertIn("not a forecast", text)
        self.assertIn("run validity                : extrapolated (exit 4)", text)

    def test_exit_codes_by_scenario_and_horizon(self):
        self.assertEqual(self.run_cli("base", 100)[0], 0)
        self.assertEqual(self.run_cli("high_adoption", 150)[0], 3)
        # Before horizon validity this exited 3, on an infeasibility created by
        # the extrapolation itself.
        self.assertEqual(self.run_cli("historical_documented_partial", 150)[0], 4)

    def test_covered_run_says_so(self):
        code, text = self.run_cli("base", 100)
        self.assertIn("horizon coverage            : every input defined through year 100", text)
        self.assertNotIn("EXTRAPOLATED", text)


class StudyHorizonTests(unittest.TestCase):
    def test_checkpoints_for_each_supported_horizon(self):
        self.assertEqual(ps.checkpoints_for(100), ps.CHECKPOINTS)
        self.assertEqual(ps.checkpoints_for(150), ps.CHECKPOINTS + (150,))
        self.assertEqual(ps.checkpoints_for(200), ps.CHECKPOINTS + (150, 200))
        self.assertEqual(ps.checkpoints_for(100, cs.CHECKPOINTS), cs.CHECKPOINTS)
        with self.assertRaises(model.ConfigError):
            ps.checkpoints_for(120)

    def test_unsupported_horizon_is_refused_on_the_command_line(self):
        for main in (ps.main, cs.main):
            with self.subTest(main=main.__module__):
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as ctx:
                    main(["--horizon", "120"])
                self.assertEqual(ctx.exception.code, 2)


def ps_runs(observations):
    return {(o.matrix, ps._schedule(o), o.stress) for o in observations}


class SensitivityExclusionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.at100 = ps.run_core_matrices(CONFIG) + ps.run_stress_layer(CONFIG)
        cls.at150 = ps.run_core_matrices(CONFIG, 150) + ps.run_stress_layer(CONFIG, 150)
        mixed = declared_config()
        cls.mixed = ps.run_core_matrices(mixed, 150) + ps.run_stress_layer(mixed, 150)
        full = declared_config(model.SHOCK_REVERT)
        cls.full = ps.run_core_matrices(full, 150) + ps.run_stress_layer(full, 150)

    def test_hundred_years_excludes_nothing(self):
        self.assertEqual(len(ps_runs(self.at100)), 184)
        self.assertEqual(ps.extrapolated_run_count(self.at100), 0)
        self.assertEqual(ps.comparable(self.at100), self.at100)

    def test_shipped_scenarios_give_no_comparable_run_at_150(self):
        self.assertEqual(len(ps_runs(self.at150)), 184)
        self.assertEqual(ps.extrapolated_run_count(self.at150), 184)
        self.assertEqual(ps.comparable(self.at150), [])
        text = ps.format_summary(self.at150, 150)
        self.assertIn("0 of 184 runs have every input defined through year 150", text)
        self.assertIn("every range is withheld", text)
        self.assertNotIn("cumulative issuance @150y", text)

    def test_extrapolated_runs_are_excluded_as_a_whole(self):
        # Mixed: base paths declared, stress shocks not. The compute contraction
        # ends inside the phases, so only two of the three stresses extrapolate.
        self.assertEqual(ps.extrapolated_run_count(self.mixed), 16)
        excluded = {(o.matrix, ps._schedule(o), o.stress) for o in self.mixed if ps.is_extrapolated_run(o)}
        self.assertEqual({k[2] for k in excluded}, {"stress_price_shock", "stress_combined"})
        kept = ps.comparable(self.mixed)
        self.assertEqual(len(ps_runs(kept)), 184 - 16)
        # Excluded at every checkpoint, including the covered year 10.
        self.assertFalse(any(ps.is_extrapolated_run(o) for o in kept))
        self.assertEqual(len(kept), (184 - 16) * len(ps.checkpoints_for(150)))

    def test_ranges_use_comparable_runs_only(self):
        text = ps.format_summary(self.mixed, 150)
        self.assertIn("168 of 184 runs have every input defined through year 150", text)
        self.assertIn("8 of 12 runs EXTRAPOLATED, excluded from every range", text)
        # Fee funding counts in the stress layer are over the 4 comparable runs.
        self.assertIn("fee funding infeasible, uncapped_primary_mechanics: ", text)
        stress = text.split("--- stress layer")[1]
        self.assertIn(" of 4 runs", stress)
        self.assertNotIn(" of 12 runs\n", stress.replace("EXTRAPOLATED", ""))
        final = [o for o in ps.comparable(self.mixed)
                 if o.year == 150 and o.stress == "none" and o.matrix == ps.UNCAPPED]
        lo = min(o.ending_supply for o in final)
        self.assertIn(f"ending supply @150y       : {lo:>18,.0f}", text)

    def test_complete_declarations_make_every_run_comparable(self):
        self.assertEqual(ps.extrapolated_run_count(self.full), 0)
        self.assertEqual({o.year for o in self.full}, set(ps.checkpoints_for(150)))
        self.assertEqual({o.run_validity for o in self.full} - {model.RUN_VALID, model.RUN_INFEASIBLE}, set())

    def test_study_exit_codes(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(ps.main([]), 0)
            self.assertEqual(ps.main(["--horizon", "150"]), 4)
            self.assertEqual(ps.main(["--horizon", "200", "--no-stress"]), 4)

    def test_hundred_year_observations_are_unchanged_by_declarations(self):
        declared = ps.run_core_matrices(declared_config(model.SHOCK_REVERT)) + ps.run_stress_layer(
            declared_config(model.SHOCK_REVERT)
        )
        self.assertEqual(len(declared), len(self.at100))
        self.assertEqual(declared, self.at100)


class CapSemanticsExclusionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.at100 = cs.run_all(CONFIG)
        cls.at150 = cs.run_all(CONFIG, 150)
        cls.full = cs.run_all(declared_config(), 150)

    def test_hundred_years_compares_every_schedule(self):
        divergences = cs.all_divergences(self.at100)
        self.assertEqual(len(divergences), 6)
        for d in divergences:
            self.assertEqual(d["excluded_extrapolated"], 0)
            self.assertEqual(d["identical"] + d["different"], 80)

    def test_shipped_scenarios_give_no_comparison_at_150(self):
        terminal = [o for o in self.at150 if o.year == 150]
        self.assertEqual(len(terminal), 480)
        self.assertTrue(all(cs.is_extrapolated_run(o) for o in terminal))
        for d in cs.all_divergences(self.at150, 150):
            self.assertEqual(d["excluded_extrapolated"], 80)
            self.assertEqual(d["identical"] + d["different"], 0)
        text = cs.format_summary(self.at150, CONFIG, 150)
        self.assertIn("0 of 480 runs defined through year 150", text)
        self.assertIn("withheld: inputs extrapolated from year 101", text)
        self.assertNotIn("identical over 150 years", text)

    def test_complete_declarations_make_every_schedule_comparable(self):
        for d in cs.all_divergences(self.full, 150):
            self.assertEqual(d["excluded_extrapolated"], 0)
            self.assertEqual(d["identical"] + d["different"], 80)

    def test_study_exit_codes(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(cs.main([]), 0)
            self.assertEqual(cs.main(["--horizon", "150"]), 4)


if __name__ == "__main__":
    unittest.main(verbosity=2)
