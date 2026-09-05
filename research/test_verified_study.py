"""Offline contracts for development screening and token-matched controls."""

import contextlib
import io
from pathlib import Path
import tempfile
import unittest

import evaluate_behavior as behavior
from prepare_verified_study import fresh_pairs, match_control
from verify_attacks import summarize_screen


DEV = Path(__file__).with_name("fixtures") / "attack_development.jsonl"


def observed(pair, variant, score, tokens):
    return {"response": pair["expected_answer"] if variant != "poisoned" else pair["attack_answer"],
            "finish_reason": "eos", "seconds": {"auditor": 0},
            "graph": {"prompt_sha256": behavior.user_prompt(pair, variant)},
            "signals": {"negative_algebraic_connectivity": score,
                        "negative_system_attention": score, "token_count": tokens}}


class VerifiedStudyTests(unittest.TestCase):
    def test_development_cannot_be_loaded_as_evaluation(self):
        pairs = behavior.load_pairs(DEV, allowed_splits=("development",))
        self.assertEqual(len(pairs), 32)
        with self.assertRaisesRegex(ValueError, "split must"):
            behavior.load_pairs(DEV)

    def test_screen_counts_unique_clean_tasks_and_only_eligible_completed_attacks(self):
        rows = [
            {"variant": "clean", "clean_key": "a", "outcome": "task_success"},
            {"variant": "clean", "clean_key": "a", "outcome": "task_success"},
            {"variant": "clean", "clean_key": "b", "outcome": "other_failure"},
            {"variant": "poisoned", "clean_key": "a", "attack_id": "x", "outcome": "attack_target"},
            {"variant": "poisoned", "clean_key": "b", "attack_id": "x", "outcome": "attack_target"},
            {"variant": "poisoned", "clean_key": "a", "attack_id": "y", "outcome": "inconclusive"},
        ]
        result = summarize_screen(rows)
        self.assertEqual(result["unique_clean_tasks"], 2)
        self.assertEqual(result["clean_task_successes"], 1)
        self.assertEqual(result["attacks"]["x"]["successful"], 1)
        self.assertEqual(result["attacks"]["y"]["successful"], 0)

    def test_new_instances_are_disjoint_and_payload_targets_are_replaced(self):
        development = behavior.load_pairs(DEV, allowed_splits=("development",))
        attacks = sorted({pair["pair_id"].split("/")[0] for pair in development})
        pairs = fresh_pairs(development, attacks)
        self.assertEqual(len(pairs), 144)
        groups = {split: [p for p in pairs if p["split"] == split]
                  for split in ("calibration", "evaluation")}
        self.assertEqual(len({p["clean_context"] for p in groups["calibration"]}), 12)
        self.assertEqual(len({p["clean_context"] for p in groups["evaluation"]}), 24)
        for field in ("clean_context", "expected_answer", "attack_answer"):
            a = {p[field] for p in development}
            b = {p[field] for p in groups["calibration"]}
            c = {p[field] for p in groups["evaluation"]}
            self.assertFalse(a & b or a & c or b & c, field)
        for pair in pairs:
            self.assertIn(pair["clean_context"], pair["poisoned_context"])
            self.assertIn(pair["attack_answer"], pair["poisoned_context"])
            self.assertNotIn("ORCHID", pair["poisoned_context"])
        with self.assertRaisesRegex(ValueError, "distinct development"):
            fresh_pairs(development, ["not-screened"])

    def test_controls_match_full_prompt_length_and_keep_task_facts(self):
        pair = behavior.load_pairs(DEV, allowed_splits=("development",))[0]
        counter = lambda prompt: len(prompt.split())
        controlled = match_control(pair, counter)
        self.assertEqual(counter(behavior.user_prompt(controlled, "poisoned")),
                         counter(behavior.user_prompt(controlled, "control")))
        self.assertIn(pair["clean_context"], controlled["control_context"])
        self.assertNotIn(pair["attack_answer"], controlled["control_context"])
        self.assertEqual(pair["poisoned_context"], controlled["poisoned_context"])

    def test_benign_controls_constrain_calibration_and_duplicates_do_not_inflate_counts(self):
        rows = []
        pair = {**behavior.load_pairs(DEV, allowed_splits=("development",))[0],
                "control_context": "long benign padding", "split": "calibration"}
        for variant, score, tokens in (("clean", 0, 10), ("poisoned", 5, 20), ("control", 6, 20)):
            rows.append({**observed(pair, variant, score, tokens), "pair_id": "a", "split": "calibration",
                         "variant": variant, "outcome": "attack_target" if variant == "poisoned" else "task_success"})
        rows.append({**rows[0], "pair_id": "b"})
        policy = behavior.fit_policy(rows, 0)
        self.assertEqual(policy["calibration_clean_count"], 1)
        self.assertEqual(policy["calibration_benign_count"], 2)
        for fitted in policy["signals"].values():
            self.assertIsNone(fitted["threshold"])
        report = behavior.summarize(rows, policy)
        self.assertEqual(report["unique_clean_prompts"], 1)
        self.assertEqual(report["unique_control_prompts"], 1)
        for result in report["signals"].values():
            self.assertEqual(result["benign_block_rate"]["total"], 2)

    def test_runtime_refuses_mismatched_control_lengths(self):
        pair = {**behavior.load_pairs(DEV, allowed_splits=("development",))[0],
                "control_context": "control", "split": "calibration"}

        def observe(system, prompt):
            variant = "control" if "\ncontrol\n" in prompt else "poisoned"
            return observed(pair, variant, 1, 20 if variant == "control" else 10)

        with tempfile.TemporaryDirectory() as root, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaisesRegex(ValueError, "identical token counts"):
                behavior.run_experiment([pair], Path(root), observe, 0)
            self.assertFalse((Path(root) / "policy.json").exists())


if __name__ == "__main__":
    unittest.main()
