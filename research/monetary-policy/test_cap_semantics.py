#!/usr/bin/env python3
"""Tests for the cap-semantics comparative study.

These guard the comparison, not a conclusion. No test asserts that any cap
family produces a better supply, budget or trajectory. They assert that the
three families are genuinely comparable, that each one enforces the
invariant it claims, that burn is the only thing the no-burn control
removes, and that the whole thing is deterministic.

Run from the repository root:

    python3 research/monetary-policy/test_cap_semantics.py
"""

from __future__ import annotations

import copy
import math
import sys
import unittest
from dataclasses import fields
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import cap_semantics as cs  # noqa: E402
import model  # noqa: E402
import primary_sensitivity as ps  # noqa: E402


CONFIG = model.load_config(model.DEFAULT_CONFIG)
ALL = cs.run_all(CONFIG)
GRID = ps.schedule_grid()


def terminal(burn_environment, semantics):
    return [
        o
        for o in ALL
        if o.burn_environment == burn_environment
        and o.cap_semantics == semantics
        and o.year == cs.HORIZON_YEARS
    ]


def schedules_of(observations):
    return [(o.initial_issuance, o.step_interval, o.decay_factor) for o in observations]


class DesignTests(unittest.TestCase):
    def test_exactly_eighty_schedules_per_family(self):
        for env in cs.BURN_ENVIRONMENTS:
            for semantics in cs.CAP_FAMILIES:
                with self.subTest(env=env, semantics=semantics):
                    self.assertEqual(len(terminal(env, semantics)), 80)

    def test_exactly_two_hundred_and_forty_burn_enabled_runs(self):
        runs = {
            (o.cap_semantics, o.initial_issuance, o.step_interval, o.decay_factor)
            for o in ALL
            if o.burn_environment == cs.WITH_BURN
        }
        self.assertEqual(len(runs), 240)

    def test_no_burn_control_covers_the_same_runs(self):
        for env in cs.BURN_ENVIRONMENTS:
            runs = {
                (o.cap_semantics, o.initial_issuance, o.step_interval, o.decay_factor)
                for o in ALL
                if o.burn_environment == env
            }
            with self.subTest(env=env):
                self.assertEqual(len(runs), 240)

    def test_schedule_tuples_identical_across_families(self):
        for env in cs.BURN_ENVIRONMENTS:
            reference = schedules_of(terminal(env, cs.CAP_FAMILIES[0]))
            self.assertEqual(reference, GRID)
            for semantics in cs.CAP_FAMILIES[1:]:
                with self.subTest(env=env, semantics=semantics):
                    self.assertEqual(schedules_of(terminal(env, semantics)), reference)

    def test_historical_reference_appears_exactly_once_per_family(self):
        self.assertIn(cs.HISTORICAL_REFERENCE, GRID)
        for env in cs.BURN_ENVIRONMENTS:
            for semantics in cs.CAP_FAMILIES:
                flagged = [o for o in terminal(env, semantics) if o.is_historical_reference]
                with self.subTest(env=env, semantics=semantics):
                    self.assertEqual(len(flagged), 1)
                    self.assertEqual(
                        (
                            flagged[0].initial_issuance,
                            flagged[0].step_interval,
                            flagged[0].decay_factor,
                        ),
                        cs.HISTORICAL_REFERENCE,
                    )

    def test_deterministic_ordering(self):
        again = cs.run_all(CONFIG)
        key = lambda o: (  # noqa: E731
            o.burn_environment,
            o.cap_semantics,
            o.initial_issuance,
            o.step_interval,
            o.decay_factor,
            o.year,
        )
        self.assertEqual([key(o) for o in ALL], [key(o) for o in again])

    def test_results_are_reproducible(self):
        self.assertEqual(ALL, cs.run_all(CONFIG))

    def test_csv_columns_match_the_observation_definition(self):
        self.assertEqual(set(cs.CSV_COLUMNS), set(vars(ALL[0]).keys()))


