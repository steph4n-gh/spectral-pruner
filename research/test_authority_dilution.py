"""Offline checks for the ambiguous authority dilution representation audit."""

from collections import defaultdict
from pathlib import Path
import tempfile
import unittest

import authority_dilution as dilution
import evaluate_authority_dilution as evaluation


ROOT = Path(__file__).parent.parent


class RepresentationTests(unittest.TestCase):
    def setUp(self):
        self.cases = dilution.build_cases()

    def test_corpus_fixes_splits_families_topologies_and_semantics(self):
        self.assertEqual(len(self.cases), 48)
        self.assertEqual({case["split"] for case in self.cases}, set(dilution.SPLITS))
        self.assertEqual({case["tool_family"] for case in self.cases},
                         set(dilution.FAMILIES))
        self.assertEqual({case["topology"] for case in self.cases},
                         set(dilution.TOPOLOGIES))
        self.assertEqual({case["semantic"] for case in self.cases},
                         set(dilution.SEMANTICS))
        self.assertEqual(sum(case["split"] == "mechanism_fit" for case in self.cases),
                         24)

    def test_every_field_and_contributor_reaches_authority(self):
        for case in self.cases:
            signals = dilution.baseline_signals(dilution.build_graph(case))
            self.assertEqual(signals["field_authority_gap"], 0.0)
            self.assertEqual(signals["contributor_authority_gap"], 0.0)
            self.assertEqual(signals["any_untrusted_contributor"], 1.0)

    def test_semantic_labels_collide_on_identical_graphs(self):
        labels = defaultdict(set)
        groups = defaultdict(set)
        for case in self.cases:
            fingerprint = dilution.graph_fingerprint(dilution.build_graph(case))
            labels[(case["split"], fingerprint)].add(case["label"])
            groups[(case["split"], case["group"], case["topology"])].add(fingerprint)
        self.assertTrue(labels)
        self.assertTrue(all(values == {0, 1} for values in labels.values()))
        self.assertTrue(all(len(values) == 1 for values in groups.values()))

    def test_linear_topology_rules_expose_the_naive_confounder(self):
        for family in dilution.FAMILIES:
            base = {
                "tool_family": family,
                "contributor_count": 4,
            }
            distributed = dilution.baseline_signals(dilution.build_graph(
                {**base, "topology": "distributed"}
            ))
            nested = dilution.baseline_signals(dilution.build_graph(
                {**base, "topology": "nested"}
            ))
            self.assertGreater(nested["contributor_bottleneck"],
                               distributed["contributor_bottleneck"])
            self.assertGreater(nested["normalized_max_authority_distance"],
                               distributed["normalized_max_authority_distance"])
            self.assertEqual(nested["untrusted_field_source_fraction"],
                             distributed["untrusted_field_source_fraction"])


class EvaluationTests(unittest.TestCase):
    def test_collisions_and_a_tied_baseline_stop_advancement(self):
        def result(tp, fp=0):
            return {"tp": tp, "tn": 12 - fp, "fp": fp, "fn": 12 - tp,
                    "true_positive_rate": tp / 12,
                    "false_positive_rate": fp / 12,
                    "accuracy": (tp + 12 - fp) / 24}

        report = {"signals": {
            name: {"check": result(6)} for name in dilution.SIGNALS
        }}
        decision = evaluation.decision(report, [{"graph_fingerprint": "same"}])
        self.assertFalse(decision["advance_to_model_acquisition"])
        self.assertIn("share graph fingerprints", " ".join(decision["reasons"]))
        self.assertIn("did not beat", " ".join(decision["reasons"]))

    def test_release_auditor_sees_naive_topology_difference(self):
        auditor = ROOT / "target/release/spectral-pruner-audit"
        if not auditor.exists():
            self.skipTest("release auditor is not built")
        base = {"tool_family": "message", "contributor_count": 3}
        distributed = dilution.observe_case(
            {**base, "topology": "distributed"}, auditor
        )
        nested = dilution.observe_case({**base, "topology": "nested"}, auditor)
        self.assertGreater(
            nested["signals"]["negative_algebraic_connectivity"],
            distributed["signals"]["negative_algebraic_connectivity"],
        )


if __name__ == "__main__":
    unittest.main()
