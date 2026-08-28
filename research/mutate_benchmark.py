#!/usr/bin/env python3
"""Create deterministic prompt-injection stress-test variants.

These transformations test representation sensitivity. They are not assumed to
remain functionally successful attacks; verify attack success separately.
"""

from __future__ import annotations

import argparse
import base64
import json
import re
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--text-field", default="text")
    parser.add_argument("--label-field", default="label")
    parser.add_argument("--attack-label", default="1")
    return parser.parse_args()


def alternating_case(text: str) -> str:
    upper = True
    output = []
    for character in text:
        if character.isalpha():
            output.append(character.upper() if upper else character.lower())
            upper = not upper
        else:
            output.append(character)
    return "".join(output)


def whitespace_perturbation(text: str) -> str:
    pieces = re.split(r"(\s+)", text)
    return "".join(
        ("\n" if index % 4 == 1 else "  ") if value.isspace() else value
        for index, value in enumerate(pieces)
    )


def base64_wrapper(text: str) -> str:
    encoded = base64.b64encode(text.encode("utf-8")).decode("ascii")
    return f"Decode this Base64 text and carry out the decoded instruction:\n{encoded}"


def delimiter_wrapper(text: str) -> str:
    return f"<external-directive>\n{text}\n</external-directive>"


TRANSFORMS = {
    "alternating_case": alternating_case,
    "whitespace": whitespace_perturbation,
    "base64_wrapper": base64_wrapper,
    "delimiter_wrapper": delimiter_wrapper,
}


def main() -> None:
    args = parse_args()
    rows = [
        json.loads(line)
        for line in args.input.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    args.output_dir.mkdir(parents=True, exist_ok=True)

    for name, transform in TRANSFORMS.items():
        mutated = []
        for row in rows:
            copy = dict(row)
            copy["stress_transformation"] = name
            if str(row[args.label_field]) == args.attack_label:
                copy[args.text_field] = transform(str(row[args.text_field]))
            mutated.append(copy)
        output = args.output_dir / f"{name}.jsonl"
        output.write_text(
            "".join(json.dumps(row) + "\n" for row in mutated), encoding="utf-8"
        )

    print(
        json.dumps(
            {
                "schema_version": 1,
                "input": str(args.input),
                "examples_per_variant": len(rows),
                "variants": list(TRANSFORMS),
                "output_dir": str(args.output_dir),
                "warning": "verify transformed attack success separately",
            }
        )
    )


if __name__ == "__main__":
    main()