class InvariantTests(unittest.TestCase):
    def test_lifetime_cumulative_issuance_never_exceeds_the_cap(self):
        for env in cs.BURN_ENVIRONMENTS:
            for o in ALL:
                if o.burn_environment == env and o.cap_semantics == cs.LIFETIME:
                    self.assertLessEqual(o.cumulative_gross_issuance, cs.CAP + cs.EPSILON)

    def test_outstanding_supply_never_exceeds_the_cap_under_outstanding_semantics(self):
        for o in ALL:
            if o.cap_semantics == cs.OUTSTANDING:
                self.assertLessEqual(o.outstanding_supply, cs.CAP + cs.EPSILON)

    def test_dual_cumulative_issuance_never_exceeds_the_cap(self):
        for o in ALL:
            if o.cap_semantics == cs.DUAL:
                self.assertLessEqual(o.cumulative_gross_issuance, cs.CAP + cs.EPSILON)

    def test_dual_outstanding_supply_never_exceeds_the_cap(self):
        for o in ALL:
            if o.cap_semantics == cs.DUAL:
                self.assertLessEqual(o.outstanding_supply, cs.CAP + cs.EPSILON)

    def test_outstanding_semantics_may_issue_beyond_the_cap_in_aggregate(self):
        # Not a defect and not an endorsement: it is the arithmetic
        # consequence of measuring headroom against a burned-down supply.
        # Recorded so the difference between the families stays visible.
        over = [
            o
            for o in terminal(cs.WITH_BURN, cs.OUTSTANDING)
            if o.cumulative_gross_issuance > cs.CAP + cs.EPSILON
        ]
        self.assertTrue(over, "expected the outstanding family to exceed C in aggregate")
        for o in over:
            self.assertLessEqual(o.outstanding_supply, cs.CAP + cs.EPSILON)

    def test_burn_never_restores_lifetime_headroom(self):
        # The lifetime accumulator must be monotonically non-decreasing over
        # the run, whatever the burns do to supply.
        base = cs.controlled_scenario(CONFIG)
        scenario = cs.build_scenario(base, (12_614_400, 20, 0.90), cs.LIFETIME, cs.WITH_BURN)
        policy = model.build_policy(scenario["monetary"])
        rows = model.simulate(scenario, cs.HORIZON_YEARS)
        granted = 0.0
        for row in rows:
            self.assertGreaterEqual(row.gross_issuance, -cs.EPSILON)
            granted += row.gross_issuance
            self.assertLessEqual(granted, cs.CAP + cs.EPSILON)
        self.assertGreater(sum(r.burned for r in rows), 0.0, "control needs real burns")
        self.assertIsNotNone(policy)

    def test_burn_may_restore_outstanding_headroom(self):
        # Same schedule, same everything, outstanding semantics: the run
        # issues strictly more in total than the cap, which can only happen
        # because burns reopened headroom.
        base = cs.controlled_scenario(CONFIG)
        schedule = (12_614_400, 20, 0.90)
        out_rows = model.simulate(
            cs.build_scenario(base, schedule, cs.OUTSTANDING, cs.WITH_BURN),
            cs.HORIZON_YEARS,
        )
        life_rows = model.simulate(
            cs.build_scenario(base, schedule, cs.LIFETIME, cs.WITH_BURN),
            cs.HORIZON_YEARS,
        )
        out_total = sum(r.gross_issuance for r in out_rows)
        life_total = sum(r.gross_issuance for r in life_rows)
        self.assertGreater(out_total, cs.CAP + cs.EPSILON)
        self.assertGreater(out_total, life_total + cs.EPSILON)

    def test_dual_respects_the_smaller_of_both_headrooms(self):
        # Constructed so the two headrooms disagree: a non-zero starting
        # supply with no burns makes the outstanding headroom the binding
        # one, which is the opposite of the controlled scenario.
        base = cs.strip_burn(cs.controlled_scenario(CONFIG))
        base["monetary"]["initial_supply"] = 20_000_000
        totals = {}
        ending = {}
        for semantics in cs.CAP_FAMILIES:
            scenario = ps.build_scenario(base, (12_614_400, 20, 0.90), cs.CAP)
            scenario["monetary"]["family"] = cs.family_name(semantics)
            scenario["monetary"]["initial_supply"] = 20_000_000
            rows = model.simulate(scenario, cs.HORIZON_YEARS)
            totals[semantics] = sum(r.gross_issuance for r in rows)
            ending[semantics] = rows[-1].ending_supply
            self.assertLessEqual(totals[semantics], cs.CAP + cs.EPSILON)
        # Dual follows the binding constraint, which here is the outstanding
        # one, and is strictly tighter than lifetime alone.
        self.assertAlmostEqual(totals[cs.DUAL], totals[cs.OUTSTANDING], places=6)
        self.assertLess(totals[cs.DUAL], totals[cs.LIFETIME] - cs.EPSILON)
        for semantics in (cs.OUTSTANDING, cs.DUAL):
            self.assertLessEqual(ending[semantics], cs.CAP + cs.EPSILON)
        # A lifetime cap bounds what is created, not what exists: with a
        # pre-existing supply it lets outstanding supply pass C. Recorded as
        # a property of the family, not as a defect and not as a preference.
        self.assertGreater(ending[cs.LIFETIME], cs.CAP + cs.EPSILON)

    def test_dual_matches_lifetime_when_the_start_is_empty(self):
        # outstanding_headroom - lifetime_headroom = cumulative_burn -
        # initial_supply. With initial_supply = 0 and non-negative burns the
        # lifetime bound is always the binding one, so dual collapses onto
        # it. Observed, and true by that identity rather than by accident.
        self.assertEqual(
            CONFIG["scenarios"][cs.CONTROLLED_SCENARIO]["monetary"]["initial_supply"], 0
        )
        for env in cs.BURN_ENVIRONMENTS:
            life = terminal(env, cs.LIFETIME)
            dual = terminal(env, cs.DUAL)
            for a, b in zip(life, dual):
                with self.subTest(env=env, schedule=(a.initial_issuance, a.step_interval)):
                    self.assertAlmostEqual(
                        a.cumulative_gross_issuance, b.cumulative_gross_issuance, places=6
                    )
                    self.assertAlmostEqual(a.outstanding_supply, b.outstanding_supply, places=6)


