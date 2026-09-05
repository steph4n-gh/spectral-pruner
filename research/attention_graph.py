"""Real Hugging Face attention extraction for spectral-pruner experiments.

This module intentionally lives outside the Rust crate: PyTorch and Transformers
are research-time tooling, while the shipped library remains dependency-free.
"""

from __future__ import annotations

from dataclasses import dataclass
from hashlib import sha256
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer


@dataclass(frozen=True)
class AttentionGraph:
    edges: tuple[tuple[int, int, float], ...]
    node_count: int
    system_start: int
    system_end: int
    tokens: tuple[str, ...]
    prompt_sha256: str
    selected_layers: tuple[int, ...]
    top_k: int
    min_weight: float

    def to_tsv(self) -> str:
        return "".join(f"{u}\t{v}\t{weight:.17g}\n" for u, v, weight in self.edges)


@dataclass(frozen=True)
class AttentionBundle:
    aggregate: AttentionGraph
    layers: tuple[AttentionGraph, ...]


def select_device(requested: str) -> str:
    if requested != "auto":
        return requested
    if torch.cuda.is_available():
        return "cuda"
    if torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def load_model(model_name: str, device: str = "auto", revision: str | None = None):
    """Load a causal LM with the eager attention path required for inspection."""
    resolved_device = select_device(device)
    tokenizer = AutoTokenizer.from_pretrained(model_name, revision=revision)
    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        revision=revision,
        attn_implementation="eager",
    )
    model.to(resolved_device)
    model.eval()
    return tokenizer, model, resolved_device


def render_chat(tokenizer, system_text: str, user_text: str, *, generation_prompt: bool = False) -> str:
    messages = [
        {"role": "system", "content": system_text},
        {"role": "user", "content": user_text},
    ]
    if getattr(tokenizer, "chat_template", None):
        return tokenizer.apply_chat_template(
            messages,
            tokenize=False,
            add_generation_prompt=generation_prompt,
        )
    suffix = "Assistant:" if generation_prompt else ""
    return f"System: {system_text}\nUser: {user_text}\n{suffix}"


def _find_system_token_interval(tokenizer, prompt: str, system_text: str):
    char_start = prompt.find(system_text)
    if char_start < 0:
        raise ValueError("rendered prompt does not contain the system message")
    char_end = char_start + len(system_text)

    try:
        encoded = tokenizer(
            prompt,
            add_special_tokens=False,
            return_offsets_mapping=True,
            return_tensors="pt",
        )
        offsets = encoded.pop("offset_mapping")[0].tolist()
        system_indices = [
            index
            for index, (start, end) in enumerate(offsets)
            if end > char_start and start < char_end
        ]
    except (NotImplementedError, TypeError, ValueError):
        encoded = tokenizer(prompt, add_special_tokens=False, return_tensors="pt")
        full_ids = encoded["input_ids"][0].tolist()
        system_ids = tokenizer(system_text, add_special_tokens=False)["input_ids"]
        system_indices = []
        for start in range(len(full_ids) - len(system_ids) + 1):
            if full_ids[start : start + len(system_ids)] == system_ids:
                system_indices = list(range(start, start + len(system_ids)))
                break

    if not system_indices:
        raise ValueError("could not map the system message to a token interval")
    if system_indices != list(range(system_indices[0], system_indices[-1] + 1)):
        raise ValueError("system message tokenization is not contiguous")
    return encoded, system_indices[0], system_indices[-1]


def parse_layer_selection(spec: str, layer_count: int) -> tuple[int, ...]:
    if layer_count <= 0:
        raise ValueError("model returned no attention layers")
    if spec == "all":
        return tuple(range(layer_count))
    if spec.startswith("last:"):
        count = int(spec.split(":", 1)[1])
        if count <= 0:
            raise ValueError("last:N requires N > 0")
        return tuple(range(max(0, layer_count - count), layer_count))

    layers = tuple(sorted({int(value) for value in spec.split(",")}))
    if not layers or any(layer < 0 or layer >= layer_count for layer in layers):
        raise ValueError(f"layer selection must be within 0..{layer_count - 1}")
    return layers


def sparsify_affinity(
    affinity: torch.Tensor,
    top_k: int,
    min_weight: float,
) -> tuple[tuple[int, int, float], ...]:
    """Keep an undirected edge when either endpoint ranks it in its top-k."""
    node_count = affinity.shape[0]
    if top_k <= 0:
        raise ValueError("top_k must be positive")
    if min_weight < 0:
        raise ValueError("min_weight cannot be negative")

    keep: set[tuple[int, int]] = set()
    count = min(top_k, max(0, node_count - 1))
    if count == 0:
        return ()
    for source in range(node_count):
        for target in torch.topk(affinity[source], k=count).indices.tolist():
            if source == target:
                continue
            u, v = sorted((source, target))
            if float(affinity[u, v]) >= min_weight:
                keep.add((u, v))

    return tuple(
        (u, v, float(affinity[u, v]))
        for u, v in sorted(keep)
        if float(affinity[u, v]) > 0.0
    )


