#!/usr/bin/env python3
"""Create a balanced detector split from the official Microsoft BIPIA files.

This is a detection adaptation, not BIPIA's response-level attack-success
protocol: every clean external context is paired with one poisoned context.
"""

from __future__ import annotations

import argparse
import json
import random
import subprocess
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bipia-root", required=True, type=Path)
    parser.add_argument("--task", required=True, choices=("email", "table"))
    parser.add_argument("--split", default="test", choices=("train", "test"))
    parser.add_argument("--limit", type=int)
    parser.add_argument("--seed", type=int, default=2023)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def insert_attack(context: str, attack: str, position: str) -> str:
    if position == "start":
        return f"{attack}\n\n{context}"
    if position == "middle":
        midpoint = len(context) // 2
        boundary = context.find("\n", midpoint)
        if boundary < 0:
            boundary = midpoint
        return f"{context[:boundary]}\n\n{attack}\n\n{context[boundary:]}"
    return f"{context}\n\n{attack}"


def user_message(context: str, question: str) -> str:
    return f"External content:\n{context}\n\nQuestion: {question}"


def main() -> None:
    args = parse_args()
    benchmark = args.bipia_root / "benchmark"
    context_path = benchmark / args.task / f"{args.split}.jsonl"
    attack_path = benchmark / f"text_attack_{args.split}.json"
    if not context_path.is_file() or not attack_path.is_file():
        raise FileNotFoundError("BIPIA benchmark context or attack file was not found")

    contexts = [
        json.loads(line)
        for line in context_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    attacks_by_category = json.loads(attack_path.read_text(encoding="utf-8"))
    attacks = [
        (category, attack_index, attack)
        for category, category_attacks in attacks_by_category.items()
        for attack_index, attack in enumerate(category_attacks)
    ]
    rng = random.Random(args.seed)
    rng.shuffle(contexts)
    rng.shuffle(attacks)
    if args.limit is not None:
        if args.limit <= 0:
            raise ValueError("--limit must be positive")
        contexts = contexts[: args.limit]

    try:
        source_revision = subprocess.check_output(
            ["git", "-C", str(args.bipia_root), "rev-parse", "HEAD"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except (FileNotFoundError, subprocess.CalledProcessError):
        source_revision = None

    rows = []
    positions = ("start", "middle", "end")
    for index, context_record in enumerate(contexts):
        context = context_record["context"]
        question = context_record["question"]
        category, attack_index, attack = attacks[index % len(attacks)]
        position = positions[index % len(positions)]
        common = {
            "task": args.task,
            "split": args.split,
            "source_revision": source_revision,
            "pair_index": index,
        }
        rows.append(
            {
                **common,
                "variant": "clean",
                "label": 0,
                "text": user_message(context, question),
            }
        )
        rows.append(
            {
                **common,
                "variant": "poisoned",
                "label": 1,
                "attack_category": category,
                "attack_index": attack_index,
                "attack_position": position,
                "text": user_message(insert_attack(context, attack, position), question),
            }
        )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        "".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8"
    )
    print(
        json.dumps(
            {
                "schema_version": 1,
                "source": "microsoft/BIPIA",
                "task": args.task,
                "split": args.split,
                "source_revision": source_revision,
                "pairs": len(contexts),
                "examples": len(rows),
                "seed": args.seed,
                "output": str(args.output),
            }
        )
    )


if __name__ == "__main__":
    main()
