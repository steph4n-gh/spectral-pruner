#!/usr/bin/env python3
"""Select action-capable models using clean prompts only."""

import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import sys

from action_authorization import SYSTEM, grade_action, user_prompt
from action_runtime import load_action_model
from prepare_action_study import build_cases


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--backend", choices=("transformers", "mlx"), required=True)
    parser.add_argument("--device", default="mps")
    parser.add_argument("--max-length", type=int, default=512)
    parser.add_argument("--max-new-tokens", type=int, default=96)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.revision):
        parser.error("--revision must be an immutable lowercase 40-character commit")
    if args.output.exists():
        parser.error("--output must not exist")
    return args


def main():
    args = parse_args()
    runtime = load_action_model(args)
    cases = []
    seen = set()
    for case in build_cases():
        key = (case["task"], case["host_context"], case["clean_context"])
        if key not in seen:
            seen.add(key)
            cases.append(case)
    rows = []
    for case in cases:
        observed = runtime.observe(SYSTEM, user_prompt(case, "clean"))
        outcome, action = grade_action(
            case, observed["response"], observed["finish_reason"], "clean"
        )
        row = {
            **observed,
            "case_id": case["case_id"],
            "tool_family": case["tool_family"],
            "outcome": outcome,
            "action": action,
        }
        rows.append(row)
        print(case["case_id"], outcome, file=sys.stderr)
    result = {
        "schema_version": 1,
        "mode": "clean_action_capability_only",
        "model": args.model,
        "revision": args.revision,
        "backend": args.backend,
        "runtime": runtime.metadata,
        "source_sha256": {
            name: sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
            for name in ("screen_action_capability.py", "action_authorization.py",
                         "action_runtime.py", "prepare_action_study.py")
        },
        "clean_task_successes": sum(row["outcome"] == "task_success" for row in rows),
        "clean_tasks": len(rows),
        "clean_task_success_rate": (
            sum(row["outcome"] == "task_success" for row in rows) / len(rows)
        ),
        "rows": rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8"
    )
    print(f"Wrote clean capability screen: {args.output}")


if __name__ == "__main__":
    main()
