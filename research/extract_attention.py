#!/usr/bin/env python3
"""Extract a real transformer attention graph as spectral-pruner TSV input."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from attention_graph import extract_attention_bundle, graph_metadata, load_model


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, help="Hugging Face causal LM name or path")
    parser.add_argument("--revision", help="exact Hugging Face commit or local revision")
    parser.add_argument("--system", required=True, help="System instruction text")
    parser.add_argument("--user", required=True, help="User message text")
    parser.add_argument("--output", required=True, type=Path, help="Output TSV path")
    parser.add_argument("--device", default="auto", help="auto, cpu, cuda, or mps")
    parser.add_argument("--layers", default="last:4", help="all, last:N, or comma list")
    parser.add_argument("--top-k", type=int, default=8, help="neighbors retained per token")
    parser.add_argument("--min-weight", type=float, default=0.0)
    parser.add_argument("--max-length", type=int, default=512)
    parser.add_argument(
        "--emit-layers",
        action="store_true",
        help="also write one TSV per selected layer",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    tokenizer, model, device = load_model(args.model, args.device, args.revision)
    revision = getattr(model.config, "_commit_hash", None)
    bundle = extract_attention_bundle(
        tokenizer,
        model,
        device,
        args.system,
        args.user,
        layers=args.layers,
        top_k=args.top_k,
        min_weight=args.min_weight,
        max_length=args.max_length,
    )
    graph = bundle.aggregate

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(graph.to_tsv(), encoding="utf-8")
    metadata_path = args.output.with_suffix(args.output.suffix + ".json")
    metadata_path.write_text(
        json.dumps(graph_metadata(graph, args.model, revision), indent=2) + "\n",
        encoding="utf-8",
    )
    layer_paths = []
    if args.emit_layers:
        for layer_graph in bundle.layers:
            layer_index = layer_graph.selected_layers[0]
            layer_path = args.output.with_name(
                f"{args.output.stem}.layer-{layer_index}{args.output.suffix}"
            )
            layer_path.write_text(layer_graph.to_tsv(), encoding="utf-8")
            layer_paths.append(str(layer_path))
    print(
        json.dumps(
            {
                "edges": str(args.output),
                "metadata": str(metadata_path),
                "device": device,
                "layer_edges": layer_paths,
                **graph_metadata(graph, args.model, revision),
            }
        )
    )


if __name__ == "__main__":
    main()
