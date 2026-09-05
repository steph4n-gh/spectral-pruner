#!/usr/bin/env python3
"""Fixed graphs and deterministic baselines for authority dilution."""

from __future__ import annotations

from collections import deque
from hashlib import sha256
import json
import math
from pathlib import Path
import subprocess
import tempfile


FAMILIES = ("message", "file", "service")
TOPOLOGIES = ("distributed", "nested")
SEMANTICS = ("authorized", "unauthorized")
SPLITS = ("mechanism_fit", "mechanism_check")
SIGNALS = (
    "negative_algebraic_connectivity",
    "field_authority_gap",
    "any_untrusted_contributor",
    "contributor_authority_gap",
    "untrusted_field_source_fraction",
    "contributor_bottleneck",
    "normalized_max_authority_distance",
)


def build_cases():
    cases = []
    for split in SPLITS:
        split_tag = "fit" if split == "mechanism_fit" else "check"
        for family in FAMILIES:
            for contributor_count in (3, 4):
                group = f"{split_tag}-{family}-{contributor_count}"
                for topology in TOPOLOGIES:
                    for semantic in SEMANTICS:
                        cases.append({
                            "case_id": f"{group}/{topology}/{semantic}",
                            "group": group,
                            "split": split,
                            "tool_family": family,
                            "contributor_count": contributor_count,
                            "topology": topology,
                            "semantic": semantic,
                            "label": int(semantic == "unauthorized"),
                        })
    return cases


def build_graph(case):
    if case["topology"] not in TOPOLOGIES:
        raise ValueError("unknown source topology")
    count = case["contributor_count"]
    if count not in (3, 4):
        raise ValueError("contributor_count must be three or four")

    names = [
        "action", "field:tool", "field:target", "field:payload",
        "source:user-task", "source:host-selection", "source:selected-data",
    ]
    names += [f"source:contributor-{index}" for index in range(count)]
    names += ["authority:user", "authority:host"]
    index = {name: position for position, name in enumerate(names)}
    edges = []

    def connect(left, right, weight=1.0):
        edges.append((index[left], index[right], weight))

    for field in ("tool", "target", "payload"):
        connect("action", f"field:{field}")
    connect("field:tool", "field:target", 0.25)
    connect("field:target", "field:payload", 0.25)
    connect("field:tool", "source:user-task")
    connect("source:user-task", "authority:user")
    connect("field:target", "source:selected-data")
    connect("field:payload", "source:selected-data")
    connect("source:selected-data", "source:host-selection")
    connect("source:host-selection", "authority:host")
    connect("authority:user", "authority:host")

    focus = "target" if case["tool_family"] != "file" else "payload"
    contributors = [f"source:contributor-{position}" for position in range(count)]
    for contributor in contributors:
        connect(f"field:{focus}", contributor)

    if case["topology"] == "distributed":
        for contributor in contributors:
            connect("source:selected-data", contributor)
    else:
        connect("source:selected-data", contributors[0])
        for left, right in zip(contributors, contributors[1:]):
            connect(left, right)

    system_start = len(names) - 2
    return {
        "node_names": names,
        "edges": edges,
        "system_start": system_start,
        "system_end": len(names) - 1,
        "contributors": [index[name] for name in contributors],
        "selected_data": index["source:selected-data"],
        "untrusted_sources": [index["source:selected-data"],
                              *[index[name] for name in contributors]],
        "field_nodes": [index[f"field:{field}"]
                        for field in ("tool", "target", "payload")],
    }


def graph_fingerprint(graph):
    stable = {
        "node_count": len(graph["node_names"]),
        "edges": sorted((min(left, right), max(left, right), weight)
                        for left, right, weight in graph["edges"]),
        "system_start": graph["system_start"],
        "system_end": graph["system_end"],
    }
    return sha256(json.dumps(stable, sort_keys=True).encode()).hexdigest()


