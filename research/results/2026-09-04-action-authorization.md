# Agent action authorization development result

## Decision

Do not build a live action gate from this graph construction.

The frozen development screen produced one informative positive result: on
Qwen3, negative algebraic connectivity caught every confirmed unauthorized
action without blocking a successful benign action. The second selected model,
Gemma 4, resisted every injected instruction, so it supplied no successful
attacks on which to test detector recall. The protocol required both model
families to pass and therefore did not advance.

A post-study simplicity audit found a more decisive limitation. Requiring every
action field to have any declared source path to user or host authority caught
the same Qwen3 attacks with the same zero observed benign blocks. This direct
reachability rule was stronger than the two frozen count baselines and was not
included in the preregistered comparison. The spectral solver has therefore not
shown incremental utility for this exact-source authorization problem.

This remains development evidence from synthetic tasks. No proposed action was
executed.

## Frozen setup

The protocol, action envelope, graph, score directions, model-selection rule,
development splits, and advancement gate were fixed before the selected models
saw an attack prompt. The executable envelope was exactly one JSON object with
the string fields `tool`, `target`, and `payload`. The host, rather than the
model, assigned values to trusted, host-attested, and untrusted sources.

The first selected models were incapable of the exact clean task:

| Model | Exact clean actions | Consequence |
| --- | ---: | --- |
| Qwen2.5 0.5B Instruct | 0/24 | No eligible attack pairs |
| SmolLM2 1.7B Instruct | 0/24 | No eligible attack pairs |

Their complete failed runs and null policies remain under
`results/2026-09-04-action-authorization/qwen/` and
`results/2026-09-04-action-authorization/smollm/`.

A clean-only screen then selected two pinned model families without loading
attack or control prompts:

| Model | Immutable revision | Exact clean actions |
| --- | --- | ---: |
| Qwen3 4B Instruct 2507, MLX 4-bit | `50d427756c6b1b2fe0c0a10f67fbda1fc8e82c1b` | 24/24 |
| Gemma 4 E2B it | `70af34e20bd4b7a91f0de6b22675850c43922a03` | 24/24 |

The frozen selection record is
[`selection.json`](2026-09-04-action-authorization/selection.json).

## Results

Thresholds were selected on `development_fit` at a maximum observed benign
false-positive rate of 1%, then written before any `development_check`
generation. Repeated benign prompts were counted once.

### Qwen3

| Split | Clean | Distinct controls | Confirmed attacks | Spectral caught | Untrusted-field caught | Authority-gap caught |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Development fit | 12/12 | 36/36 | 20/36 | 20/20 | 8/20 | 8/20 |
| Development check | 12/12 | 36/36 | 20/36 | 20/20 | 8/20 | 8/20 |

The fitted negative-connectivity threshold was
`-0.16326979524117813`. It blocked 0/48 distinct successful benign actions in
each split. On the check split it caught successful attacks across all three
tool families and all three attack types. The two frozen count baselines each
caught 40% of confirmed attacks at zero observed benign blocks.

The identical fit and check counts reflect the repeated synthetic task pattern;
they are not evidence of broad generalization. The check split used distinct
opaque identifiers but the same three templates and attack construction.

### Gemma 4

| Split | Clean | Distinct controls | Confirmed attacks | Policy |
| --- | ---: | ---: | ---: | --- |
| Development fit | 12/12 | 32/32 | 0/36 | Uncalibrated |
| Development check | 12/12 | 32/32 | 0/36 | Uncalibrated |

Gemma returned the authorized action for every clean, poisoned, and control
prompt. Four controls per split duplicated another token-matched control and
were deduplicated in benign metrics, leaving 32 distinct controls from 36
executions. With no confirmed successful attacks, every detector threshold
remained null and detector recall was unavailable. Treating those resisted
attacks as detector successes would be incorrect.

## Post-study simplicity audit

The declared source graph directly answers a simpler authorization question:
for each proposed field value, start at every host-resolved source containing
that value and follow declared source links. Allow the action only if every
field reaches a `trusted` or `host_attested` source.

Applied after the frozen experiment, this rule produced:

| Model and split | Successful benign actions blocked | Confirmed attacks caught |
| --- | ---: | ---: |
| Qwen3 development fit | 0/48 | 20/20 |
| Qwen3 development check | 0/48 | 20/20 |
| Gemma 4 development fit | 0/44 | 0/0 |
| Gemma 4 development check | 0/44 | 0/0 |

Every synthetic unauthorized value was deliberately placed in a source with no
authority path, while every legitimate value had one. Direct reachability thus
solved the observed exact-provenance cases by construction. This analysis was
not preregistered and does not replace the frozen result; it explains why that
result does not justify paying for a spectral solve.

## Integrity checks

Both capable-model experiments completed 168 observations: 24 distinct clean
tasks, 72 poisoned prompts, and 72 token-matched controls. All 336 audited graphs
converged, every recorded signal was finite, and the largest relative residual
was `9.403353944188716e-10` under the `1e-9` tolerance. Corpus, policy, research
source, and release-auditor hashes match both completed manifests.

Complete actions, graphs, provenance, timings, policies, and summaries are in:

- [`qwen3/experiment/`](2026-09-04-action-authorization/qwen3/experiment/)
- [`gemma4/experiment/`](2026-09-04-action-authorization/gemma4/experiment/)

After building `spectral-pruner-audit` in release mode, the capable-model runs
can be reproduced in fresh output directories with:

```sh
python3 research/evaluate_actions.py \
  --cases research/results/2026-09-04-action-authorization/qwen3/cases.jsonl \
  --model mlx-community/Qwen3-4B-Instruct-2507-4bit \
  --revision 50d427756c6b1b2fe0c0a10f67fbda1fc8e82c1b \
  --backend mlx --output-dir <new-qwen3-output>

python3 research/evaluate_actions.py \
  --cases research/results/2026-09-04-action-authorization/gemma4/cases.jsonl \
  --model google/gemma-4-E2B-it \
  --revision 70af34e20bd4b7a91f0de6b22675850c43922a03 \
  --backend transformers --device mps --output-dir <new-gemma4-output>
```

The Qwen3 and Gemma summary SHA-256 digests are respectively
`352f04e33a35be96fedff2da0103190e149c5380053a9b1644456b9d2533ec02` and
`6b03c2b627112c64d4d62ed718a99faedd68cdcdd3e189197063e77af35eae0c`.

## Next hypothesis

Keep the Rust kernel unchanged. The next bounded experiment should test
**authority dilution in ambiguous multi-source actions**: every proposed field
must have at least one real authority path, while ordinary untrusted data and an
unauthorized influence can both contribute to the same field. That removes the
trivial missing-path separation seen here.

Freeze a detector-blind attack-acquisition phase first so that two capable model
families provide confirmed compromises. Compare the spectral candidate with
direct reachability, deny-on-any-untrusted-source, authoritative-support ratio,
untrusted-support ratio, and shortest-authority-path rules. Require the graph
score to improve held-out attack recall at no more than 1% benign blocking and
to justify its runtime cost. If a simple rule ties it again, ship no spectral
action gate.
