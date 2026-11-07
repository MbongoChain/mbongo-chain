#!/usr/bin/env python3
"""Regression tests for fee funding feasibility.

An accounting identity says the books balance. Fee funding says whether the
fees a scenario assumes could have been paid with the MBO that existed. The
two are different checks, and a run can pass the first and fail the second:
`high_adoption` did, silently, before these flags existed.

These tests pin the classification and the scenarios it currently flags. They
assert no economic outcome as desirable.

Run from the repository root:

    python3 research/monetary-policy/test_fee_funding.py
"""

from __future__ import annotations

import contextlib
import io
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import model  # noqa: E402
import primary_sensitivity as ps  # noqa: E402
import cap_semantics as cs  # noqa: E402


CONFIG = model.load_config(model.DEFAULT_CONFIG)
SCENARIOS = sorted(CONFIG["scenarios"])
RUNS = {name: model.simulate(CONFIG["scenarios"][name], 100) for name in SCENARIOS}

INFEASIBLE = model.FEE_FUNDING_INFEASIBLE
VELOCITY = model.FEE_FUNDING_VELOCITY_DEPENDENT
FEASIBLE = model.FEE_FUNDING_FEASIBLE


class ClassificationTests(unittest.TestCase):
    def test_no_mbo_at_all_cannot_pay_any_fee(self):
        self.assertEqual(model.classify_fee_funding(0.0, 10.0, 0.0), INFEASIBLE)

    def test_burn_above_existing_mbo_is_infeasible(self):
        self.assertEqual(model.classify_fee_funding(100.0, 150.0, 120.0), INFEASIBLE)

    def test_fee_pool_above_existing_mbo_needs_velocity(self):
        self.assertEqual(model.classify_fee_funding(100.0, 150.0, 50.0), VELOCITY)

    def test_fees_within_existing_mbo_are_feasible(self):
        self.assertEqual(model.classify_fee_funding(100.0, 80.0, 40.0), FEASIBLE)

    def test_no_fees_are_feasible_even_without_mbo(self):
        self.assertEqual(model.classify_fee_funding(0.0, 0.0, 0.0), FEASIBLE)


class ScenarioTests(unittest.TestCase):
    def test_eleven_scenarios_are_checked(self):
        self.assertEqual(len(RUNS), 11)

    def test_high_adoption_reaches_zero_supply_and_is_flagged(self):
        rows = RUNS["high_adoption"]
        zero = [r.year for r in rows if r.year > 0 and r.ending_supply == 0.0]
        infeasible = model.years_with_fee_funding(rows, INFEASIBLE)
        self.assertEqual(zero, list(range(52, 101)))
        self.assertEqual(infeasible, list(range(52, 101)))
        # The defect being flagged: validator fee revenue is still reported.
        self.assertTrue(all(rows[y].validator_fee_revenue > 0 for y in infeasible))
        self.assertFalse(model.is_fee_funding_feasible(rows))

    def test_high_adoption_needs_velocity_before_it_fails(self):
        rows = RUNS["high_adoption"]
        self.assertEqual(model.years_with_fee_funding(rows, VELOCITY), [48, 49, 50, 51])

    def test_only_high_adoption_is_infeasible(self):
        flagged = [n for n, rows in RUNS.items() if not model.is_fee_funding_feasible(rows)]
        self.assertEqual(flagged, ["high_adoption"])

    def test_identities_still_hold_where_funding_fails(self):
        # The two checks are independent: an infeasible run balances its books.
        for name, rows in RUNS.items():
            with self.subTest(scenario=name):
                self.assertEqual(model.identity_violations(rows), [])

    def test_a_burn_cap_is_never_silent(self):
        for name, rows in RUNS.items():
            for r in rows:
                with self.subTest(scenario=name, year=r.year):
                    if r.burn_shortfall > 1e-6:
                        self.assertEqual(r.fee_funding, INFEASIBLE)
                    self.assertAlmostEqual(r.burned, r.burn_requested - r.burn_shortfall, places=6)

    def test_fees_are_reported_in_full_not_clamped(self):
        for name, rows in RUNS.items():
            for r in rows:
                with self.subTest(scenario=name, year=r.year):
                    self.assertAlmostEqual(
                        r.fee_pool, r.transaction_fee_revenue + r.protocol_compute_fee, places=6
                    )
                    self.assertAlmostEqual(
                        r.validator_fee_revenue + r.treasury_revenue + r.burn_requested,
                        r.fee_pool,
                        places=4,
                    )
                    self.assertAlmostEqual(
                        r.available_supply, r.starting_supply + r.gross_issuance, places=6
                    )


