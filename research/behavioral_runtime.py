"""Optional model runtime for the paired behavioral experiment.

Heavy imports stay inside the runtime so experiment/contract tests work offline.
"""

from hashlib import sha256
import copy
import json
import time


def checked_continuation(prefix, baseline, instrumented):
    """Refuse to associate an attention observation with different behavior."""
    if baseline[:len(prefix)] != prefix or instrumented[:len(prefix)] != prefix:
        raise ValueError("generation did not preserve the exact input prefix")
    if baseline != instrumented:
        raise ValueError("attention instrumentation changed the generated token IDs")
    continuation = instrumented[len(prefix):]
    if not continuation:
        raise ValueError("model generated no continuation")
    return continuation


def prefill_snapshot(generation_attentions, prefix_length):
    """Use only the forward pass preceding the first generated token."""
    if not generation_attentions or not generation_attentions[0]:
        raise ValueError("generation returned no prefill attention")
    snapshot = generation_attentions[0]
    if any(layer is None or tuple(layer.shape[-2:]) != (prefix_length, prefix_length)
           for layer in snapshot):
        raise ValueError("prefill attention must cover the exact full input prefix")
    return snapshot


class BehavioralModel:
    def __init__(self, args):
        import torch
        import transformers
        import attention_graph

        self.torch = torch
        self.graph_tools = attention_graph
        self.args = args
        self.tokenizer, self.model, self.device = attention_graph.load_model(
            args.model, args.device, args.revision
        )
        if self.model.config.is_encoder_decoder:
            raise ValueError("this experiment requires a decoder-only causal model")
        resolved_revision = getattr(self.model.config, "_commit_hash", None)
        if resolved_revision != args.revision:
            raise ValueError("loaded model revision does not match the pinned commit")
        original = self.model.generation_config
        if original.eos_token_id is None:
            raise ValueError("model must define an EOS token for complete-response grading")
        eos = original.eos_token_id
        self.eos_ids = set(eos if isinstance(eos, (list, tuple)) else [eos])
        self.config = transformers.GenerationConfig(
            do_sample=False, num_beams=1, use_cache=True,
            temperature=1.0, top_p=1.0, top_k=50,
            return_dict_in_generate=True, output_attentions=False,
            max_new_tokens=args.max_new_tokens,
            eos_token_id=eos,
            pad_token_id=original.pad_token_id if original.pad_token_id is not None
            else min(self.eos_ids),
            bos_token_id=original.bos_token_id,
        )
        self.metadata = {
            "model": args.model,
            "model_revision": resolved_revision,
            "tokenizer_revision": args.revision,
            "tokenizer_class": type(self.tokenizer).__name__,
            "chat_template_sha256": sha256(
                (self.tokenizer.chat_template or "").encode()
            ).hexdigest(),
            "device": self.device,
            "dtype": str(self.model.dtype),
            "torch_version": torch.__version__,
            "transformers_version": transformers.__version__,
            "requested_generation_config": self.config.to_dict(),
            "model_generation_config": original.to_dict(),
            "instrumentation_override": {"output_attentions": True},
            "attention_implementation": "eager",
        }

    def sync(self):
        if self.device.startswith("cuda"):
            self.torch.cuda.synchronize(self.device)
        elif self.device == "mps":
            self.torch.mps.synchronize()

    def observe(self, system_text, user_text):
        from evaluate import run_auditor

        tools, args = self.graph_tools, self.args
        self.sync()
        total_start = time.perf_counter()
        prompt = tools.render_chat(
            self.tokenizer, system_text, user_text, generation_prompt=True
        )
        encoded, system_start, system_end = tools._find_system_token_interval(
            self.tokenizer, prompt, system_text
        )
        prefix = encoded["input_ids"][0].tolist()
        if len(prefix) > args.max_length:
            raise ValueError(f"prompt has {len(prefix)} tokens; max_length is {args.max_length}")
        context_limit = getattr(self.model.config, "max_position_embeddings", None)
        if context_limit and len(prefix) + args.max_new_tokens > context_limit:
            raise ValueError("prompt plus generation budget exceeds the model context limit")
        inputs = {key: value.to(self.device) for key, value in encoded.items()}
        self.sync()
        ready = time.perf_counter()
        observed_config = copy.deepcopy(self.config)
        observed_config.output_attentions = True
        with self.torch.inference_mode():
            baseline = self.model.generate(
                **inputs, generation_config=self.config,
            )
            self.sync()
            baseline_end = time.perf_counter()
            observed = self.model.generate(
                **inputs, generation_config=observed_config,
            )
            self.sync()
            observed_end = time.perf_counter()
            continuation = checked_continuation(
                prefix, baseline.sequences[0].tolist(), observed.sequences[0].tolist()
            )
            snapshot = prefill_snapshot(observed.attentions, len(prefix))
            bundle = tools.bundle_from_attentions(
                self.tokenizer, prompt, prefix, system_start, system_end, snapshot,
                layers=args.layers, top_k=args.top_k, min_weight=args.min_weight,
            )
            graph = bundle.aggregate
            # Before symmetrization or sparsification; never a generated-token query.
            system_attention = sum(
                float(snapshot[index][0, :, -1, system_start:system_end + 1]
                      .float().sum(dim=-1).mean())
                for index in graph.selected_layers
            ) / len(graph.selected_layers)
            self.sync()
            graph_end = time.perf_counter()

        audit = run_auditor(
            args.auditor, graph, 2.0, None, 0.1, ("--spectral-only",),
            max_iterations=args.max_iterations, tolerance=args.tolerance,
        )
        audit_end = time.perf_counter()
        metadata = tools.graph_metadata(graph, args.model, args.revision)
        metadata.pop("tokens")
        return {
            "response": self.tokenizer.decode(continuation, skip_special_tokens=True),
            "finish_reason": "eos" if continuation[-1] in self.eos_ids else "length",
            "generated_tokens": len(continuation),
            "prefix_token_ids_sha256": sha256(json.dumps(prefix).encode()).hexdigest(),
            "continuation_token_ids_sha256": sha256(json.dumps(continuation).encode()).hexdigest(),
            "instrumentation_preserved_token_ids": True,
            "graph": metadata,
            "signals": {
                "negative_algebraic_connectivity": -audit["connectivity_score"],
                "negative_system_attention": -system_attention,
                "token_count": len(prefix),
            },
            "solver": {key: audit["diagnostics"][key] for key in
                       ("solver_converged", "solver_iterations", "relative_residual")},
            "seconds": {
                "prompt_preparation": ready - total_start,
                "baseline_generation": baseline_end - ready,
                "instrumented_generation": observed_end - baseline_end,
                "graph_conversion": graph_end - observed_end,
                "auditor": audit_end - graph_end,
                "experiment_total": audit_end - total_start,
            },
        }
