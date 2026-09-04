#!/usr/bin/env python3
"""Evaluate attention-graph signals, policy ablations, and standard baselines."""

from __future__ import annotations

import argparse
import json
import math
import random
import subprocess
import sys
from collections import defaultdict
from hashlib import sha256
from pathlib import Path


ABLATIONS = {
    "full": (),
    "spectral_only": ("--spectral-only",),
    "without_connectivity": (),
    "without_density": ("--disable-density",),
    "without_neglect": ("--disable-neglect",),
    "without_tripwire": ("--disable-tripwire",),
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--jsonl", type=Path, help="local JSONL benchmark")
    source.add_argument("--hf-dataset", help="Hugging Face dataset name")
    parser.add_argument("--hf-config")
    parser.add_argument("--dataset-revision", help="exact Hugging Face dataset commit")
    parser.add_argument("--split", default="test")
    parser.add_argument("--text-field", default="text")
    parser.add_argument("--label-field", default="label")
    parser.add_argument("--attack-label", default="1")
    parser.add_argument("--max-per-class", type=int)
    parser.add_argument("--seed", type=int, default=17)

    parser.add_argument("--model", required=True)
    parser.add_argument("--revision", help="exact Hugging Face commit or local revision")
    parser.add_argument("--system", required=True)
    parser.add_argument("--device", default="auto")
    parser.add_argument("--layers", default="last:4")
    parser.add_argument("--top-k", type=int, default=8)
    parser.add_argument("--min-weight", type=float, default=0.0)
    parser.add_argument("--max-length", type=int, default=512)

    parser.add_argument(
        "--auditor",
        type=Path,
        default=Path("target/release/spectral-pruner-audit"),
    )
    parser.add_argument("--threat-threshold", type=float, default=2.0)
    parser.add_argument("--connectivity-threshold", type=float)
    parser.add_argument("--instruction-threshold", type=float, default=0.1)
    parser.add_argument("--max-iterations", type=int, default=10_000)
    parser.add_argument("--tolerance", type=float, default=1e-9)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument(
        "--resume",
        action="store_true",
        help="continue an interrupted run from predictions.jsonl",
    )
    return parser.parse_args()


def load_records(args: argparse.Namespace) -> tuple[list[dict], dict]:
    if args.jsonl:
        records = [
            json.loads(line)
            for line in args.jsonl.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        identity = {
            "kind": "jsonl",
            "path": str(args.jsonl),
            "sha256": sha256(args.jsonl.read_bytes()).hexdigest(),
        }
    else:
        from datasets import load_dataset

        dataset = load_dataset(
            args.hf_dataset,
            args.hf_config,
            split=args.split,
            revision=args.dataset_revision,
        )
        records = list(dataset)
        identity = {
            "kind": "hugging_face",
            "name": args.hf_dataset,
            "config": args.hf_config,
            "split": args.split,
            "revision": args.dataset_revision,
            "fingerprint": dataset._fingerprint,
        }

    normalized = []
    for index, record in enumerate(records):
        if args.text_field not in record or args.label_field not in record:
            raise ValueError(
                f"record {index} lacks {args.text_field!r} or {args.label_field!r}"
            )
        normalized.append(
            {
                "source_index": index,
                "text": str(record[args.text_field]),
                "label": int(str(record[args.label_field]) == args.attack_label),
            }
        )

    if args.max_per_class is not None:
        if args.max_per_class <= 0:
            raise ValueError("--max-per-class must be positive")
        groups: dict[int, list[dict]] = defaultdict(list)
        for record in normalized:
            groups[record["label"]].append(record)
        rng = random.Random(args.seed)
        normalized = []
        for label in sorted(groups):
            group = groups[label]
            rng.shuffle(group)
            normalized.extend(group[: args.max_per_class])
        normalized.sort(key=lambda row: row["source_index"])

    if {record["label"] for record in normalized} != {0, 1}:
        raise ValueError("evaluation requires at least one benign and one attack record")
    return normalized, identity


def run_auditor(
    auditor: Path,
    graph,
    threat_threshold: float,
    connectivity_threshold: float | None,
    instruction_threshold: float,
    extra_args: tuple[str, ...],
    max_iterations: int = 10_000,
    tolerance: float = 1e-9,
) -> dict:
    command = [
        str(auditor),
        "--nodes",
        str(graph.node_count),
        "--system-start",
        str(graph.system_start),
        "--system-end",
        str(graph.system_end),
        "--threat-threshold",
        str(threat_threshold),
        "--instruction-threshold",
        str(instruction_threshold),
        "--max-iterations",
        str(max_iterations),
        "--tolerance",
        str(tolerance),
        *extra_args,
        "-",
    ]
    if connectivity_threshold is not None:
        command[1:1] = ["--connectivity-threshold", str(connectivity_threshold)]
    completed = subprocess.run(
        command,
        input=graph.to_tsv(),
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"auditor failed: {completed.stderr.strip()}")
    audit = json.loads(completed.stdout)
    if not audit["diagnostics"].get("solver_converged", False):
        raise RuntimeError("auditor did not converge; increase --max-iterations before evaluating")
    score = audit["connectivity_score"]
    if score is None or not math.isfinite(score):
        raise RuntimeError("auditor returned a non-finite connectivity score")
    return audit


def run_identity(args, dataset_identity: dict, revision: str | None, device: str) -> dict:
    """Include every input that can change selected examples, labels, or signals."""
    return {
        "schema_version": 2,
        "dataset": dataset_identity,
        "model": args.model,
        "requested_revision": args.revision,
        "model_revision": revision,
        "device": device,
        "auditor_sha256": sha256(args.auditor.read_bytes()).hexdigest(),
        "research_sha256": {
            name: sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
            for name in ("evaluate.py", "attention_graph.py")
        },
        "system_sha256": sha256(args.system.encode("utf-8")).hexdigest(),
        **{name: getattr(args, name) for name in (
            "text_field", "label_field", "attack_label", "max_per_class", "seed",
            "max_length", "layers", "top_k", "min_weight", "threat_threshold",
            "connectivity_threshold", "instruction_threshold", "max_iterations", "tolerance",
        )},
    }


def resume_rows(manifest_path: Path, result_path: Path, identity: dict, records: list[dict]) -> list[dict]:
    if not manifest_path.is_file():
        raise ValueError("cannot safely resume: run.json is missing")
    if json.loads(manifest_path.read_text(encoding="utf-8")) != identity:
        raise ValueError("cannot safely resume: run configuration has changed")
    rows = [json.loads(line) for line in result_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    selected = {record["source_index"]: record for record in records}
    seen = set()
    for row in rows:
        index = row["source_index"]
        record = selected.get(index)
        if (record is None or index in seen or row["label"] != record["label"]
                or row["text_sha256"] != sha256(record["text"].encode("utf-8")).hexdigest()):
            raise ValueError("cannot safely resume: saved predictions do not match selected records")
        seen.add(index)
    return rows


def roc_auc(labels: list[int], scores: list[float]) -> float:
    """Mann-Whitney AUROC with average ranks for exact ties."""
    pairs = sorted(zip(scores, labels), key=lambda pair: pair[0])
    rank_sum_positive = 0.0
    cursor = 0
    while cursor < len(pairs):
        end = cursor + 1
        while end < len(pairs) and pairs[end][0] == pairs[cursor][0]:
            end += 1
        average_rank = ((cursor + 1) + end) / 2.0
        rank_sum_positive += average_rank * sum(label for _, label in pairs[cursor:end])
        cursor = end
    positives = sum(labels)
    negatives = len(labels) - positives
    return (rank_sum_positive - positives * (positives + 1) / 2.0) / (
        positives * negatives
    )


def policy_metrics(labels: list[int], predictions: list[int]) -> dict:
    tp = sum(label == 1 and prediction == 1 for label, prediction in zip(labels, predictions))
    tn = sum(label == 0 and prediction == 0 for label, prediction in zip(labels, predictions))
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


def trajectory_features(layer_audits: list[dict]) -> dict:
    values = [audit["connectivity_score"] for audit in layer_audits]
    mean = sum(values) / len(values)
    minimum = min(values)
    maximum = max(values)
    center = (len(values) - 1) / 2.0
    denominator = sum((index - center) ** 2 for index in range(len(values)))
    slope = (
        sum((index - center) * (value - mean) for index, value in enumerate(values))
        / denominator
        if denominator > 0.0
        else 0.0
    )
    return {
        "lambda2_late_mean": mean,
        "lambda2_min": minimum,
        "lambda2_max": maximum,
        "lambda2_range": maximum - minimum,
        "lambda2_slope": slope,
        "lambda2_first_to_last_drop": values[0] - values[-1],
    }


def main() -> None:
    # Pure evaluation/manifest helpers and their tests need no model runtime.
    from attention_graph import extract_attention_bundle, graph_metadata, load_model

    args = parse_args()
    records, dataset_identity = load_records(args)
    if not args.auditor.is_file():
        raise FileNotFoundError(
            f"auditor not found at {args.auditor}; run "
            "cargo build --release --bin spectral-pruner-audit"
        )

    tokenizer, model, device = load_model(args.model, args.device, args.revision)
    revision = getattr(model.config, "_commit_hash", None)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    result_path = args.output_dir / "predictions.jsonl"
    manifest_path = args.output_dir / "run.json"
    identity = run_identity(args, dataset_identity, revision, device)
    if args.resume and result_path.is_file():
        rows = resume_rows(manifest_path, result_path, identity, records)
    else:
        manifest_path.write_text(
            json.dumps(identity, indent=2) + "\n", encoding="utf-8"
        )
        result_path.write_text("", encoding="utf-8")
        rows = []
    completed_indices = {row["source_index"] for row in rows}
    pending_records = [
        record for record in records if record["source_index"] not in completed_indices
    ]

    for position, record in enumerate(pending_records, start=len(rows) + 1):
        bundle = extract_attention_bundle(
            tokenizer,
            model,
            device,
            args.system,
            record["text"],
            layers=args.layers,
            top_k=args.top_k,
            min_weight=args.min_weight,
            max_length=args.max_length,
        )
        graph = bundle.aggregate
        audits = {
            name: run_auditor(
                args.auditor,
                graph,
                args.threat_threshold,
                None if name == "without_connectivity" else args.connectivity_threshold,
                args.instruction_threshold,
                flags,
                args.max_iterations,
                args.tolerance,
            )
            for name, flags in ABLATIONS.items()
        }
        full = audits["full"]
        diagnostics = full["diagnostics"]
        layer_audits = [
            run_auditor(
                args.auditor,
                layer_graph,
                args.threat_threshold,
                None,
                args.instruction_threshold,
                ("--spectral-only",),
                args.max_iterations,
                args.tolerance,
            )
            for layer_graph in bundle.layers
        ]
        row = {
            "source_index": record["source_index"],
            "label": record["label"],
            "text_sha256": sha256(record["text"].encode("utf-8")).hexdigest(),
            "graph": graph_metadata(graph, args.model, revision),
            "solver": {key: diagnostics[key] for key in (
                "solver_converged", "solver_iterations", "relative_residual",
            )},
            "signals": {
                "algebraic_connectivity": full["connectivity_score"],
                "conductance": diagnostics["conductance"],
                "density_ratio": diagnostics["density_ratio"],
                "density_ratio_status": diagnostics["density_ratio_status"],
                "instruction_connection": diagnostics["instruction_connection"],
                "token_count": graph.node_count,
                **trajectory_features(layer_audits),
            },
            "layer_signals": [
                {
                    "layer": layer_graph.selected_layers[0],
                    "algebraic_connectivity": audit["connectivity_score"],
                    "conductance": audit["diagnostics"]["conductance"],
                    "density_ratio": audit["diagnostics"]["density_ratio"],
                    "instruction_connection": audit["diagnostics"][
                        "instruction_connection"
                    ],
                }
                for layer_graph, audit in zip(bundle.layers, layer_audits)
            ],
            "ablations": {
                name: {
                    "action": audit["action"],
                    "connectivity_triggered": audit["diagnostics"][
                        "connectivity_triggered"
                    ],
                    "density_triggered": audit["diagnostics"]["density_triggered"],
                    "instruction_neglect_triggered": audit["diagnostics"][
                        "instruction_neglect_triggered"
                    ],
                    "single_token_triggered": audit["diagnostics"][
                        "single_token_triggered"
                    ],
                }
                for name, audit in audits.items()
            },
        }
        rows.append(row)
        with result_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(row) + "\n")
        print(f"[{position}/{len(records)}] source_index={record['source_index']}", file=sys.stderr)

    rows.sort(key=lambda row: row["source_index"])

    labels = [row["label"] for row in rows]
    signals = {
        "negative_algebraic_connectivity": [
            -row["signals"]["algebraic_connectivity"] for row in rows
        ],
        "negative_conductance": [-row["signals"]["conductance"] for row in rows],
        "density_ratio": [
            1.0e12
            if row["signals"]["density_ratio_status"] == "infinite"
            else float(row["signals"]["density_ratio"] or 0.0)
            for row in rows
        ],
        "negative_instruction_connection": [
            -row["signals"]["instruction_connection"] for row in rows
        ],
        "token_count": [row["signals"]["token_count"] for row in rows],
        "negative_layerwise_lambda2_mean": [
            -row["signals"]["lambda2_late_mean"] for row in rows
        ],
        "negative_layerwise_lambda2_min": [
            -row["signals"]["lambda2_min"] for row in rows
        ],
        "layerwise_lambda2_range": [
            row["signals"]["lambda2_range"] for row in rows
        ],
        "layerwise_lambda2_drop": [
            row["signals"]["lambda2_first_to_last_drop"] for row in rows
        ],
    }
    summary = {
        "schema_version": 1,
        "dataset": dataset_identity,
        "examples": len(rows),
        "class_counts": {
            "benign": labels.count(0),
            "attack": labels.count(1),
        },
        "model": {
            "name": args.model,
            "revision": revision,
            "device": device,
            "layers": args.layers,
            "top_k": args.top_k,
            "min_weight": args.min_weight,
            "connectivity_threshold": args.connectivity_threshold,
            "density_ratio_threshold": args.threat_threshold,
            "instruction_connection_threshold": args.instruction_threshold,
        },
        "signal_auroc": {
            name: roc_auc(labels, values) for name, values in signals.items()
        },
        "policy_ablations": {
            name: policy_metrics(
                labels,
                [int(row["ablations"][name]["action"] == "FATAL_BLOCK") for row in rows],
            )
            for name in ABLATIONS
        },
    }
    (args.output_dir / "summary.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
