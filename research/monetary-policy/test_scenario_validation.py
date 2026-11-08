#!/usr/bin/env python3
"""Regression tests for strict scenario validation (#243).

The scenario loader reads keys with defaults. Before these checks a misspelled
key was ignored and the run still reported valid: a misspelled shock metric
disabled the shock, a misspelled `annual_growth` became growth 0, and, after
R2-1, a shock whose `end_year` key was absent became persistent. Each of those
cases is pinned below, together with the declarations that must stay accepted.

A malformed configuration is a configuration error (exit 2). It is never a
result, so it is never valid, conditional, infeasible or extrapolated.

Run from the repository root:

    python3 research/monetary-policy/test_scenario_validation.py
"""

from __future__ import annotations

import contextlib
import copy
import io
import json
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


def scenario(name):
    return copy.deepcopy(model.get_scenario(CONFIG, name))


def rejected(test, s, fragment, years=100):
    """Assert that both entry points reject `s`, for the stated reason."""
    for call in (lambda: model.simulate(s, years), lambda: model.input_coverage(s)):
        with test.assertRaises(model.ConfigError) as ctx:
            call()
        test.assertIn(fragment, str(ctx.exception))


class ShippedScenariosTests(unittest.TestCase):
    def test_all_eleven_scenarios_and_six_shocks_are_accepted(self):
        self.assertEqual(len(SCENARIOS), 11)
        shocks = [shock for name in SCENARIOS for shock in scenario(name)["shocks"]]
        self.assertEqual(len(shocks), 6)
        for name in SCENARIOS:
            with self.subTest(scenario=name):
                s = scenario(name)
                model.validate_scenario_keys(s)
                model.validate_shocks(s["shocks"])
                model.simulate(s, 100)

    def test_every_shipped_shock_states_its_end_year(self):
        for name in SCENARIOS:
            for shock in scenario(name)["shocks"]:
                with self.subTest(scenario=name, metric=shock["metric"]):
                    self.assertIn("end_year", shock)
                    self.assertIsInstance(shock["end_year"], int)


class DemonstratedFailureTests(unittest.TestCase):
    """The four cases recorded in #243, each now a configuration error."""

    def test_misspelled_shock_metric_is_rejected(self):
        s = scenario("stress_price_shock")
        s["shocks"][0]["metric"] = "mbo_price"
        rejected(self, s, "shocks[0].metric must be one of")

    def test_shock_without_end_year_is_rejected_not_persistent(self):
        s = scenario("stress_compute_contraction")
        shock = s["shocks"][0]
        shock["end_yr"] = shock.pop("end_year")
        rejected(self, s, "shocks[0] has unknown keys ['end_yr']")
        del shock["end_yr"]
        rejected(self, s, "shocks[0].end_year is required")

    def test_misspelled_after_end_is_rejected(self):
        s = scenario("stress_compute_contraction")
        s["shocks"][0]["after_end_"] = model.SHOCK_UNSPECIFIED
        rejected(self, s, "shocks[0] has unknown keys ['after_end_']")

    def test_misspelled_annual_growth_is_rejected(self):
        s = scenario("stress_compute_contraction")
        phase = s["activity"]["compute_user_spend_mbo"]["phases"][0]
        phase["anual_growth"] = phase.pop("annual_growth")
        rejected(self, s, "activity.compute_user_spend_mbo.phases[0] has unknown keys ['anual_growth']")


class UnknownKeyTests(unittest.TestCase):
    def test_unknown_keys_are_rejected_at_every_level(self):
        cases = [
            ("scenario", lambda s: s.update(prices=s.pop("price")), "scenario has unknown keys ['prices']"),
            ("activity", lambda s: s["activity"].update(
                compute_user_spend=s["activity"].pop("compute_user_spend_mbo")),
             "activity has unknown keys ['compute_user_spend']"),
            ("path", lambda s: s["price"].update(continuaton={"policy": "constant"}),
             "price has unknown keys ['continuaton']"),
            ("phase", lambda s: s["price"]["phases"][0].update(until=10),
             "price.phases[0] has unknown keys ['until']"),
            ("shock", lambda s: s["shocks"][0].update(duration=5),
             "shocks[0] has unknown keys ['duration']"),
        ]
        for level, mutate, fragment in cases:
            with self.subTest(level=level):
                s = scenario("stress_price_shock")
                mutate(s)
                rejected(self, s, fragment)

    def test_malformed_shock_values_are_rejected(self):
        cases = [
            ({"start_year": True}, "start_year must be an integer >= 0"),
            ({"start_year": -1}, "start_year must be an integer >= 0"),
            ({"end_year": 10}, "end_year must be an integer >= start_year, or null"),
            ({"end_year": "100"}, "end_year must be an integer >= start_year, or null"),
            ({"multiplier": True}, "multiplier must be a number >= 0"),
            ({"multiplier": -0.5}, "multiplier must be a number >= 0"),
            ({"end_year": None, "after_end": model.SHOCK_REVERT}, "after_end needs an integer end_year"),
            ({"after_end": "recover"}, "after_end must be one of"),
        ]
        for fields, fragment in cases:
            with self.subTest(fields=fields):
                s = scenario("stress_price_shock")
                s["shocks"][0].update(fields)
                rejected(self, s, fragment)

    def test_a_shock_on_an_unused_metric_is_still_validated(self):
        # Before, a shock was validated only by the path its metric matched, so
        # a malformed shock on another metric could pass unread.
        s = scenario("base")
        s["shocks"] = [{"metric": "staking_ratio", "start_year": 5, "end_year": 2, "multiplier": 0.5}]
        rejected(self, s, "end_year must be an integer >= start_year")


