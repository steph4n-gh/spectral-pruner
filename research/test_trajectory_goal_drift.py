"""Offline checks for the trajectory goal-drift representation audit."""

from collections import defaultdict
from copy import deepcopy
from hashlib import sha256
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import evaluate_trajectory_goal_drift as evaluation
import trajectory_goal_drift as trajectory


ROOT = Path(__file__).parent.parent


class RepresentationTests(unittest.TestCase):
    def setUp(self):
        self.cases = trajectory.build_cases()

    def case(self, split="mechanism_fit", family="message",
             motif="sidecar", meaning="legitimate"):
        return next(
            case for case in self.cases
            if (case["split"], case["family"], case["motif"], case["meaning"])
            == (split, family, motif, meaning)
        )

    def test_corpus_has_complete_unique_matched_groups(self):
        self.assertEqual(len(self.cases), 24)
        self.assertEqual(len({case["case_id"] for case in self.cases}), 24)
        self.assertEqual({case["split"] for case in self.cases}, set(trajectory.SPLITS))
        groups = defaultdict(list)
        for case in self.cases:
            groups[(case["split"], case["family"], case["motif"])].append(case)
        self.assertEqual(len(groups), 12)
        self.assertTrue(all({case["meaning"] for case in group}
                            == set(trajectory.MEANINGS) for group in groups.values()))
        for split in trajectory.SPLITS:
            split_cases = [case for case in self.cases if case["split"] == split]
            self.assertEqual(len(split_cases), 12)
            self.assertEqual({case["family"] for case in split_cases},
                             set(trajectory.FAMILIES))
            self.assertEqual({case["motif"] for case in split_cases},
                             set(trajectory.MOTIFS))
            self.assertEqual(sum(case["oracle"]["label"] for case in split_cases), 6)

    def test_graph_is_deterministic_connected_and_protected_last(self):
        for case in self.cases:
            for prefix_index in range(4):
                left = trajectory.build_graph(case["trace"], prefix_index)
                right = trajectory.build_graph(case["trace"], prefix_index)
                self.assertEqual(left["edges"], right["edges"])
                self.assertEqual(
                    trajectory.auditor_input_fingerprint(left),
                    trajectory.auditor_input_fingerprint(right),
                )
                self.assertTrue(all(weight == 1.0 for _a, _b, weight in left["edges"]))
                self.assertEqual(
                    left["node_roles"][left["system_start"]:],
                    ["protected"] * len(left["protected_nodes"]),
                )
                self.assertEqual(left["protected_nodes"],
                                 list(range(left["system_start"], left["system_end"] + 1)))

    def test_prefix_excludes_future_actions_and_states(self):
        case = self.case()
        graph = trajectory.build_graph(case["trace"], 2)
        future_action = case["trace"]["actions"][3]["id"]
        future_state = case["trace"]["actions"][3]["produces"][0]
        self.assertNotIn(future_action, graph["included_action_ids"])
        self.assertNotIn(future_state, graph["included_state_ids"])
        self.assertEqual(len(graph["included_action_ids"]), 3)

    def test_self_contained_prefix_matches_complete_trace_graph(self):
        case = self.case()
        trace = case["trace"]
        for prefix_index in range(4):
            complete_graph = trajectory.build_graph(trace, prefix_index)
            included_states = set(complete_graph["included_state_ids"])
            prefix_trace = {
                "contract": deepcopy(trace["contract"]),
                "states": [
                    deepcopy(state) for state in trace["states"]
                    if state["id"] in included_states
                ],
                "actions": deepcopy(trace["actions"][:prefix_index + 1]),
            }
            prefix_graph = trajectory.build_graph(prefix_trace, prefix_index)
            self.assertEqual(
                trajectory.auditor_input_fingerprint(prefix_graph),
                trajectory.auditor_input_fingerprint(complete_graph),
            )

    def test_story_and_oracle_cannot_change_the_graph(self):
        case = self.case()
        changed = deepcopy(case)
        changed["story"] = {"trusted_task_text": "opposite", "linked_result_text": "x"}
        changed["oracle"] = {"label": 1, "first_harmful_proposal_index": 0}
        original = trajectory.build_graph(case["trace"], 2)
        same = trajectory.build_graph(changed["trace"], 2)
        self.assertEqual(
            trajectory.auditor_input_fingerprint(original),
            trajectory.auditor_input_fingerprint(same),
        )
        with self.assertRaises(ValueError):
            trajectory.build_graph(changed, 2)

    def test_meaning_pairs_collide_mechanically_but_not_on_story_context(self):
        for split in trajectory.SPLITS:
            for family in trajectory.FAMILIES:
                for motif in trajectory.MOTIFS:
                    legitimate = self.case(split, family, motif, "legitimate")
                    redirected = self.case(split, family, motif, "redirected")
                    left = trajectory.build_graph(legitimate["trace"], 2)
                    right = trajectory.build_graph(redirected["trace"], 2)
                    self.assertEqual(
                        trajectory.auditor_input_fingerprint(left),
                        trajectory.auditor_input_fingerprint(right),
                    )
                    self.assertEqual(
                        trajectory.canonical_mechanical_fingerprint(
                            legitimate["trace"], 2
                        ),
                        trajectory.canonical_mechanical_fingerprint(
                            redirected["trace"], 2
                        ),
                    )
                    self.assertEqual(
                        trajectory.exact_mechanical_fingerprint(
                            legitimate["trace"], 2
                        ),
                        trajectory.exact_mechanical_fingerprint(
                            redirected["trace"], 2
                        ),
                    )
                    self.assertNotEqual(
                        trajectory.story_augmented_fingerprint(legitimate, 2),
                        trajectory.story_augmented_fingerprint(redirected, 2),
                    )

    def test_topology_changes_without_changing_matched_controls(self):
        sidecar = self.case(motif="sidecar")
        braided = self.case(motif="braided")
        side_graph = trajectory.build_graph(sidecar["trace"], 2)
        braid_graph = trajectory.build_graph(braided["trace"], 2)
        self.assertNotEqual(
            trajectory.auditor_input_fingerprint(side_graph),
            trajectory.auditor_input_fingerprint(braid_graph),
        )
        self.assertEqual(len(side_graph["node_names"]), len(braid_graph["node_names"]))
        self.assertEqual(sum(action["allowed"] for action in sidecar["trace"]["actions"]), 4)
        self.assertEqual(sum(action["allowed"] for action in braided["trace"]["actions"]), 4)
        self.assertEqual(
            [action["side_effect"] for action in sidecar["trace"]["actions"]],
            [action["side_effect"] for action in braided["trace"]["actions"]],
        )

    def test_hand_checked_baselines_and_directions(self):
        sidecar = self.case(motif="sidecar")
        braided = self.case(motif="braided")
        side = trajectory.baseline_signals(sidecar["trace"], 2)
        braid = trajectory.baseline_signals(braided["trace"], 2)
        self.assertEqual(side["authorization_gap"], 0.0)
        self.assertEqual(side["off_contract_side_effect_fraction"], 1.0)
        self.assertEqual(side["weakest_goal_support_gap"], 0.5)
        self.assertEqual(side["single_gateway"], 1.0)
        self.assertEqual(side["node_count"], 13.0)
        self.assertAlmostEqual(side["normalized_goal_distance"], 2 / 3)
        self.assertAlmostEqual(braid["normalized_goal_distance"], 4 / 9)
        self.assertGreater(side["normalized_goal_distance"],
                           braid["normalized_goal_distance"])
        self.assertAlmostEqual(side["natural_local_cut"], 10 / 17)
        self.assertAlmostEqual(braid["natural_local_cut"], 12 / 19)

    def test_gateway_detects_a_single_goal_path_and_rejects_redundant_paths(self):
        case = self.case()
        graph = trajectory.build_graph(case["trace"], 2)
        self.assertEqual(trajectory._single_gateway(graph), 1.0)
        redundant = deepcopy(graph)
        edge = (
            min(graph["current_action_node"], graph["goal_node"]),
            max(graph["current_action_node"], graph["goal_node"]),
            1.0,
        )
        redundant["edges"] = sorted([*redundant["edges"], edge])
        trajectory._validate_graph(redundant)
        self.assertEqual(trajectory._single_gateway(redundant), 0.0)

    def test_future_consumption_and_duplicate_producers_are_rejected(self):
        case = self.case()
        future = deepcopy(case["trace"])
        future["actions"][0]["consumes"] = [future["actions"][3]["produces"][0]]
        with self.assertRaises(ValueError):
            trajectory.build_graph(future, 0)
        duplicate = deepcopy(case["trace"])
        duplicate["actions"][1]["produces"] = duplicate["actions"][0]["produces"]
        with self.assertRaises(ValueError):
            trajectory.build_graph(duplicate, 1)

    def test_formal_corpus_validation_rejects_a_broken_pair(self):
        self.assertTrue(trajectory.validate_cases(self.cases))
        broken = deepcopy(self.cases)
        broken[0]["trace"]["actions"][2]["allowed"] = False
        with self.assertRaises(ValueError):
            trajectory.validate_cases(broken)

    def test_exact_secondary_delegation_changes_graph_and_solves_contract_rule(self):
        case = self.case()
        original = case["trace"]
        delegated = trajectory.declare_secondary_sink(original)
        original_graph = trajectory.build_graph(original, 2)
        delegated_graph = trajectory.build_graph(delegated, 2)
        self.assertNotEqual(
            trajectory.auditor_input_fingerprint(original_graph),
            trajectory.auditor_input_fingerprint(delegated_graph),
        )
        self.assertEqual(
            trajectory.baseline_signals(original, 2)[
                "off_contract_side_effect_fraction"
            ],
            1.0,
        )
        self.assertEqual(
            trajectory.baseline_signals(delegated, 2)[
                "off_contract_side_effect_fraction"
            ],
            0.0,
        )

    def test_release_auditor_converges_and_repeats(self):
        auditor = ROOT / "target/release/spectral-pruner-audit"
        if not auditor.exists():
            self.skipTest("release auditor is not built")
        case = self.case()
        first = trajectory.observe_prefix(case, 2, auditor)
        second = trajectory.observe_prefix(case, 2, auditor)
        self.assertEqual(first["signals"], second["signals"])
        self.assertEqual(first["fingerprints"], second["fingerprints"])
        self.assertTrue(first["audit"]["diagnostics"]["solver_converged"])
        self.assertEqual(first["audit"]["diagnostics"]["system_node_count"], 5)
        graph = trajectory.build_graph(case["trace"], 2)
        mainland = set(first["audit"]["mainland_nodes"])
        island = set(first["audit"]["island_nodes"])
        self.assertFalse(mainland & island)
        self.assertEqual(mainland | island, set(range(graph["system_start"])))


