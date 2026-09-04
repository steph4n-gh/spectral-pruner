#!/usr/bin/env python3
"""Compare the Rust weighted Fiedler estimate with NumPy's dense eigensolver."""

from __future__ import annotations

import argparse
import json
import random
import subprocess
from pathlib import Path

import numpy as np


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--auditor",
        type=Path,
        default=Path("target/release/spectral-pruner-audit"),
    )
    parser.add_argument("--seed", type=int, default=7021)
    parser.add_argument("--trials", type=int, default=24)
    parser.add_argument("--max-nodes", type=int, default=18)
    parser.add_argument("--relative-tolerance", type=float, default=1.0e-5)
    return parser.parse_args()


def random_connected_graph(rng: random.Random, node_count: int):
    edges: dict[tuple[int, int], float] = {}
    order = list(range(node_count))
    rng.shuffle(order)
    for index in range(1, node_count):
        u = order[index]
        v = order[rng.randrange(index)]
        edges[tuple(sorted((u, v)))] = rng.uniform(0.1, 2.0)
    for u in range(node_count):
        for v in range(u + 1, node_count):
            if (u, v) not in edges and rng.random() < 0.25:
                edges[(u, v)] = rng.uniform(0.1, 2.0)
    return sorted((u, v, weight) for (u, v), weight in edges.items())


def numpy_lambda_2(node_count: int, edges) -> float:
    adjacency = np.zeros((node_count, node_count), dtype=np.float64)
    for u, v, weight in edges:
        adjacency[u, v] += weight
        adjacency[v, u] += weight
    laplacian = np.diag(adjacency.sum(axis=1)) - adjacency
    return float(np.linalg.eigvalsh(laplacian)[1])


def rust_lambda_2(auditor: Path, node_count: int, edges) -> float:
    edge_tsv = "".join(f"{u}\t{v}\t{weight:.17g}\n" for u, v, weight in edges)
    completed = subprocess.run(
        [
            str(auditor),
            "--nodes",
            str(node_count),
            "--system-start",
            "0",
            "--system-end",
            "0",
            "--spectral-only",
            "-",
        ],
        input=edge_tsv,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.strip())
    return float(json.loads(completed.stdout)["connectivity_score"])


def main() -> None:
    args = parse_args()
    if args.trials <= 0 or args.max_nodes < 3:
        raise ValueError("trials must be positive and max-nodes must be at least 3")
    if not args.auditor.is_file():
        raise FileNotFoundError(
            f"auditor not found at {args.auditor}; run "
            "cargo build --release --bin spectral-pruner-audit"
        )

    rng = random.Random(args.seed)
    trials = []
    for trial in range(args.trials):
        node_count = rng.randint(3, args.max_nodes)
        edges = random_connected_graph(rng, node_count)
        expected = numpy_lambda_2(node_count, edges)
        observed = rust_lambda_2(args.auditor, node_count, edges)
        relative_error = abs(observed - expected) / max(abs(expected), 1.0e-15)
        trials.append(
            {
                "trial": trial,
                "nodes": node_count,
                "edges": len(edges),
                "numpy_lambda_2": expected,
                "rust_lambda_2": observed,
                "relative_error": relative_error,
            }
        )

    maximum = max(trial["relative_error"] for trial in trials)
    report = {
        "schema_version": 1,
        "seed": args.seed,
        "trials": args.trials,
        "relative_tolerance": args.relative_tolerance,
        "maximum_relative_error": maximum,
        "passed": maximum <= args.relative_tolerance,
        "results": trials,
    }
    print(json.dumps(report, indent=2))
    if not report["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

