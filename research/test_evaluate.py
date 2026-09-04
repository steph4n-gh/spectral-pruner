"""Offline evaluation and CLI contract checks; no model downloads required."""

import argparse
import contextlib
import copy
import io
import json
import math
import os
import subprocess
import tempfile
import unittest
from hashlib import sha256
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import evaluate
from calibrate import choose_threshold


AUDITOR = Path(os.environ.get("SPECTRAL_PRUNER_AUDITOR", "target/release/spectral-pruner-audit")).resolve()


class EvaluationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binary = self.root / "auditor"
        self.binary.write_bytes(b"auditor identity fixture")
        with patch("sys.argv", ["evaluate.py", "--jsonl", "fixture.jsonl",
                                "--model", "fixture-model", "--system", "instruction",
                                "--output-dir", str(self.root)]):
            self.args = evaluate.parse_args()
        self.args.auditor = self.binary
        self.dataset = {"kind": "jsonl", "sha256": "dataset-digest"}

    def identity(self, args=None):
        return evaluate.run_identity(args or self.args, self.dataset, "model-commit", "cpu")

    def test_resume_identity_changes_with_sampling_labels_and_solver(self):
        original = self.identity()
        for key, value in {
            "text_field": "body", "label_field": "attack", "attack_label": "true",
            "max_per_class": 10, "seed": 42, "max_length": 1024,
            "max_iterations": 20_000, "tolerance": 1e-8,
        }.items():
            with self.subTest(key=key):
                args = copy.copy(self.args)
                setattr(args, key, value)
                self.assertNotEqual(original, self.identity(args))
        self.binary.write_bytes(b"updated auditor")
        self.assertNotEqual(original, self.identity())

    def test_resume_validates_saved_rows_and_manifest(self):
        identity = self.identity()
        manifest = self.root / "run.json"
        results = self.root / "predictions.jsonl"
        records = [{"source_index": 2, "text": "content", "label": 1}]
        row = {"source_index": 2, "label": 1, "text_sha256": sha256(b"content").hexdigest()}
        manifest.write_text(json.dumps(identity))
        results.write_text(json.dumps(row) + "\n")
        self.assertEqual(evaluate.resume_rows(manifest, results, identity, records), [row])
        for bad_rows in [[row, row], [{**row, "label": 0}], [{**row, "source_index": 9}],
                         [{**row, "text_sha256": "wrong"}]]:
            results.write_text("".join(json.dumps(value) + "\n" for value in bad_rows))
            with self.assertRaisesRegex(ValueError, "saved predictions"):
                evaluate.resume_rows(manifest, results, identity, records)
        with self.assertRaisesRegex(ValueError, "configuration has changed"):
            evaluate.resume_rows(manifest, results, {**identity, "seed": 99}, records)
        manifest.unlink()
        with self.assertRaisesRegex(ValueError, "run.json is missing"):
            evaluate.resume_rows(manifest, results, identity, records)

    def test_auc_handles_ties_and_direction(self):
        self.assertEqual(evaluate.roc_auc([0, 1, 0, 1], [1, 1, 1, 1]), 0.5)
        self.assertEqual(evaluate.roc_auc([0, 0, 1, 1], [0, 1, 2, 3]), 1.0)
        self.assertEqual(evaluate.roc_auc([0, 0, 1, 1], [3, 2, 1, 0]), 0.0)

    def test_prediction_files_omit_text_and_tokens_but_preserve_provenance(self):
        records = [{"source_index": i, "text": f"private sample {i}", "label": i}
                   for i in range(2)]
        graph = SimpleNamespace(node_count=7, selected_layers=[3])
        metadata = {"node_count": 7, "prompt_sha256": "prompt-digest",
                    "tokens": ["private", " sample", " 0"], "model_revision": "model-commit"}
        runtime = SimpleNamespace(
            load_model=lambda *args: (None, SimpleNamespace(
                config=SimpleNamespace(_commit_hash="model-commit")), "cpu"),
            extract_attention_bundle=lambda *args, **kwargs: SimpleNamespace(
                aggregate=graph, layers=[graph]),
            graph_metadata=lambda *args: copy.deepcopy(metadata),
        )
        audit = {"connectivity_score": 0.2, "action": "ALLOW", "diagnostics": {
            "solver_converged": True, "solver_iterations": 12, "relative_residual": 1e-10,
            "conductance": 0.3, "density_ratio": 0.4, "density_ratio_status": "finite",
            "instruction_connection": 0.5, "connectivity_triggered": False,
            "density_triggered": False, "instruction_neglect_triggered": False,
            "single_token_triggered": False,
        }}
        with patch.dict("sys.modules", {"attention_graph": runtime}), \
                patch.object(evaluate, "parse_args", return_value=self.args), \
                patch.object(evaluate, "load_records", return_value=(records, self.dataset)), \
                patch.object(evaluate, "run_auditor", return_value=audit), \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            evaluate.main()
        rows = [json.loads(line) for line in (self.root / "predictions.jsonl").read_text().splitlines()]
        self.assertEqual(len(rows), 2)
        for record, row in zip(records, rows):
            self.assertNotIn("tokens", row["graph"])
            self.assertEqual(row["graph"]["prompt_sha256"], "prompt-digest")
            self.assertEqual(row["graph"]["model_revision"], "model-commit")
            self.assertEqual(row["text_sha256"], sha256(record["text"].encode()).hexdigest())
            self.assertEqual(row["signals"]["token_count"], 7)
        for filename in ("predictions.jsonl", "run.json", "summary.json"):
            contents = (self.root / filename).read_text()
            self.assertNotIn('"tokens"', contents)
            self.assertNotIn("private", contents)

    def test_calibration_respects_false_positive_ceiling(self):
        rows = [{"label": label, "score": score} for label, score in
                [(0, 0), (0, 2), (1, 1), (1, 3)]]
        threshold, metrics = choose_threshold(rows, lambda row: row["score"], 0.0)
        self.assertEqual(threshold, 3)
        self.assertEqual(metrics["false_positive_rate"], 0.0)
        self.assertEqual(metrics["true_positive_rate"], 0.5)


class CliContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not AUDITOR.is_file():
            raise RuntimeError("build the release audit CLI before running research tests")

    def audit(self, nodes, edges, *flags, start=0, end=0):
        return subprocess.run([str(AUDITOR), "--nodes", str(nodes),
                               "--system-start", str(start), "--system-end", str(end),
                               *flags, "-"], input=edges, text=True, capture_output=True)

    def test_version_and_help_need_no_graph(self):
        for flag in ("--version", "-V"):
            result = subprocess.run([str(AUDITOR), flag], text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertRegex(result.stdout, r"^spectral-pruner-audit \d+\.\d+\.\d+")
        result = subprocess.run([str(AUDITOR), "--help"], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--version", result.stdout)

    def test_invalid_tau_and_overflow_emit_errors_without_a_verdict(self):
        for tau in ("NaN", "nan", "inf", "-inf"):
            result = self.audit(3, "0 1 1\n", "--tau", tau)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, "")
        result = self.audit(3, "0 1 1e308\n1 2 1e308\n")
        self.assertEqual(result.returncode, 2)
        self.assertIn("Accumulated graph weight", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_invalid_boundaries_emit_block_and_complete_partitions(self):
        for nodes, edges in [(0, ""), (2, "0 1 1\n"), (3, ""), (3, "0 1 1\n1 2 1\n")]:
            result = self.audit(nodes, edges, "--tau", "100", start=5, end=1)
            self.assertEqual(result.returncode, 0, result.stderr)
            audit = json.loads(result.stdout)
            self.assertEqual(audit["action"], "FATAL_BLOCK")
            self.assertFalse(audit["diagnostics"]["boundary_configuration_valid"])
            self.assertEqual(sorted(audit["mainland_nodes"] + audit["island_nodes"]), list(range(nodes)))

    def test_small_finite_scores_round_trip_without_becoming_zero(self):
        result = self.audit(4, "0 1 1e-200\n1 2 1e-200\n2 3 1e-200\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        audit = json.loads(result.stdout)
        self.assertTrue(audit["diagnostics"]["solver_converged"])
        self.assertAlmostEqual(audit["connectivity_score"] / 1e-200, 2 - math.sqrt(2), places=7)

    def test_evaluation_rejects_an_unconverged_cli_result(self):
        graph = argparse.Namespace(node_count=20, system_start=19, system_end=19,
                                   to_tsv=lambda: "".join(f"{i-1} {i} 1\n" for i in range(1, 20)))
        with self.assertRaisesRegex(RuntimeError, "did not converge"):
            evaluate.run_auditor(AUDITOR, graph, 2.0, 0.0, 0.1, (), max_iterations=1)
        audit = evaluate.run_auditor(AUDITOR, graph, 2.0, 0.0, 0.1, ())
        self.assertTrue(audit["diagnostics"]["solver_converged"])


if __name__ == "__main__":
    unittest.main()