class AcceptedDeclarationTests(unittest.TestCase):
    def test_persistent_shock_needs_an_explicit_null(self):
        s = scenario("stress_price_shock")
        s["shocks"][0]["end_year"] = None
        rows = model.simulate(s, 100)
        self.assertEqual(model.input_coverage(s)["mbo_reference_price"], 100)
        self.assertEqual(model.run_validity(rows).status, model.RUN_VALID)

    def test_declared_after_end_and_continuation_are_accepted(self):
        for after in model.SHOCK_AFTER_END:
            with self.subTest(after_end=after):
                s = scenario("stress_compute_contraction")
                s["shocks"][0]["after_end"] = after
                s["price"]["continuation"] = {"policy": model.CONTINUATION_CONSTANT}
                model.simulate(s, 100)

    def test_an_absent_path_is_still_extrapolation_not_an_error(self):
        # Absence is a statement the scenario makes (nothing beyond year 0);
        # a misspelling is not. Only the second is a configuration error.
        s = scenario("base")
        del s["price"]
        rows = model.simulate(s, 5)
        self.assertEqual(model.run_validity(rows).status, model.RUN_EXTRAPOLATED)

    def test_validation_changes_no_hundred_year_output(self):
        for name in SCENARIOS:
            with self.subTest(scenario=name):
                rows = model.simulate(scenario(name), 100)
                self.assertEqual(len(rows), 101)
                self.assertEqual(model.identity_violations(rows), [])


class ExitCodeTests(unittest.TestCase):
    """A malformed configuration exits 2 everywhere, prints no result and
    writes no CSV. It is never classified."""

    def write_config(self, tmp, mutate):
        cfg = copy.deepcopy(CONFIG)
        mutate(cfg["scenarios"])
        path = Path(tmp) / "scenarios.json"
        path.write_text(json.dumps(cfg), encoding="utf-8")
        return path

    def invoke(self, main, args):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = main(args)
        return code, out.getvalue(), err.getvalue()

    def test_model_cli_exits_2_without_a_result(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = self.write_config(
                tmp, lambda s: s["stress_price_shock"]["shocks"][0].update(metric="mbo_price")
            )
            csv_path = Path(tmp) / "out.csv"
            code, out, err = self.invoke(model.main, [
                "--config", str(config), "--scenario", "stress_price_shock",
                "--years", "100", "--output", str(csv_path),
            ])
            self.assertFalse(csv_path.exists())
        self.assertEqual(code, 2)
        self.assertIn("shocks[0].metric must be one of", err)
        self.assertNotIn("run validity", out)

    def test_studies_exit_2_on_a_malformed_config(self):
        cases = [
            (ps.main, lambda s: s["stress_combined"]["shocks"][0].pop("end_year"), "end_year is required"),
            (cs.main, lambda s: s["base"]["price"]["phases"][0].update(anual_growth=0.1), "unknown keys"),
        ]
        for main, mutate, fragment in cases:
            with self.subTest(study=main.__module__):
                with tempfile.TemporaryDirectory() as tmp:
                    code, out, err = self.invoke(main, ["--config", str(self.write_config(tmp, mutate))])
                self.assertEqual(code, 2)
                self.assertIn(fragment, err)
                self.assertEqual(out, "")

    def test_studies_accept_a_valid_config_given_on_the_command_line(self):
        # cap_semantics used to crash on any --config (a str, not a Path).
        for main in (ps.main, cs.main):
            with self.subTest(study=main.__module__):
                with tempfile.TemporaryDirectory() as tmp:
                    code, out, err = self.invoke(
                        main, ["--config", str(self.write_config(tmp, lambda s: None)), "--summary"]
                    )
                self.assertEqual(code, 0, err)
                self.assertEqual(err, "")
                self.assertIn("horizon coverage", out)


if __name__ == "__main__":
    unittest.main(verbosity=2)
