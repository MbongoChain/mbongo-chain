#!/usr/bin/env python3
"""Tests for the primary-issuance sensitivity study.

These guard the experimental design, not an expected economic outcome: no
test asserts that any schedule produces a particular supply, budget or
"winning" result. They assert that the two matrices are comparable, that the
controlled variables really are controlled, and that the study is
deterministic.

Run from the repository root:

    python3 research/monetary-policy/test_primary_sensitivity.py
"""

from __future__ import annotations

import copy
import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import model  # noqa: E402
import primary_sensitivity as ps  # noqa: E402


CONFIG = model.load_config(model.DEFAULT_CONFIG)
GRID = ps.schedule_grid()
CORE = ps.run_core_matrices(CONFIG)


def core_for(matrix):
    return [o for o in CORE if o.matrix == matrix]


def schedules_in(matrix):
    return [
        (o.initial_annual_issuance, o.step_years, o.step_factor)
        for o in core_for(matrix)
        if o.year == ps.HORIZON_YEARS
    ]


class GridTests(unittest.TestCase):
    def test_grid_has_eighty_unique_schedules(self):
        self.assertEqual(len(GRID), 80)
        self.assertEqual(len(set(GRID)), 80)

    def test_both_matrices_contain_exactly_eighty_schedules(self):
        for matrix in ps.MATRICES:
            with self.subTest(matrix=matrix):
                self.assertEqual(len(schedules_in(matrix)), 80)

    def test_schedule_tuples_are_identical_between_matrices(self):
        a, b = (schedules_in(m) for m in ps.MATRICES)
        self.assertEqual(a, b, "matrices must cover the same schedules in the same order")

    def test_historical_reference_appears_exactly_once_per_matrix(self):
        self.assertIn(ps.HISTORICAL_REFERENCE, GRID)
        for matrix in ps.MATRICES:
            with self.subTest(matrix=matrix):
                flagged = [
                    o
                    for o in core_for(matrix)
                    if o.is_historical_reference and o.year == ps.HORIZON_YEARS
                ]
                self.assertEqual(len(flagged), 1)
                self.assertEqual(
                    (
                        flagged[0].initial_annual_issuance,
                        flagged[0].step_years,
                        flagged[0].step_factor,
                    ),
                    ps.HISTORICAL_REFERENCE,
                )

    def test_grid_ordering_is_deterministic(self):
        self.assertEqual(ps.schedule_grid(), GRID)
        self.assertEqual(
            [(o.matrix, o.initial_annual_issuance, o.step_years, o.step_factor, o.year) for o in CORE],
            [
                (o.matrix, o.initial_annual_issuance, o.step_years, o.step_factor, o.year)
                for o in ps.run_core_matrices(CONFIG)
            ],
        )


class MatrixSemanticsTests(unittest.TestCase):
    def test_uncapped_matrix_never_reports_a_clamp(self):
        for o in core_for(ps.UNCAPPED):
            self.assertFalse(o.clamped, f"{o.initial_annual_issuance}/{o.step_years}/{o.step_factor}")
            self.assertEqual(o.cap, "")

    def test_capped_matrix_uses_the_historical_cap_for_every_run(self):
        for o in core_for(ps.FIXED_CAP):
            self.assertEqual(o.cap, str(ps.HISTORICAL_CAP))

    def test_a_high_schedule_clamps_under_the_fixed_cap(self):
        # Analytically far above the cap; must clamp.
        high = (12_614_400, 20, 0.90)
        self.assertIn(high, GRID)
        rows = [
            o
            for o in core_for(ps.FIXED_CAP)
            if (o.initial_annual_issuance, o.step_years, o.step_factor) == high
        ]
        self.assertTrue(rows and all(o.clamped for o in rows))

    def test_a_low_schedule_stays_below_the_cap(self):
        # I x S / (1 - F) = 788,400 x 2 / 0.75 well below 31,536,000.
        low = (788_400, 2, 0.25)
        self.assertIn(low, GRID)
        rows = [
            o
            for o in core_for(ps.FIXED_CAP)
            if (o.initial_annual_issuance, o.step_years, o.step_factor) == low
        ]
        self.assertTrue(rows and not any(o.clamped for o in rows))

    def test_capped_supply_never_exceeds_the_cap(self):
        for o in core_for(ps.FIXED_CAP):
            self.assertLessEqual(o.ending_supply, ps.HISTORICAL_CAP + 1e-6)

    def test_uncapped_matrix_keeps_full_resolution(self):
        # The whole reason the study runs two matrices: without a cap every
        # schedule must remain distinguishable.
        final = [o for o in core_for(ps.UNCAPPED) if o.year == ps.HORIZON_YEARS]
        totals = {round(o.cumulative_issuance, 2) for o in final}
        self.assertEqual(len(totals), 80)