class ControlTests(unittest.TestCase):
    def test_only_family_and_burn_environment_differ_from_the_controlled_scenario(self):
        base = cs.controlled_scenario(CONFIG)
        built = cs.build_scenario(base, cs.HISTORICAL_REFERENCE, cs.DUAL, cs.WITH_BURN)
        stripped_base = copy.deepcopy(base)
        stripped_built = copy.deepcopy(built)
        for s in (stripped_base, stripped_built):
            s["monetary"].pop("family")
        self.assertEqual(stripped_base, stripped_built)

    def test_cap_families_share_every_non_cap_parameter(self):
        base = cs.controlled_scenario(CONFIG)
        built = {
            semantics: cs.build_scenario(base, cs.HISTORICAL_REFERENCE, semantics, cs.WITH_BURN)
            for semantics in cs.CAP_FAMILIES
        }
        reference = built[cs.LIFETIME]
        for semantics, scenario in built.items():
            with self.subTest(semantics=semantics):
                for key in ("supply_distribution", "issuance_allocation", "fees", "activity", "price", "shocks"):
                    self.assertEqual(scenario.get(key), reference.get(key), key)
                self.assertEqual(
                    scenario["monetary"]["primary_emission"],
                    reference["monetary"]["primary_emission"],
                )
                self.assertEqual(scenario["monetary"]["cap"], cs.CAP)

    def test_no_burn_control_changes_burn_and_nothing_that_pays_validators(self):
        base = cs.controlled_scenario(CONFIG)
        burning = cs.build_scenario(base, cs.HISTORICAL_REFERENCE, cs.LIFETIME, cs.WITH_BURN)
        control = cs.build_scenario(base, cs.HISTORICAL_REFERENCE, cs.LIFETIME, cs.NO_BURN)
        self.assertGreater(burning["fees"]["burn_share"], 0.0)
        self.assertEqual(control["fees"]["burn_share"], 0.0)
        # Validators are paid identically, so the control isolates burn and
        # does not smuggle in a security-budget change.
        self.assertEqual(
            control["fees"]["validator_share"], burning["fees"]["validator_share"]
        )
        self.assertAlmostEqual(sum(control["fees"].values()), 1.0, places=9)
        for key in ("supply_distribution", "issuance_allocation", "activity", "price", "shocks", "monetary"):
            self.assertEqual(control.get(key), burning.get(key), key)

    def test_no_burn_control_actually_removes_burn(self):
        for o in ALL:
            if o.burn_environment == cs.NO_BURN:
                self.assertEqual(o.cumulative_burn, 0.0)
        with_burn = [o for o in ALL if o.burn_environment == cs.WITH_BURN and o.year == 100]
        self.assertTrue(any(o.cumulative_burn > 0.0 for o in with_burn))

    def test_registry_additions_do_not_clobber_the_original_families(self):
        for key in ("hard_cap", "fixed_tail", "percentage_tail", "adaptive_bounded"):
            with self.subTest(family=key):
                self.assertIn(key, model.POLICY_FAMILIES)
        self.assertIs(model.POLICY_FAMILIES["hard_cap"], model.HardCapPolicy)
        for semantics in cs.CAP_FAMILIES:
            self.assertIn(cs.family_name(semantics), model.POLICY_FAMILIES)

    def test_outstanding_family_reproduces_the_current_model_behaviour(self):
        # The study must not quietly redefine what PR #72 and PR #75 already
        # measured: the outstanding family has to be the current model.
        base = cs.controlled_scenario(CONFIG)
        for schedule in ((12_614_400, 20, 0.90), cs.HISTORICAL_REFERENCE, (788_400, 2, 0.25)):
            with self.subTest(schedule=schedule):
                current = model.simulate(ps.build_scenario(base, schedule, cs.CAP), 100)
                study = model.simulate(
                    cs.build_scenario(base, schedule, cs.OUTSTANDING, cs.WITH_BURN), 100
                )
                self.assertEqual(current, study)