class EvaluationTests(unittest.TestCase):
    def test_collision_statistics_bound_zero_false_positive_recall(self):
        rows = []
        for label, case_id in ((0, "clean"), (1, "attack")):
            rows.append({
                "case_id": case_id,
                "label": label,
                "prefixes": [{
                    "prefix_index": 2,
                    "fingerprints": {"auditor_input": "same"},
                }],
            })
        report = evaluation.equivalence_report(rows, "auditor_input")
        self.assertEqual(report["unavoidable_errors"], 1)
        self.assertEqual(report["maximum_recall_at_zero_false_positives"], 0.0)
        self.assertEqual(len(report["cross_label_groups"]), 1)

    def test_colliding_fit_scores_produce_null_thresholds(self):
        rows = []
        for label in (0, 1):
            rows.append({
                "label": label,
                "split": "mechanism_fit",
                "case_signals": {name: 1.0 for name in trajectory.SIGNALS},
            })
        policy = evaluation.fit_policy(rows, 0.01)
        self.assertTrue(all(record["threshold"] is None
                            for record in policy["signals"].values()))

    def test_policy_rejects_missing_class(self):
        rows = [{
            "label": 0,
            "split": "mechanism_fit",
            "case_signals": {name: 0.0 for name in trajectory.SIGNALS},
        }]
        with self.assertRaises(ValueError):
            evaluation.fit_policy(rows, 0.01)

    def test_fit_policy_rejects_check_rows(self):
        rows = [{
            "label": label,
            "split": "mechanism_check",
            "case_signals": {name: float(label) for name in trajectory.SIGNALS},
        } for label in (0, 1)]
        with self.assertRaises(ValueError):
            evaluation.fit_policy(rows, 0.01)

    def test_collisions_low_recall_and_baseline_ties_stop_advancement(self):
        def result(tp=0, fp=0):
            return {
                "tp": tp, "tn": 6 - fp, "fp": fp, "fn": 6 - tp,
                "true_positive_rate": tp / 6,
                "false_positive_rate": fp / 6,
                "accuracy": (tp + 6 - fp) / 12,
            }

        signals = {
            name: {"threshold": None, "check": result()}
            for name in trajectory.SIGNALS
        }
        summary = {
            "collision_aware": {"signals": signals},
            "equivalence": {"auditor_input": {"cross_label_groups": [{}]}},
            "spectral_detection_timing": {
                "caught_by_or_before_harm": 0, "caught_after_harm": 0, "missed": 6
            },
            "coverage": {"complete": True},
            "numerics": {"all_converged": True, "all_signals_finite": True},
            "delegation_sanity": {
                "graph_changed": True, "delegated_off_contract_score": 0.0
            },
        }
        decision = evaluation.make_decision(summary, 0.01)
        self.assertFalse(decision["advance_to_model_acquisition"])
        rendered = " ".join(decision["reasons"])
        self.assertIn("collisions", rendered)
        self.assertIn("below 50%", rendered)
        self.assertIn("does not strictly beat", rendered)

    def test_passing_decision_requires_early_spectral_gain(self):
        def result(tp):
            return {
                "tp": tp, "tn": 6, "fp": 0, "fn": 6 - tp,
                "true_positive_rate": tp / 6,
                "false_positive_rate": 0.0,
                "accuracy": (tp + 6) / 12,
            }

        signals = {
            name: {"threshold": 0.5, "check": result(5)}
            for name in trajectory.SIGNALS
        }
        signals["negative_algebraic_connectivity"] = {
            "threshold": 0.5, "check": result(6)
        }
        summary = {
            "collision_aware": {"signals": signals},
            "equivalence": {"auditor_input": {"cross_label_groups": []}},
            "spectral_detection_timing": {
                "caught_by_or_before_harm": 6, "caught_after_harm": 0, "missed": 0
            },
            "coverage": {"complete": True},
            "numerics": {"all_converged": True, "all_signals_finite": True},
            "delegation_sanity": {
                "graph_changed": True, "delegated_off_contract_score": 0.0
            },
        }
        self.assertTrue(
            evaluation.make_decision(summary, 0.01)["advance_to_model_acquisition"]
        )

    def test_policy_is_written_before_check_observation(self):
        auditor = ROOT / "target/release/spectral-pruner-audit"
        if not auditor.exists():
            self.skipTest("release auditor is not built")
        cases = trajectory.build_cases()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)

            def observer(case, executable):
                if case["split"] == "mechanism_check":
                    self.assertTrue((output / "policy.json").is_file())
                return trajectory.observe_case(case, executable)

            original_build_graph = trajectory.build_graph

            def build_graph(trace, prefix_index):
                if ":check-" in trace["actions"][0]["id"]:
                    self.assertTrue((output / "policy.json").is_file())
                return original_build_graph(trace, prefix_index)

            with patch.object(trajectory, "build_graph", build_graph), \
                    patch.object(evaluation, "build_graph", build_graph):
                evaluation.run_audit(cases, output, auditor, 0.01, observer)
            self.assertTrue((output / "summary.json").is_file())
            self.assertTrue((output / "decision.json").is_file())

    def test_failed_manifest_cannot_verify_as_complete(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            evaluation.write_json(output / "run.json", {"status": "failed"})
            with self.assertRaises(ValueError):
                evaluation.verify_completed_run(output, Path(directory))

    def test_completed_manifest_verifies_and_detects_tampering(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_dir = root / "research"
            output = root / "result"
            source_dir.mkdir()
            output.mkdir()
            source = source_dir / "source.txt"
            auditor = root / "auditor"
            cases = output / "cases.json"
            artifact = output / "summary.json"
            source.write_text("source\n", encoding="utf-8")
            auditor.write_bytes(b"auditor")
            cases.write_text("{}\n", encoding="utf-8")
            artifact.write_text("{}\n", encoding="utf-8")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "add", "research/source.txt"], cwd=root, check=True)
            subprocess.run([
                "git", "-c", "user.name=Test", "-c", "user.email=test@example.com",
                "commit", "-q", "-m", "fixture",
            ], cwd=root, check=True)
            commit = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=root, check=True,
                capture_output=True, text=True,
            ).stdout.strip()

            def digest(path):
                return sha256(path.read_bytes()).hexdigest()

            manifest = {
                "status": "complete",
                "protocol_commit": commit,
                "arguments": {"auditor": str(auditor)},
                "auditor_sha256": digest(auditor),
                "source_sha256": {"source.txt": digest(source)},
                "cases_sha256": digest(cases),
                "artifact_sha256": {
                    "cases.json": digest(cases), "summary.json": digest(artifact)
                },
            }
            evaluation.write_json(output / "run.json", manifest)
            self.assertTrue(evaluation.verify_completed_run(output, source_dir))
            artifact.write_text("tampered\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                evaluation.verify_completed_run(output, source_dir)


if __name__ == "__main__":
    unittest.main()