class ControlledVariableTests(unittest.TestCase):
    def test_only_schedule_and_cap_differ_from_the_controlled_scenario(self):
        base = ps.controlled_scenario(CONFIG)
        built = ps.build_scenario(base, (1_000_000, 7, 0.4), cap=None)
        stripped_base = copy.deepcopy(base)
        stripped_built = copy.deepcopy(built)
        for s in (stripped_base, stripped_built):
            s["monetary"].pop("primary_emission")
            s["monetary"].pop("cap")
        self.assertEqual(stripped_base, stripped_built)

    def test_paired_runs_share_every_non_primary_parameter(self):
        base = ps.controlled_scenario(CONFIG)
        schedule = ps.HISTORICAL_REFERENCE
        a = ps.build_scenario(base, schedule, cap=None)
        b = ps.build_scenario(base, schedule, cap=ps.HISTORICAL_CAP)
        for key in ("supply_distribution", "issuance_allocation", "fees", "activity", "price", "shocks"):
            self.assertEqual(a[key], b[key], key)
        self.assertEqual(a["monetary"]["primary_emission"], b["monetary"]["primary_emission"])

    def test_overriding_the_schedule_actually_changes_issuance(self):
        base = ps.controlled_scenario(CONFIG)
        small = model.simulate(ps.build_scenario(base, (788_400, 2, 0.25), None), 20)
        large = model.simulate(ps.build_scenario(base, (12_614_400, 20, 0.90), None), 20)
        self.assertGreater(
            sum(r.gross_issuance for r in large), sum(r.gross_issuance for r in small) * 5
        )


class OutputTests(unittest.TestCase):
    def test_every_run_covers_the_full_horizon(self):
        years = sorted({o.year for o in CORE})
        self.assertEqual(years, sorted(ps.CHECKPOINTS))
        self.assertEqual(max(years), 100)

    def test_no_nan_infinite_or_negative_supply(self):
        for o in CORE:
            for name in ("cumulative_issuance", "ending_supply", "security_budget_mbo"):
                value = getattr(o, name)
                self.assertFalse(math.isnan(value) or math.isinf(value), f"{name} not finite")
            self.assertGreaterEqual(o.ending_supply, 0.0)
            self.assertGreaterEqual(o.cumulative_issuance, 0.0)

    def test_accounting_identities_hold_for_every_run(self):
        base = ps.controlled_scenario(CONFIG)
        for matrix, cap in ps.MATRICES.items():
            for schedule in (ps.HISTORICAL_REFERENCE, (12_614_400, 20, 0.90), (788_400, 2, 0.25)):
                with self.subTest(matrix=matrix, schedule=schedule):
                    rows = model.simulate(ps.build_scenario(base, schedule, cap), 100)
                    self.assertEqual(model.identity_violations(rows), [])

    def test_results_are_reproducible(self):
        self.assertEqual(CORE, ps.run_core_matrices(CONFIG))

    def test_csv_columns_match_the_observation_definition(self):
        self.assertEqual(set(ps.CSV_COLUMNS), set(vars(CORE[0]).keys()))

    def test_summary_reports_matrices_separately(self):
        text = ps.format_summary(CORE)
        self.assertIn(ps.UNCAPPED, text)
        self.assertIn(ps.FIXED_CAP, text)
        self.assertIn("NOT OPTIMIZATION", text)


class StressLayerTests(unittest.TestCase):
    def test_representative_shapes_are_all_in_the_grid(self):
        for name, schedule in ps.REPRESENTATIVE_SHAPES.items():
            with self.subTest(shape=name):
                self.assertIn(schedule, GRID)

    def test_stress_sources_exist_and_carry_shocks(self):
        for name in ps.STRESS_SOURCES:
            with self.subTest(stress=name):
                scenario = model.get_scenario(CONFIG, name)
                self.assertTrue(scenario.get("shocks"), f"{name} defines no shocks")


if __name__ == "__main__":
    unittest.main(verbosity=2)