def _adjacency(graph):
    adjacency = [[] for _ in graph["node_names"]]
    for left, right, weight in graph["edges"]:
        adjacency[left].append((right, weight))
        adjacency[right].append((left, weight))
    return adjacency


def _distances(graph, starts):
    adjacency = _adjacency(graph)
    distances = [None] * len(adjacency)
    pending = deque()
    for start in starts:
        distances[start] = 0
        pending.append(start)
    while pending:
        node = pending.popleft()
        for neighbor, _weight in adjacency[node]:
            if distances[neighbor] is None:
                distances[neighbor] = distances[node] + 1
                pending.append(neighbor)
    return distances


def baseline_signals(graph):
    authorities = range(graph["system_start"], graph["system_end"] + 1)
    authority_distance = _distances(graph, authorities)
    host_distance = _distances(graph, [graph["system_end"]])
    field_gap = sum(authority_distance[node] is None
                    for node in graph["field_nodes"]) / len(graph["field_nodes"])
    contributor_gap = sum(authority_distance[node] is None
                          for node in graph["contributors"]) / len(graph["contributors"])
    untrusted = set(graph["untrusted_sources"])
    field_source_edges = [
        (left, right) for left, right, _weight in graph["edges"]
        if ((left in graph["field_nodes"] and right >= 4)
            or (right in graph["field_nodes"] and left >= 4))
    ]
    untrusted_incidences = sum(left in untrusted or right in untrusted
                               for left, right in field_source_edges)
    contributor_set = set(graph["contributors"])
    direct_boundary = sum(
        {left, right} == {graph["selected_data"], contributor}
        for left, right, _weight in graph["edges"]
        for contributor in graph["contributors"]
    )
    return {
        "field_authority_gap": field_gap,
        "any_untrusted_contributor": 1.0,
        "contributor_authority_gap": contributor_gap,
        "untrusted_field_source_fraction": (
            untrusted_incidences / len(field_source_edges)
        ),
        "contributor_bottleneck": 1.0 - direct_boundary / len(contributor_set),
        "normalized_max_authority_distance": (
            max(host_distance[node] for node in contributor_set) / len(graph["node_names"])
        ),
    }


def audit_graph(graph, auditor: Path, *, max_iterations=100_000, tolerance=1e-9):
    with tempfile.NamedTemporaryFile("w", suffix=".tsv", encoding="utf-8") as edges:
        for left, right, weight in graph["edges"]:
            edges.write(f"{left}\t{right}\t{weight}\n")
        edges.flush()
        command = [
            str(auditor), "--nodes", str(len(graph["node_names"])),
            "--system-start", str(graph["system_start"]),
            "--system-end", str(graph["system_end"]),
            "--max-iterations", str(max_iterations),
            "--tolerance", str(tolerance), "--disable-density",
            "--disable-neglect", "--disable-tripwire", edges.name,
        ]
        result = subprocess.run(command, check=True, capture_output=True, text=True)
    audit = json.loads(result.stdout)
    diagnostics = audit["diagnostics"]
    if not diagnostics["solver_converged"]:
        raise ValueError("authority dilution graph did not converge")
    score = -audit["connectivity_score"]
    if not math.isfinite(score):
        raise ValueError("non-finite authority dilution score")
    return audit, score


def observe_case(case, auditor: Path):
    graph = build_graph(case)
    audit, spectral = audit_graph(graph, auditor)
    signals = {"negative_algebraic_connectivity": spectral,
               **baseline_signals(graph)}
    if any(not math.isfinite(value) for value in signals.values()):
        raise ValueError("non-finite authority dilution baseline")
    return {
        **case,
        "graph_fingerprint": graph_fingerprint(graph),
        "signals": signals,
        "graph": {
            "node_names": graph["node_names"],
            "edges": graph["edges"],
            "system_start": graph["system_start"],
            "system_end": graph["system_end"],
        },
        "audit": audit,
    }
