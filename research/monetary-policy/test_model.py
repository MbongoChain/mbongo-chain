#!/usr/bin/env python3
"""Tests for the MBO monetary policy model.

These test the arithmetic, not the presence of files: accounting identities,
policy-family behaviour, shock windowing, and input validation.

Run from the repository root:
    python3 -m unittest discover -s research/monetary-policy -t .
or:
    python3 research/monetary-policy/test_model.py
"""

from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import model  # noqa: E402


CONFIG = model.load_config(model.DEFAULT_CONFIG)


def scenario(name: str) -> dict:
    return copy.deepcopy(model.get_scenario(CONFIG, name))


class HorizonTests(unittest.TestCase):
    def test_hundred_year_run_has_one_row_per_year_including_year_zero(self):
        rows = model.simulate(scenario("base"), 100)
        self.assertEqual(len(rows), 101)
        self.assertEqual(rows[0].year, 0)
        self.assertEqual(rows[-1].year, 100)
        self.assertEqual([r.year for r in rows], list(range(101)))

    def test_year_zero_issues_nothing(self):
        rows = model.simulate(scenario("base"), 10)
        self.assertEqual(rows[0].gross_issuance, 0.0)
        self.assertEqual(rows[0].burned, 0.0)
        self.assertEqual(rows[0].ending_supply, rows[0].starting_supply)

    def test_every_required_summary_year_is_present(self):
        rows = model.simulate(scenario("base"), 100)
        years = {r.year for r in rows}
        for checkpoint in (1, 5, 10, 20, 30, 50, 75, 100):
            self.assertIn(checkpoint, years)


class DeterminismTests(unittest.TestCase):
    def test_same_configuration_produces_identical_results(self):
        first = model.simulate(scenario("base"), 100)
        second = model.simulate(scenario("base"), 100)
        self.assertEqual(first, second)

    def test_all_shipped_scenarios_are_deterministic_and_finite(self):
        for name in CONFIG["scenarios"]:
            with self.subTest(scenario=name):
                first = model.simulate(scenario(name), 100)
                second = model.simulate(scenario(name), 100)
                self.assertEqual(first, second)
                self.assertEqual(model.identity_violations(first), [])


class AccountingIdentityTests(unittest.TestCase):
    def test_supply_identity_holds_for_every_scenario(self):
        for name in CONFIG["scenarios"]:
            with self.subTest(scenario=name):
                rows = model.simulate(scenario(name), 100)
                for row in rows:
                    self.assertAlmostEqual(
                        row.ending_supply,
                        row.starting_supply + row.gross_issuance - row.burned,
                        places=6,
                    )

    def test_burn_is_subtracted_exactly_once(self):
        cfg = scenario("base")
        cfg["fees"] = {"validator_share": 0.0, "treasury_share": 0.0, "burn_share": 1.0}
        rows = model.simulate(cfg, 30)
        for row in rows[1:]:
            fee_pool = row.transaction_fee_revenue + row.protocol_compute_fee
            # The whole pool is burned in this configuration, and the supply
            # delta must reflect exactly that burn, not twice.
            self.assertAlmostEqual(row.burned, fee_pool, places=6)
            self.assertAlmostEqual(
                row.ending_supply - row.starting_supply,
                row.gross_issuance - fee_pool,
                places=6,
            )

    def test_compute_spend_splits_without_leakage(self):
        rows = model.simulate(scenario("base"), 50)
        for row in rows:
            self.assertAlmostEqual(
                row.worker_revenue + row.protocol_compute_fee,
                row.compute_user_spend,
                places=6,
            )

    def test_fee_pool_splits_three_ways_without_double_counting(self):
        cfg = scenario("base")
        rows = model.simulate(cfg, 40)
        shares = cfg["fees"]
        for row in rows[1:]:
            fee_pool = row.transaction_fee_revenue + row.protocol_compute_fee
            self.assertAlmostEqual(
                row.validator_fee_revenue + row.treasury_revenue + row.burned,
                fee_pool,
                places=6,
            )
            self.assertAlmostEqual(
                row.validator_fee_revenue, fee_pool * shares["validator_share"], places=6
            )

    def test_security_budget_is_the_sum_of_its_components(self):
        rows = model.simulate(scenario("base"), 60)
        for row in rows:
            self.assertAlmostEqual(
                row.security_budget_mbo,
                row.validator_issuance_revenue + row.validator_fee_revenue,
                places=6,
            )

    def test_supply_layers_are_ordered(self):
        for name in CONFIG["scenarios"]:
            with self.subTest(scenario=name):
                for row in model.simulate(scenario(name), 100):
                    self.assertLessEqual(row.staked_supply, row.circulating_supply + 1e-6)
                    self.assertLessEqual(row.circulating_supply, row.ending_supply + 1e-6)
                    self.assertGreaterEqual(row.ending_supply, 0.0)