@torch.inference_mode()
def extract_attention_bundle(
    tokenizer,
    model,
    device: str,
    system_text: str,
    user_text: str,
    *,
    layers: str = "last:4",
    top_k: int = 8,
    min_weight: float = 0.0,
    max_length: int = 512,
) -> AttentionBundle:
    prompt = render_chat(tokenizer, system_text, user_text)
    encoded, system_start, system_end = _find_system_token_interval(
        tokenizer, prompt, system_text
    )
    if encoded["input_ids"].shape[1] > max_length:
        raise ValueError(
            f"prompt has {encoded['input_ids'].shape[1]} tokens; max_length is {max_length}"
        )
    inputs = {key: value.to(device) for key, value in encoded.items()}

    outputs = model(
        **inputs,
        output_attentions=True,
        use_cache=False,
        return_dict=True,
    )
    return bundle_from_attentions(
        tokenizer, prompt, encoded["input_ids"][0].tolist(), system_start, system_end,
        outputs.attentions, layers=layers, top_k=top_k, min_weight=min_weight,
    )


def bundle_from_attentions(
    tokenizer, prompt: str, input_ids: list[int], system_start: int, system_end: int,
    attentions, *, layers: str, top_k: int, min_weight: float,
) -> AttentionBundle:
    """Convert a complete prefill attention snapshot with the existing graph transform."""
    if not attentions:
        raise RuntimeError(
            "model did not return attentions; verify that its architecture supports "
            "output_attentions with the eager attention implementation"
        )
    selected_layers = parse_layer_selection(layers, len(attentions))

    layer_means = []
    for index in selected_layers:
        attention = attentions[index]
        if attention is None or tuple(attention.shape[-2:]) != (len(input_ids), len(input_ids)):
            raise ValueError("attention snapshot must cover the exact full input prefix")
        layer_means.append(attention[0].float().mean(dim=0))

    def build_graph(directed: torch.Tensor, layer_ids: tuple[int, ...]) -> AttentionGraph:
        return graph_from_directed(
            tokenizer, prompt, input_ids, system_start, system_end, directed,
            layer_ids=layer_ids, top_k=top_k, min_weight=min_weight,
        )

    layer_graphs = tuple(
        build_graph(attention, (layer_index,))
        for layer_index, attention in zip(selected_layers, layer_means)
    )
    aggregate = build_graph(torch.stack(layer_means).mean(dim=0), selected_layers)
    return AttentionBundle(aggregate=aggregate, layers=layer_graphs)


def graph_from_directed(tokenizer, prompt, input_ids, system_start, system_end,
                        directed, *, layer_ids, top_k, min_weight):
    """Apply the same full-token transform to an explicitly chosen head average."""
    if tuple(directed.shape) != (len(input_ids), len(input_ids)):
        raise ValueError("directed attention must cover every input token")
    if not bool(torch.isfinite(directed).all()) or bool((directed < 0).any()):
        raise ValueError("attention weights must be finite and nonnegative")
    affinity = (directed + directed.transpose(0, 1)) * 0.5
    affinity.fill_diagonal_(0.0)
    return AttentionGraph(
        edges=sparsify_affinity(affinity.cpu(), top_k, min_weight),
        node_count=len(input_ids), system_start=system_start, system_end=system_end,
        tokens=tuple(tokenizer.convert_ids_to_tokens(input_ids)),
        prompt_sha256=sha256(prompt.encode("utf-8")).hexdigest(),
        selected_layers=layer_ids, top_k=top_k, min_weight=min_weight,
    )


def extract_attention_graph(*args, **kwargs) -> AttentionGraph:
    return extract_attention_bundle(*args, **kwargs).aggregate


def graph_metadata(
    graph: AttentionGraph, model_name: str, model_revision: str | None = None
) -> dict:
    return {
        "schema_version": 1,
        "model": model_name,
        "model_revision": model_revision,
        "node_count": graph.node_count,
        "edge_count": len(graph.edges),
        "system_start": graph.system_start,
        "system_end": graph.system_end,
        "selected_layers": list(graph.selected_layers),
        "aggregation": {
            "heads": "mean",
            "layers": "mean",
            "causal_symmetrization": "(A + A.T) / 2",
            "top_k_per_node": graph.top_k,
            "min_weight": graph.min_weight,
        },
        "prompt_sha256": graph.prompt_sha256,
        "tokens": list(graph.tokens),
    }
