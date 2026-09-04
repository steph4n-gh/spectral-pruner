#!/usr/bin/env python3
"""Fit signal thresholds on one split and report untouched-split performance."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path


SIGNALS = {
    "negative_algebraic_connectivity": lambda row: -row["signals"][
        "algebraic_connectivity"
    ],
    "negative_conductance": lambda row: -row["signals"]["conductance"],
    "density_ratio": lambda row: (
        1.0e12
        if row["signals"]["density_ratio_status"] == "infinite"
        else float(row["signals"]["density_ratio"] or 0.0)
    ),
    "negative_instruction_connection": lambda row: -row["signals"][
        "instruction_connection"
    ],
    "token_count": lambda row: row["signals"].get(
        "token_count", row["graph"]["node_count"]
    ),
    "negative_layerwise_lambda2_mean": lambda row: -row["signals"][
        "lambda2_late_mean"
    ],
    "negative_layerwise_lambda2_min": lambda row: -row["signals"]["lambda2_min"],
    "layerwise_lambda2_range": lambda row: row["signals"]["lambda2_range"],
    "layerwise_lambda2_drop": lambda row: row["signals"][
        "lambda2_first_to_last_drop"
    ],
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--calibration", required=True, type=Path)
    parser.add_argument("--evaluation", required=True, type=Path)
    parser.add_argument("--max-calibration-fpr", type=float, default=0.05)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def read_rows(path: Path) -> list[dict]:
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def metrics(labels: list[int], predictions: list[int]) -> dict:
    tp = sum(label == prediction == 1 for label, prediction in zip(labels, predictions))
    tn = sum(label == prediction == 0 for label, prediction in zip(labels, predictions))
    fp = sum(label == 0 and prediction == 1 for label, prediction in zip(labels, predictions))
    fn = sum(label == 1 and prediction == 0 for label, prediction in zip(labels, predictions))
    return {
        "tp": tp,
        "tn": tn,
        "fp": fp,
        "fn": fn,
        "true_positive_rate": tp / (tp + fn) if tp + fn else None,
        "false_positive_rate": fp / (fp + tn) if fp + tn else None,
        "accuracy": (tp + tn) / len(labels),
    }


def choose_threshold(rows: list[dict], signal, max_fpr: float):
    labels = [row["label"] for row in rows]
    scores = [signal(row) for row in rows]
    candidates = [math.inf, *sorted(set(scores), reverse=True)]
    feasible = []
    for threshold in candidates:
        result = metrics(labels, [int(score >= threshold) for score in scores])
        if result["false_positive_rate"] <= max_fpr:
            feasible.append((result["true_positive_rate"], result["accuracy"], threshold, result))
    if not feasible:
        raise ValueError("no feasible calibration threshold")
    _, _, threshold, result = max(feasible, key=lambda item: (item[0], item[1], item[2]))
    return threshold, result


def fit_benign_length_baseline(rows: list[dict]) -> tuple[float, float]:
    benign = [row for row in rows if row["label"] == 0]
    x = [row["graph"]["node_count"] for row in benign]
    y = [-row["signals"]["algebraic_connectivity"] for row in benign]
    mean_x = sum(x) / len(x)
    mean_y = sum(y) / len(y)
    denominator = sum((value - mean_x) ** 2 for value in x)
    slope = (
        sum((x_value - mean_x) * (y_value - mean_y) for x_value, y_value in zip(x, y))
        / denominator
        if denominator > 0.0
        else 0.0
    )
    return mean_y - slope * mean_x, slope


def main() -> None:
    args = parse_args()
    if not 0.0 <= args.max_calibration_fpr <= 1.0:
        raise ValueError("--max-calibration-fpr must be within [0, 1]")
    calibration = read_rows(args.calibration)
    evaluation = read_rows(args.evaluation)
    if not calibration or not evaluation:
        raise ValueError("both prediction files must be non-empty")

    report = {
        "schema_version": 1,
        "selection_rule": "maximize calibration TPR subject to the requested FPR ceiling",
        "max_calibration_fpr": args.max_calibration_fpr,
        "calibration_examples": len(calibration),
        "evaluation_examples": len(evaluation),
        "signals": {},
    }
    evaluation_labels = [row["label"] for row in evaluation]
    for name, signal in SIGNALS.items():
        threshold, calibration_metrics = choose_threshold(
            calibration, signal, args.max_calibration_fpr
        )
        evaluation_scores = [signal(row) for row in evaluation]
        report["signals"][name] = {
            "threshold": threshold,
            "calibration": calibration_metrics,
            "evaluation": metrics(
                evaluation_labels,
                [int(score >= threshold) for score in evaluation_scores],
            ),
        }

    intercept, slope = fit_benign_length_baseline(calibration)

    def residualized_connectivity(row: dict) -> float:
        expected = intercept + slope * row["graph"]["node_count"]
        return -row["signals"]["algebraic_connectivity"] - expected

    threshold, calibration_metrics = choose_threshold(
        calibration, residualized_connectivity, args.max_calibration_fpr
    )
    report["signals"]["length_residualized_negative_algebraic_connectivity"] = {
        "benign_length_fit": {"intercept": intercept, "slope": slope},
        "threshold": threshold,
        "calibration": calibration_metrics,
        "evaluation": metrics(
            evaluation_labels,
            [
                int(residualized_connectivity(row) >= threshold)
                for row in evaluation
            ],
        ),
    }

    rendered = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")


if __name__ == "__main__":
    main()