class PolicyFamilyTests(unittest.TestCase):
    def test_hard_cap_is_never_exceeded(self):
        cfg = scenario("historical_documented")
        cap = cfg["monetary"]["cap"]
        rows = model.simulate(cfg, 100)
        for row in rows:
            self.assertLessEqual(row.ending_supply, cap + 1e-6)
        # The cap bounds cumulative issuance. Ending supply can finish far
        # lower because this scenario burns every fee, so asserting on the
        # final supply would test the burn assumptions, not the cap.
        cumulative_issuance = sum(r.gross_issuance for r in rows)
        self.assertLessEqual(cumulative_issuance, cap + 1e-6)
        # And the schedule really does approach the cap, or the test is vacuous.
        self.assertGreater(cumulative_issuance, cap * 0.99)

    def test_cap_clamps_issuance_that_would_overshoot(self):
        # A schedule that would blow through the cap must be clamped exactly
        # at the remaining headroom, then stop.
        cfg = scenario("base")
        cfg["monetary"]["cap"] = 1000000
        cfg["monetary"]["primary_emission"]["initial_annual_issuance"] = 600000
        cfg["monetary"]["primary_emission"]["step_factor"] = 1.0
        cfg["fees"] = {"validator_share": 1.0, "treasury_share": 0.0, "burn_share": 0.0}
        rows = model.simulate(cfg, 10)
        for row in rows:
            self.assertLessEqual(row.ending_supply, 1000000 + 1e-6)
        self.assertAlmostEqual(rows[1].gross_issuance, 600000.0, places=6)
        self.assertAlmostEqual(rows[2].gross_issuance, 400000.0, places=6)
        for row in rows[3:]:
            self.assertAlmostEqual(row.gross_issuance, 0.0, places=6)

    def test_fixed_tail_issues_its_configured_amount_after_transition(self):
        cfg = scenario("fixed_tail_illustrative")
        start = cfg["monetary"]["tail"]["start_year"]
        amount = cfg["monetary"]["tail"]["annual_amount"]
        rows = model.simulate(cfg, 60)
        for row in rows:
            if row.year >= start:
                self.assertAlmostEqual(row.gross_issuance, amount, places=6)
            elif row.year >= 1:
                self.assertNotAlmostEqual(row.gross_issuance, amount, places=6)

    def test_percentage_tail_issues_its_configured_rate_after_transition(self):
        cfg = scenario("percentage_tail_illustrative")
        start = cfg["monetary"]["tail"]["start_year"]
        rate = cfg["monetary"]["tail"]["annual_rate"]
        rows = model.simulate(cfg, 60)
        for row in rows:
            if row.year >= start:
                self.assertAlmostEqual(row.gross_issuance, row.starting_supply * rate, places=6)

    def test_adaptive_issuance_stays_inside_its_band(self):
        cfg = scenario("adaptive_bounded_illustrative")
        start = cfg["monetary"]["tail"]["start_year"]
        band = cfg["monetary"]["adaptive"]
        rows = model.simulate(cfg, 100)
        for row in rows:
            if row.year >= start and row.starting_supply > 0:
                rate = row.gross_issuance / row.starting_supply
                self.assertGreaterEqual(rate, band["min_rate"] - 1e-9)
                self.assertLessEqual(rate, band["max_rate"] + 1e-9)

    def test_primary_emission_steps_down_by_its_factor(self):
        cfg = scenario("base")
        cfg["monetary"]["cap"] = None
        rows = model.simulate(cfg, 20)
        step_years = cfg["monetary"]["primary_emission"]["step_years"]
        factor = cfg["monetary"]["primary_emission"]["step_factor"]
        first_block = rows[1].gross_issuance
        second_block = rows[1 + step_years].gross_issuance
        self.assertAlmostEqual(second_block, first_block * factor, places=6)

    def test_families_produce_distinct_trajectories(self):
        finals = {}
        for name in (
            "base",
            "fixed_tail_illustrative",
            "percentage_tail_illustrative",
            "adaptive_bounded_illustrative",
        ):
            finals[name] = model.simulate(scenario(name), 100)[-1].ending_supply
        self.assertEqual(len(set(round(v, 3) for v in finals.values())), len(finals))


