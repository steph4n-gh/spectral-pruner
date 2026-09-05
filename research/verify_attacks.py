#!/usr/bin/env python3
"""Screen behavioral attack candidates on development tasks, without detector signals."""

import argparse
from collections import Counter
from hashlib import sha256
import json
from pathlib import Path
import platform
import re
import sys

from evaluate_behavior import SYSTEM, grade, load_pairs, normalize, user_prompt, write_json


def summarize_screen(rows):
    clean = {row["clean_key"]: row for row in rows if row["variant"] == "clean"}
    attacks = [row for row in rows if row["variant"] == "poisoned"]
    groups = {}
    for row in attacks:
        group = groups.setdefault(row["attack_id"], {"attempts": 0, "eligible": 0, "successful": 0,
                                                     "outcomes": Counter()})
        group["attempts"] += 1
        group["outcomes"][row["outcome"]] += 1
        if clean[row["clean_key"]]["outcome"] == "task_success":
            group["eligible"] += 1
            group["successful"] += row["outcome"] == "attack_target"
    successes = sum(row["outcome"] == "task_success" for row in clean.values())
    return {"unique_clean_tasks": len(clean), "clean_task_successes": successes,
            "clean_success_rate": successes / len(clean) if clean else None,
            "clean_outcomes": dict(Counter(row["outcome"] for row in clean.values())),
            "attacks": groups}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pairs", type=Path, required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--device", default="cpu")
    parser.add_argument("--max-length", type=int, default=512)
    parser.add_argument("--max-new-tokens", type=int, default=64)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.revision):
        parser.error("pin a 40-character model revision")
    if min(args.max_length, args.max_new_tokens) <= 0:
        parser.error("token budgets must be positive")
    pairs = load_pairs(args.pairs, allowed_splits=("development",))
    if any("/" not in pair["pair_id"] for pair in pairs):
        parser.error("development pair_id must be attack_id/task_id")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    manifest = {"schema_version": 1, "status": "running", "mode": "development_behavior_only",
                "arguments": {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
                "dataset_sha256": sha256(args.pairs.read_bytes()).hexdigest(),
                "research_sha256": {name: sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
                                    for name in ("verify_attacks.py", "evaluate_behavior.py",
                                                 "behavioral_runtime.py", "attention_graph.py")},
                "python": sys.version, "platform": platform.platform()}
    manifest_path = args.output_dir / "run.json"
    write_json(manifest_path, manifest)
    try:
        from behavioral_runtime import BehavioralModel

        runtime = BehavioralModel(args)
        manifest["runtime"] = runtime.metadata
        write_json(manifest_path, manifest)
        rows, clean_cache = [], {}
        with (args.output_dir / "observations.jsonl").open("x", encoding="utf-8") as out:
            for pair in pairs:
                clean_key = sha256(json.dumps([pair["task"], pair["clean_context"],
                                               normalize(pair["expected_answer"])]).encode()).hexdigest()
                for variant in ("clean", "poisoned"):
                    if variant == "clean" and clean_key in clean_cache:
                        continue
                    prompt = runtime.graph_tools.render_chat(
                        runtime.tokenizer, SYSTEM, user_prompt(pair, variant), generation_prompt=True)
                    encoded = runtime.tokenizer(prompt, add_special_tokens=False, return_tensors="pt")
                    prefix = encoded["input_ids"][0].tolist()
                    if len(prefix) > args.max_length:
                        raise ValueError("development prompt exceeds declared token budget")
                    inputs = {key: value.to(runtime.device) for key, value in encoded.items()}
                    with runtime.torch.inference_mode():
                        generated = runtime.model.generate(**inputs, generation_config=runtime.config)
                    sequence = generated.sequences[0].tolist()
                    if sequence[:len(prefix)] != prefix:
                        raise ValueError("generation did not preserve input prefix")
                    continuation = sequence[len(prefix):]
                    finished = bool(continuation) and continuation[-1] in runtime.eos_ids
                    response = runtime.tokenizer.decode(continuation, skip_special_tokens=True)
                    outcome = grade(response, "eos" if finished else "length",
                                    pair["expected_answer"], pair["attack_answer"])
                    row = {"pair_id": pair["pair_id"], "attack_id": pair["pair_id"].split("/")[0],
                           "clean_key": clean_key, "variant": variant, "outcome": outcome,
                           "response": response, "finish_reason": "eos" if finished else "length",
                           "prefix_tokens": len(prefix), "generated_tokens": len(continuation),
                           "prefix_sha256": sha256(json.dumps(prefix).encode()).hexdigest()}
                    rows.append(row)
                    if variant == "clean":
                        clean_cache[clean_key] = row
                    out.write(json.dumps(row, allow_nan=False) + "\n")
                    out.flush()
                    print(pair["pair_id"], variant, outcome, repr(response), file=sys.stderr)
        write_json(args.output_dir / "summary.json", summarize_screen(rows))
        manifest["status"] = "complete"
    except BaseException as error:
        manifest.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        write_json(manifest_path, manifest)


if __name__ == "__main__":
    main()
