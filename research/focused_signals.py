"""Renderer-owned trust spans and the two prespecified focused signals.

Pure span/selection functions are testable without model dependencies. Model
imports remain in the observation path, outside the dependency-free Rust crate.
"""

import math
import time

from evaluate_behavior import user_prompt


def owned_spans(render, system, task, document):
    """Locate inserted content structurally, never by searching attacker text."""
    system_marker, user_marker = "SPECTRAL_SYSTEM_PLACEHOLDER", "SPECTRAL_USER_PLACEHOLDER"
    skeleton = render(system_marker, user_marker)
    if skeleton.count(system_marker) != 1 or skeleton.count(user_marker) != 1:
        raise ValueError("chat template must preserve each content placeholder once")
    before, rest = skeleton.split(system_marker)
    if user_marker not in rest:
        raise ValueError("chat template must put system before user")
    middle, after = rest.split(user_marker)
    user = user_prompt({"task": task, "clean_context": document}, "clean")
    prompt = before + system + middle + user + after
    if prompt != render(system, user):
        raise ValueError("chat template changed content; cannot prove owned spans")
    system_start = len(before)
    task_start = system_start + len(system) + len(middle) + len("Task: ")
    document_start = task_start + len(task) + len("\n\n<external_document>\n")
    spans = {"system": (system_start, system_start + len(system)),
             "task": (task_start, task_start + len(task)),
             "document": (document_start, document_start + len(document))}
    for name, text in (("system", system), ("task", task), ("document", document)):
        start, end = spans[name]
        if not text or prompt[start:end] != text:
            raise ValueError("content span is empty or does not reproduce its source")
    return prompt, spans


def token_regions(offsets, spans, prompt_length):
    """Untrusted overlap wins; only wholly owned tokens enter trusted masks."""
    regions = {name: [] for name in spans}
    doc_start, doc_end = spans["document"]
    for index, (start, end) in enumerate(offsets):
        if not 0 <= start <= end <= prompt_length:
            raise ValueError("invalid tokenizer offset")
        if start == end:
            continue  # Formatting/special tokens still remain in the graph.
        if end > doc_start and start < doc_end:
            regions["document"].append(index)
        else:
            for name in ("system", "task"):
                left, right = spans[name]
                if left <= start < end <= right:
                    regions[name].append(index)
    if any(not indices for indices in regions.values()):
        raise ValueError("tokenizer did not map every nonempty content region")
    return regions


def select_heads(observations, count=4):
    """Rank by worst clean task attention; no outcome or attack score is used."""
    if not observations or count <= 0:
        raise ValueError("head selection needs clean observations and a positive count")
    layout = [(layer["layer"], head) for layer in observations[0]
              for head in range(len(layer["mass"]))]
    if len(layout) < count or len(set(layout)) != len(layout):
        raise ValueError("invalid or insufficient head layout")
    ranks = {key: [] for key in layout}
    for observation in observations:
        current = {(layer["layer"], head): mass for layer in observation
                   for head, mass in enumerate(layer["mass"])}
        if list(current) != layout:
            raise ValueError("head layout changed between observations")
        for key, mass in current.items():
            if len(mass) != 3 or any(not math.isfinite(v) or not 0 <= v <= 1.001 for v in mass):
                raise ValueError("invalid per-head attention mass")
            ranks[key].append(mass[1])  # [system, task, document]
    ordered = sorted(layout, key=lambda key: (-min(ranks[key]), key))
    return [{"layer": layer, "head": head, "minimum_clean_task_mass": min(ranks[layer, head])}
            for layer, head in ordered[:count]]