class ShockTests(unittest.TestCase):
    def test_shock_applies_only_inside_its_window(self):
        cfg = scenario("base")
        cfg["shocks"] = [
            {"metric": "compute_user_spend_mbo", "start_year": 10, "end_year": 12, "multiplier": 0.5}
        ]
        shocked = model.simulate(cfg, 20)
        clean = model.simulate(scenario("base"), 20)
        for year in range(0, 10):
            self.assertAlmostEqual(
                shocked[year].compute_user_spend, clean[year].compute_user_spend, places=6
            )
        for year in range(10, 13):
            self.assertAlmostEqual(
                shocked[year].compute_user_spend, clean[year].compute_user_spend * 0.5, places=6
            )
        for year in range(13, 21):
            self.assertAlmostEqual(
                shocked[year].compute_user_spend, clean[year].compute_user_spend, places=6
            )

    def test_price_shock_changes_reference_values_but_not_mbo_quantities(self):
        cfg = scenario("base")
        cfg["shocks"] = [
            {"metric": "mbo_reference_price", "start_year": 5, "end_year": 100, "multiplier": 0.2}
        ]
        shocked = model.simulate(cfg, 30)
        clean = model.simulate(scenario("base"), 30)
        for year in range(5, 31):
            self.assertAlmostEqual(
                shocked[year].security_budget_mbo, clean[year].security_budget_mbo, places=6
            )
            self.assertAlmostEqual(
                shocked[year].security_budget_reference_value,
                clean[year].security_budget_reference_value * 0.2,
                places=6,
            )

    def test_staking_shock_reduces_staked_supply_only_in_window(self):
        cfg = scenario("base")
        cfg["shocks"] = [
            {"metric": "staking_ratio", "start_year": 10, "end_year": 15, "multiplier": 0.5}
        ]
        shocked = model.simulate(cfg, 20)
        clean = model.simulate(scenario("base"), 20)
        self.assertAlmostEqual(shocked[9].staked_supply, clean[9].staked_supply, places=6)
        self.assertAlmostEqual(shocked[12].staked_supply, clean[12].staked_supply * 0.5, places=6)
        self.assertAlmostEqual(shocked[16].staked_supply, clean[16].staked_supply, places=6)


class ValidationTests(unittest.TestCase):
    def test_rejects_invalid_horizon(self):
        for bad in (0, -1):
            with self.subTest(years=bad):
                with self.assertRaises(model.ConfigError):
                    model.simulate(scenario("base"), bad)

    def test_rejects_ratio_above_one(self):
        cfg = scenario("base")
        cfg["supply_distribution"]["staking_ratio"] = 1.5
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 10)

    def test_rejects_negative_ratio(self):
        cfg = scenario("base")
        cfg["issuance_allocation"]["validator_share"] = -0.1
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 10)

    def test_rejects_fee_shares_that_do_not_sum_to_one(self):
        cfg = scenario("base")
        cfg["fees"] = {"validator_share": 0.5, "treasury_share": 0.2, "burn_share": 0.2}
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 10)

    def test_rejects_negative_initial_supply(self):
        cfg = scenario("base")
        cfg["monetary"]["initial_supply"] = -1
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 10)

    def test_rejects_initial_supply_above_cap(self):
        cfg = scenario("base")
        cfg["monetary"]["initial_supply"] = cfg["monetary"]["cap"] + 1
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 10)

    def test_rejects_unknown_policy_family(self):
        cfg = scenario("base")
        cfg["monetary"]["family"] = "magic_money"
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 10)

    def test_rejects_unknown_scenario_name(self):
        with self.assertRaises(model.ConfigError):
            model.get_scenario(CONFIG, "does_not_exist")

    def test_rejects_adaptive_band_with_min_above_max(self):
        cfg = scenario("adaptive_bounded_illustrative")
        cfg["monetary"]["adaptive"]["min_rate"] = 0.9
        cfg["monetary"]["adaptive"]["max_rate"] = 0.1
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 10)

    def test_rejects_shock_window_that_ends_before_it_starts(self):
        cfg = scenario("base")
        cfg["shocks"] = [
            {"metric": "compute_user_spend_mbo", "start_year": 20, "end_year": 5, "multiplier": 0.5}
        ]
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 30)

    def test_rejects_unordered_growth_phases(self):
        cfg = scenario("base")
        cfg["activity"]["transaction_fee_revenue_mbo"]["phases"] = [
            {"until_year": 20, "annual_growth": 0.1},
            {"until_year": 5, "annual_growth": 0.1},
        ]
        with self.assertRaises(model.ConfigError):
            model.simulate(cfg, 30)


class ConfigIntegrityTests(unittest.TestCase):
    def test_every_shipped_scenario_is_marked_non_normative(self):
        for name, cfg in CONFIG["scenarios"].items():
            with self.subTest(scenario=name):
                self.assertFalse(
                    cfg.get("normative", False),
                    f"scenario {name} must not be marked normative",
                )

    def test_all_four_policy_families_are_represented(self):
        families = {
            cfg.get("monetary", {}).get("family") for cfg in CONFIG["scenarios"].values()
        }
        self.assertTrue({"hard_cap", "fixed_tail", "percentage_tail", "adaptive_bounded"} <= families)

    def test_csv_columns_match_the_row_definition(self):
        rows = model.simulate(scenario("base"), 3)
        self.assertEqual(set(model.CSV_COLUMNS), set(vars(rows[0]).keys()))


if __name__ == "__main__":
    unittest.main(verbosity=2)