class CliTests(unittest.TestCase):
    def run_cli(self, name):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = model.main(["--scenario", name, "--years", "100"])
        return code, out.getvalue()

    def test_infeasible_run_is_rejected_with_its_own_exit_code(self):
        code, text = self.run_cli("high_adoption")
        self.assertEqual(code, 3)
        self.assertIn("NOT VALID", text)
        self.assertIn("accounting identities       : OK", text)

    def test_feasible_run_exits_zero(self):
        code, text = self.run_cli("base")
        self.assertEqual(code, 0)
        self.assertNotIn("NOT VALID", text)


class StudyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ps_obs = ps.run_core_matrices(CONFIG) + ps.run_stress_layer(CONFIG)
        cls.cs_obs = cs.run_all(CONFIG)

    def test_sensitivity_flags_ten_of_184_runs(self):
        runs = {(o.matrix, ps._schedule(o), o.stress) for o in self.ps_obs}
        bad = {(o.matrix, ps._schedule(o), o.stress) for o in self.ps_obs
               if o.fee_funding_first_infeasible_year}
        self.assertEqual(len(runs), 184)
        self.assertEqual(len(bad), 10)
        self.assertEqual({b[1] for b in bad}, {(788400, 2, 0.25), (788400, 2, 0.5)})

    def test_sensitivity_historical_reference_is_feasible(self):
        hist = [o for o in self.ps_obs if o.is_historical_reference]
        self.assertGreater(len(hist), 0)
        self.assertTrue(all(not o.fee_funding_first_infeasible_year for o in hist))

    def test_cap_semantics_flags_six_of_480_runs_all_with_burn(self):
        key = lambda o: (o.burn_environment, o.cap_semantics,
                         (o.initial_issuance, o.step_interval, o.decay_factor))
        runs = {key(o) for o in self.cs_obs}
        bad = {key(o) for o in self.cs_obs if o.fee_funding_first_infeasible_year}
        self.assertEqual(len(runs), 480)
        self.assertEqual(len(bad), 6)
        self.assertEqual({b[0] for b in bad}, {cs.WITH_BURN})

    def test_cap_semantics_divergence_involves_no_infeasible_run(self):
        divergences = cs.all_divergences(self.cs_obs)
        self.assertEqual(len(divergences), 6)
        for d in divergences:
            with self.subTest(env=d["burn_environment"], left=d["left"], right=d["right"]):
                self.assertEqual(d["different_with_infeasible"], 0)

    def test_infeasible_count_to_date_never_decreases(self):
        for obs, key in (
            (self.ps_obs, lambda o: (o.matrix, ps._schedule(o), o.stress)),
            (self.cs_obs, lambda o: (o.burn_environment, o.cap_semantics,
                                     (o.initial_issuance, o.step_interval, o.decay_factor))),
        ):
            last = {}
            for o in sorted(obs, key=lambda o: (str(key(o)), o.year)):
                k = key(o)
                self.assertGreaterEqual(o.fee_funding_infeasible_years_to_date, last.get(k, 0))
                last[k] = o.fee_funding_infeasible_years_to_date


if __name__ == "__main__":
    unittest.main(verbosity=2)