def inspect_prompt(runtime, system, task, document):
    render = lambda s, u: runtime.graph_tools.render_chat(
        runtime.tokenizer, s, u, generation_prompt=True)
    started = time.perf_counter()
    prompt, spans = owned_spans(render, system, task, document)
    encoded = runtime.tokenizer(prompt, add_special_tokens=False, return_offsets_mapping=True)
    if len(encoded["offset_mapping"]) != len(encoded["input_ids"]):
        raise ValueError("tokenizer must provide one offset per input token")
    regions = token_regions(encoded["offset_mapping"], spans, len(prompt))
    mapping_seconds = time.perf_counter() - started
    captured = runtime.capture(system, user_prompt({"task": task, "clean_context": document}, "clean"))
    if captured["prompt"] != prompt or captured["prefix"] != encoded["input_ids"]:
        raise ValueError("span mapping and generation must use the exact same input IDs")
    snapshot = captured["snapshot"]
    layers = runtime.graph_tools.parse_layer_selection(runtime.args.layers, len(snapshot))
    runtime.sync()
    started = time.perf_counter()
    masses = []
    with runtime.torch.inference_mode():
        for layer in layers:
            values = [snapshot[layer][0, :, -1, regions[name]].float().sum(dim=-1)
                      for name in ("system", "task", "document")]
            mass = runtime.torch.stack(values, dim=-1).cpu().tolist()
            if any(not math.isfinite(v) or not 0 <= v <= 1.001 for row in mass for v in row):
                raise ValueError("invalid per-head attention mass")
            masses.append({"layer": layer, "mass": mass})
    runtime.sync()
    captured["observation"].update(
        regions={"characters": spans, "tokens": regions},
        head_mass_columns=["system", "task", "document"], head_masses=masses,
    )
    captured["observation"]["seconds"].update(
        span_mapping=mapping_seconds, head_measurement=time.perf_counter() - started)
    return captured


def focused_observation(runtime, system, task, document, heads):
    from evaluate import run_auditor

    started = time.perf_counter()
    captured = inspect_prompt(runtime, system, task, document)
    args, torch, tools = runtime.args, runtime.torch, runtime.graph_tools
    snapshot, observation = captured["snapshot"], captured["observation"]
    masses = {item["layer"]: item["mass"] for item in observation["head_masses"]}
    selected = [(item["layer"], item["head"]) for item in heads]
    if len(selected) != 4 or len(set(selected)) != 4:
        raise ValueError("the fixed protocol requires four distinct heads")
    if any(layer not in masses or not 0 <= head < len(masses[layer]) for layer, head in selected):
        raise ValueError("selected head is outside the observed layout")
    focus_mass = sum(masses[layer][head][0] + masses[layer][head][1]
                     for layer, head in selected) / len(selected)
    # Keep the old baseline's inclusive system interval, for an exact comparison.
    system_mass = sum(float(snapshot[layer][0, :, -1,
        captured["system_start"]:captured["system_end"] + 1].float().sum(dim=-1).mean())
        for layer in masses) / len(masses)
    graph_start = time.perf_counter()
    with torch.inference_mode():
        aggregate = torch.stack([snapshot[layer][0].float().mean(dim=0) for layer in masses]).mean(dim=0)
        focused = torch.stack([snapshot[layer][0, head].float() for layer, head in selected]).mean(dim=0)
        graphs = {name: tools.graph_from_directed(
            runtime.tokenizer, captured["prompt"], captured["prefix"],
            captured["system_start"], captured["system_end"], matrix,
            layer_ids=tuple(sorted(masses if name == "aggregate" else {v[0] for v in selected})),
            top_k=args.top_k, min_weight=args.min_weight,
        ) for name, matrix in (("aggregate", aggregate), ("focused", focused))}
    runtime.sync()
    observation["seconds"]["graph_conversion"] = time.perf_counter() - graph_start
    audits = {}
    for name, graph in graphs.items():
        before = time.perf_counter()
        audits[name] = run_auditor(args.auditor, graph, 2.0, None, 0.1, ("--spectral-only",),
                                  max_iterations=args.max_iterations, tolerance=args.tolerance)
        observation["seconds"][name + "_auditor"] = time.perf_counter() - before
    metadata = {name: tools.graph_metadata(graph, args.model, args.revision) for name, graph in graphs.items()}
    for value in metadata.values():
        value.pop("tokens")
    metadata["focused"]["aggregation"]["heads"] = "mean of the four frozen layer/head pairs"
    metadata["focused"]["aggregation"]["layers"] = "equal weight per selected head"
    metadata["focused"]["selected_heads"] = heads
    observation.update(
        graph=metadata["aggregate"], focused_graph=metadata["focused"],
        signals={"negative_algebraic_connectivity": -audits["aggregate"]["connectivity_score"],
                 "negative_system_attention": -system_mass, "token_count": len(captured["prefix"]),
                 "negative_focused_attention": -focus_mass,
                 "negative_focused_connectivity": -audits["focused"]["connectivity_score"]},
        solver={name: {key: audit["diagnostics"][key] for key in
                      ("solver_converged", "solver_iterations", "relative_residual")}
                for name, audit in audits.items()},
    )
    observation["seconds"]["experiment_total"] = time.perf_counter() - started
    return observation