class OutputTests(unittest.TestCase):
    def test_no_negative_issuance_or_supply(self):
        for o in ALL:
            self.assertGreaterEqual(o.gross_issuance, 0.0)
            self.assertGreaterEqual(o.cumulative_gross_issuance, 0.0)
            self.assertGreaterEqual(o.outstanding_supply, 0.0)
            self.assertGreaterEqual(o.cumulative_burn, 0.0)

    def test_no_nan_or_infinite_values(self):
        numeric = [
            f.name for f in fields(cs.Observation) if f.type in ("float", "int")
        ]
        for o in ALL:
            for name in numeric:
                value = getattr(o, name)
                self.assertFalse(
                    math.isnan(value) or math.isinf(value), f"{name} not finite"
                )

    def test_accounting_identities_hold_for_every_family(self):
        base = cs.controlled_scenario(CONFIG)
        for env in cs.BURN_ENVIRONMENTS:
            for semantics in cs.CAP_FAMILIES:
                for schedule in (cs.HISTORICAL_REFERENCE, (12_614_400, 20, 0.90)):
                    with self.subTest(env=env, semantics=semantics, schedule=schedule):
                        rows = model.simulate(
                            cs.build_scenario(base, schedule, semantics, env), 100
                        )
                        self.assertEqual(model.identity_violations(rows), [])

    def test_cumulative_issuance_matches_the_policy_accumulator(self):
        # The accumulator inside the policy and the externally summed
        # gross issuance must be the same number, or the lifetime headroom
        # was measured against something the report does not show.
        base = cs.controlled_scenario(CONFIG)
        scenario = cs.build_scenario(base, (6_307_200, 10, 0.75), cs.LIFETIME, cs.WITH_BURN)
        policy = model.build_policy(scenario["monetary"])
        rows = model.simulate(scenario, cs.HORIZON_YEARS)
        replay = 0.0
        for row in rows:
            if row.year >= 1:
                replay += policy.gross_issuance(
                    row.year,
                    row.starting_supply,
                    {"previous_staked_value": 0.0, "previous_security_budget_value": 0.0},
                )
        self.assertAlmostEqual(replay, policy.cumulative_granted, places=6)
        self.assertAlmostEqual(
            replay, sum(r.gross_issuance for r in rows), places=6
        )

    def test_summary_reports_families_separately(self):
        text = cs.format_summary(ALL, CONFIG)
        for semantics in cs.CAP_FAMILIES:
            self.assertIn(semantics, text)
        self.assertIn("NOT A DECISION", text)
        self.assertIn(cs.NO_BURN, text)


class HistoricalReferenceTests(unittest.TestCase):
    def test_documented_schedule_is_identical_across_all_three_families(self):
        traj = cs.historical_reference_rows(CONFIG)
        reference = traj[cs.LIFETIME]
        for semantics in cs.CAP_FAMILIES[1:]:
            with self.subTest(semantics=semantics):
                self.assertIsNone(
                    cs.first_divergence_year(reference, traj[semantics]),
                    "expected no divergence over the 100-year horizon",
                )

    def test_documented_schedule_never_clamps_under_any_family(self):
        for env in cs.BURN_ENVIRONMENTS:
            for semantics in cs.CAP_FAMILIES:
                row = [o for o in terminal(env, semantics) if o.is_historical_reference][0]
                with self.subTest(env=env, semantics=semantics):
                    self.assertEqual(row.first_clamp_year, "")
                    self.assertEqual(row.clamped_periods, 0)

    def test_first_divergence_year_detects_a_real_divergence(self):
        # Guards the detector itself: a schedule that does clamp must be
        # reported as divergent, or the identical verdict above is worthless.
        base = cs.controlled_scenario(CONFIG)
        left = model.simulate(
            cs.build_scenario(base, (12_614_400, 20, 0.90), cs.LIFETIME, cs.WITH_BURN), 100
        )
        right = model.simulate(
            cs.build_scenario(base, (12_614_400, 20, 0.90), cs.OUTSTANDING, cs.WITH_BURN), 100
        )
        self.assertIsNotNone(cs.first_divergence_year(left, right))


if __name__ == "__main__":
    unittest.main(verbosity=2)
