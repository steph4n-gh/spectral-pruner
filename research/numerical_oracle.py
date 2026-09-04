#!/usr/bin/env python3
"""Compare the Rust weighted Fiedler estimate with NumPy's dense eigensolver."""

from __future__ import annotations

import argparse
import json
import math
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


def rust_audit(auditor: Path, node_count: int, edges, max_iterations: int = 10_000) -> dict:
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
            "--max-iterations",
            str(max_iterations),
            "-",
        ],
        input=edge_tsv,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.strip())
    return json.loads(completed.stdout)


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
        audit = rust_audit(args.auditor, node_count, edges)
        observed = audit["connectivity_score"]
        relative_error = abs(observed - expected) / max(abs(expected), 1.0e-15)
        trials.append(
            {
                "trial": trial,
                "nodes": node_count,
                "edges": len(edges),
                "numpy_lambda_2": expected,
                "rust_lambda_2": observed,
                "relative_error": relative_error,
                "solver_converged": audit["diagnostics"]["solver_converged"],
            }
        )

    # Fixed hard cases supplement the small random connected graphs.
    hard_cases = []
    path_edges = lambda n, w: [(i - 1, i, w) for i in range(1, n)]
    cases = [
        (f"scaled_path_{weight:g}", 4, path_edges(4, weight), 10_000, True,
         4 * math.sin(math.pi / 8) ** 2 * weight)
        for weight in (1e-200, 1e-12, 1.0, 1e150)
    ]
    cases += [
        ("long_path_default_budget", 1000, path_edges(1000, 1.0), 10_000, False,
         4 * math.sin(math.pi / 2000) ** 2),
        ("long_path_extended_budget", 1000, path_edges(1000, 1.0), 500_000, True,
         4 * math.sin(math.pi / 2000) ** 2),
        ("disconnected", 6, [(0, 1, 1.0), (1, 2, 1.0), (3, 4, 1.0)], 10_000, True, 0.0),
        ("isolated", 4, [], 10_000, True, 0.0),
    ]
    for bridge in (1e-3, 1e-8):
        edges = [(0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0),
                 (3, 4, 1.0), (4, 5, 1.0), (3, 5, 1.0), (2, 3, bridge)]
        cases.append((f"weak_bridge_{bridge:g}", 6, edges, 10_000, True, numpy_lambda_2(6, edges)))
    for name, nodes, edges, budget, expected_converged, expected in cases:
        audit = rust_audit(args.auditor, nodes, edges, budget)
        observed = audit["connectivity_score"]
        diagnostics = audit["diagnostics"]
        finite = observed is not None and math.isfinite(observed)
        error = abs(observed - expected) / (abs(expected) if expected else 1.0) if finite else math.inf
        passed = finite and diagnostics["solver_converged"] == expected_converged
        if expected_converged:
            passed = passed and error <= args.relative_tolerance
        else:
            passed = passed and diagnostics["solver_iterations"] == budget
        hard_cases.append({"name": name, "expected": expected, "observed": observed,
                           "error": error, "solver_converged": diagnostics["solver_converged"],
                           "solver_iterations": diagnostics["solver_iterations"],
                           "relative_residual": diagnostics["relative_residual"], "passed": passed})

    maximum = max(trial["relative_error"] for trial in trials)
    report = {
        "schema_version": 1,
        "seed": args.seed,
        "trials": args.trials,
        "relative_tolerance": args.relative_tolerance,
        "maximum_relative_error": maximum,
        "passed": maximum <= args.relative_tolerance
        and all(trial["solver_converged"] for trial in trials)
        and all(case["passed"] for case in hard_cases),
        "hard_cases": hard_cases,
        "results": trials,
    }
    print(json.dumps(report, indent=2))
    if not report["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

