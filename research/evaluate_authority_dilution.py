#!/usr/bin/env python3
"""Run the frozen pre-model authority dilution representation audit."""

from __future__ import annotations

import argparse
from collections import defaultdict
from hashlib import sha256
import json
import math
from pathlib import Path
import platform
import sys

from authority_dilution import SIGNALS, SPLITS, build_cases, observe_case
from calibrate import choose_threshold, metrics


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def evaluate_rows(fit_rows, check_rows, max_fpr):
    report = {"signals": {}}
    labels = [row["label"] for row in check_rows]
    for name in SIGNALS:
        threshold, fitted = choose_threshold(
            fit_rows, lambda row, signal=name: row["signals"][signal], max_fpr
        )
        checked = metrics(
            labels,
            [int(row["signals"][name] >= threshold) for row in check_rows],
        )
        report["signals"][name] = {
            "threshold": threshold if math.isfinite(threshold) else None,
            "fit": fitted,
            "check": checked,
        }
    return report


def collision_groups(rows):
    labels = defaultdict(set)
    cases = defaultdict(list)
    for row in rows:
        labels[row["graph_fingerprint"]].add(row["label"])
        cases[row["graph_fingerprint"]].append(row["case_id"])
    return [
        {"graph_fingerprint": fingerprint, "case_ids": sorted(cases[fingerprint])}
        for fingerprint, values in labels.items() if values == {0, 1}
    ]


def decision(full_report, collisions):
    candidate = full_report["signals"]["negative_algebraic_connectivity"]["check"]
    baselines = [full_report["signals"][name]["check"]
                 for name in SIGNALS if name != "negative_algebraic_connectivity"]
    reasons = []
    if collisions:
        reasons.append("authorized and unauthorized cases share graph fingerprints")
    if (candidate["true_positive_rate"] or 0.0) < 0.50:
        reasons.append("spectral check recall below 50%")
    if candidate["false_positive_rate"] is None or candidate["false_positive_rate"] > 0.01:
        reasons.append("spectral check false-positive rate above 1% or unavailable")
    if candidate["tp"] <= max((baseline["tp"] for baseline in baselines), default=0):
        reasons.append("spectral signal did not beat every deterministic baseline")
    return {"advance_to_model_acquisition": not reasons, "reasons": reasons}


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--auditor", type=Path,
                        default=Path("target/release/spectral-pruner-audit"))
    parser.add_argument("--max-fpr", type=float, default=0.01)
    args = parser.parse_args()
    if args.output_dir.exists():
        parser.error("--output-dir must not already exist")
    if not 0 <= args.max_fpr <= 1:
        parser.error("--max-fpr must be within [0, 1]")
    args.auditor = args.auditor.resolve()
    if not args.auditor.is_file():
        parser.error("build the release auditor before running the study")
    return args


def main():
    args = parse_args()
    args.output_dir.mkdir(parents=True)
    here = Path(__file__).parent
    manifest = {
        "schema_version": 1,
        "status": "running",
        "arguments": {
            "output_dir": str(args.output_dir),
            "auditor": str(args.auditor),
            "max_fpr": args.max_fpr,
        },
        "python": sys.version,
        "platform": platform.platform(),
        "auditor_sha256": sha256(args.auditor.read_bytes()).hexdigest(),
        "source_sha256": {
            name: sha256((here / name).read_bytes()).hexdigest()
            for name in ("AUTHORITY_DILUTION_STUDY.md", "authority_dilution.py",
                         "evaluate_authority_dilution.py", "calibrate.py")
        },
    }
    write_json(args.output_dir / "run.json", manifest)
    try:
        rows_by_split = {split: [] for split in SPLITS}
        observations = args.output_dir / "observations.jsonl"
        with observations.open("x", encoding="utf-8") as destination:
            for case in build_cases():
                row = observe_case(case, args.auditor)
                rows_by_split[case["split"]].append(row)
                destination.write(json.dumps(row, allow_nan=False) + "\n")
        naive = {
            split: [row for row in rows if
                    (row["semantic"], row["topology"]) in {
                        ("authorized", "distributed"),
                        ("unauthorized", "nested"),
                    }]
            for split, rows in rows_by_split.items()
        }
        full = evaluate_rows(
            rows_by_split["mechanism_fit"], rows_by_split["mechanism_check"],
            args.max_fpr,
        )
        naive_report = evaluate_rows(
            naive["mechanism_fit"], naive["mechanism_check"], args.max_fpr,
        )
        collisions = collision_groups(rows_by_split["mechanism_check"])
        summary = {
            "schema_version": 1,
            "fit_graphs": len(rows_by_split["mechanism_fit"]),
            "check_graphs": len(rows_by_split["mechanism_check"]),
            "check_cross_label_graph_collisions": collisions,
            "naive_subset": naive_report,
            "collision_aware": full,
        }
        write_json(args.output_dir / "summary.json", summary)
        write_json(args.output_dir / "decision.json", decision(full, collisions))
        manifest["observations_sha256"] = sha256(observations.read_bytes()).hexdigest()
        manifest["summary_sha256"] = sha256(
            (args.output_dir / "summary.json").read_bytes()
        ).hexdigest()
        manifest["status"] = "complete"
    except BaseException as error:
        manifest.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        write_json(args.output_dir / "run.json", manifest)
    print(f"Completed authority dilution audit: {args.output_dir / 'summary.json'}")


if __name__ == "__main__":
    main()
