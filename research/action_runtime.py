"""Optional deterministic model runtime for proposed agent actions."""

from hashlib import sha256
import json
import time


def render_prompt(tokenizer, system_text, user_text):
    messages = [
        {"role": "system", "content": system_text},
        {"role": "user", "content": user_text},
    ]
    if getattr(tokenizer, "chat_template", None):
        return tokenizer.apply_chat_template(
            messages, tokenize=False, add_generation_prompt=True
        )
    return f"System: {system_text}\nUser: {user_text}\nAssistant:"


class ActionModel:
    def __init__(self, args):
        import torch
        import transformers
        import attention_graph

        self.torch = torch
        self.tools = attention_graph
        self.args = args
        self.tokenizer, self.model, self.device = attention_graph.load_model(
            args.model, args.device, args.revision
        )
        if self.model.config.is_encoder_decoder:
            raise ValueError("action study requires a decoder-only causal model")
        resolved_revision = getattr(self.model.config, "_commit_hash", None)
        if resolved_revision != args.revision:
            raise ValueError("loaded model revision does not match the pinned commit")
        original = self.model.generation_config
        if original.eos_token_id is None:
            raise ValueError("model needs an EOS token for executable-action grading")
        eos = original.eos_token_id
        self.eos_ids = set(eos if isinstance(eos, (list, tuple)) else [eos])
        self.config = transformers.GenerationConfig(
            do_sample=False,
            num_beams=1,
            use_cache=True,
            temperature=1.0,
            top_p=1.0,
            top_k=50,
            return_dict_in_generate=True,
            output_attentions=False,
            max_new_tokens=args.max_new_tokens,
            eos_token_id=eos,
            pad_token_id=(original.pad_token_id if original.pad_token_id is not None
                          else min(self.eos_ids)),
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
            "generation_config": self.config.to_dict(),
        }

    def sync(self):
        if self.device.startswith("cuda"):
            self.torch.cuda.synchronize(self.device)
        elif self.device == "mps":
            self.torch.mps.synchronize()

    def count_tokens(self, system_text, user_text):
        prompt = render_prompt(self.tokenizer, system_text, user_text)
        return len(self.tokenizer(prompt, add_special_tokens=False)["input_ids"])

    def observe(self, system_text, user_text):
        self.sync()
        started = time.perf_counter()
        prompt = render_prompt(self.tokenizer, system_text, user_text)
        encoded = self.tokenizer(prompt, add_special_tokens=False, return_tensors="pt")
        prefix = encoded["input_ids"][0].tolist()
        if len(prefix) > self.args.max_length:
            raise ValueError("action-study prompt exceeds declared token budget")
        context_limit = getattr(self.model.config, "max_position_embeddings", None)
        if context_limit and len(prefix) + self.args.max_new_tokens > context_limit:
            raise ValueError("prompt plus action budget exceeds model context limit")
        inputs = {key: value.to(self.device) for key, value in encoded.items()}
        self.sync()
        ready = time.perf_counter()
        with self.torch.inference_mode():
            generated = self.model.generate(**inputs, generation_config=self.config)
        self.sync()
        finished = time.perf_counter()
        sequence = generated.sequences[0].tolist()
        if sequence[:len(prefix)] != prefix:
            raise ValueError("generation changed the exact input prefix")
        continuation = sequence[len(prefix):]
        if not continuation:
            raise ValueError("model generated no action tokens")
        finish_reason = "eos" if continuation[-1] in self.eos_ids else "length"
        return {
            "response": self.tokenizer.decode(continuation, skip_special_tokens=True),
            "finish_reason": finish_reason,
            "prefix_tokens": len(prefix),
            "generated_tokens": len(continuation),
            "prompt_sha256": sha256(prompt.encode()).hexdigest(),
            "prefix_token_ids_sha256": sha256(json.dumps(prefix).encode()).hexdigest(),
            "continuation_token_ids_sha256": sha256(
                json.dumps(continuation).encode()
            ).hexdigest(),
            "seconds": {
                "prompt_preparation": ready - started,
                "generation": finished - ready,
            },
        }


class MlxActionModel:
    def __init__(self, args):
        import mlx
        import mlx_lm
        from mlx_lm.sample_utils import make_sampler

        self.mlx_lm = mlx_lm
        self.sampler = make_sampler(temp=0.0)
        self.args = args
        self.model, self.tokenizer = mlx_lm.load(
            args.model, revision=args.revision
        )
        self.metadata = {
            "model": args.model,
            "model_revision": args.revision,
            "tokenizer_revision": args.revision,
            "tokenizer_class": type(self.tokenizer).__name__,
            "chat_template_sha256": sha256(
                (getattr(self.tokenizer, "chat_template", None) or "").encode()
            ).hexdigest(),
            "device": "metal",
            "mlx_version": mlx.__version__,
            "mlx_lm_version": mlx_lm.__version__,
            "generation_config": {
                "do_sample": False,
                "max_new_tokens": args.max_new_tokens,
            },
        }

    def count_tokens(self, system_text, user_text):
        prompt = render_prompt(self.tokenizer, system_text, user_text)
        return len(self.tokenizer.encode(prompt))

    def observe(self, system_text, user_text):
        started = time.perf_counter()
        prompt = render_prompt(self.tokenizer, system_text, user_text)
        prefix = self.tokenizer.encode(prompt)
        if len(prefix) > self.args.max_length:
            raise ValueError("action-study prompt exceeds declared token budget")
        ready = time.perf_counter()
        responses = list(self.mlx_lm.stream_generate(
            self.model,
            self.tokenizer,
            prefix,
            max_tokens=self.args.max_new_tokens,
            sampler=self.sampler,
        ))
        finished = time.perf_counter()
        if not responses:
            raise ValueError("model generated no action tokens")
        continuation = [int(response.token) for response in responses]
        finish_reason = "eos" if responses[-1].finish_reason == "stop" else "length"
        return {
            "response": "".join(response.text for response in responses),
            "finish_reason": finish_reason,
            "prefix_tokens": len(prefix),
            "generated_tokens": len(continuation),
            "prompt_sha256": sha256(prompt.encode()).hexdigest(),
            "prefix_token_ids_sha256": sha256(json.dumps(prefix).encode()).hexdigest(),
            "continuation_token_ids_sha256": sha256(
                json.dumps(continuation).encode()
            ).hexdigest(),
            "seconds": {
                "prompt_preparation": ready - started,
                "generation": finished - ready,
            },
        }


def load_action_model(args):
    if args.backend == "transformers":
        return ActionModel(args)
    if args.backend == "mlx":
        return MlxActionModel(args)
    raise ValueError("unknown action model backend")
