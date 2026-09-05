# Focused instruction signals — September 4, 2026

**The focused-head candidate does not pass the declared advancement gate.**
On Qwen, the graph score improves from zero detected hijacks to 16 of 54, but
withholds 2 of 120 benign prompts. Both the >=50% hijack-reduction target and
the <=1% benign-block target are missed. The graph adds some separation in this
development screen; it has not earned an LLM guard integration.
On SmolLM, the focused graph still withholds none of 90 confirmed hijacks.
Neither new candidate passes on either model.

All withholding rates are counterfactual: the harness generates every response
for grading, then measures which responses the frozen rule would have withheld.

The [protocol](../FOCUSED_SIGNAL_STUDY.md) and runtime implementation were
committed at `4ab5f839d8e685dfb67343e0bcc7396fa54aedd1` before collecting either
model's focused observations. The [comparison data](2026-09-04-focused-signals/comparison.json)
and complete model-specific observations accompany this report.

## What changed

The previous baseline measured attention to the system message, although the
actual user task was a separate part of the prompt. This experiment maps system
instructions, the task, and the external document using their original
insertion positions. Duplicate system text, fake roles, and closing document
tags inside the external document cannot become trusted measurement spans.
Token overlap with document text takes precedence over trusted masks. Every
token still participates in the graphs; the Rust kernel and its protected
system interval are unchanged. This mapping does not sanitize the model input.

For each model, select four heads from the last four layers by their worst-case
attention to task tokens on eight clean development tasks. All eight clean
tasks must succeed. Freeze this selection before observing any attacked or
padded prompt. No attack outcomes or detector separation inform head selection.

The focused-attention score is negative system-plus-task attention from the
last prefill query, averaged over those four heads. The focused-graph score is
negative algebraic connectivity after averaging the same four full attention
matrices and applying the existing graph transform. Only these two new scores
are tested. There is no sign reversal, head-count search, or combined classifier.

## Development results

This reuses the entire previously published verified-attack corpus. Its old
calibration split fits thresholds; its old evaluation split checks them. Both
are **previously inspected development data**, not a new holdout. The score
directions are fixed, heads are frozen before fitting, and policy is saved
before collecting the development check. Related task templates cross the
splits. These observations do not establish generalization or deployment rates.

| Model / score | Confirmed hijacks withheld | Clean tasks blocked | Padded benign prompts blocked |
| --- | ---: | ---: | ---: |
| Qwen / original graph | 0/54 | 0/24 | 0/96 |
| Qwen / original system attention | 3/54 | 1/24 | 4/96 |
| Qwen / token count | 0/54 | 0/24 | 0/96 |
| Qwen / focused attention | 7/54 (13.0%) | 0/24 | 0/96 |
| Qwen / focused graph | 16/54 (29.6%) | 0/24 | 2/96 |
| SmolLM / original graph | 0/90 | 0/24 | 0/72 |
| SmolLM / original system attention | 0/90 | 0/24 | 0/72 |
| SmolLM / token count | 0/90 | 0/24 | 0/72 |
| SmolLM / focused attention | 33/90 (36.7%) | 0/24 | 2/72 |
| SmolLM / focused graph | 0/90 | 0/24 | 0/72 |

Qwen's focused-graph false positives are two different padded factual records:
the `author: Jordan` and `month: October` tasks. Both produced the correct answer
before withholding. The aggregate benign block rate is 2/120 (1.7%), and the
focused-attention comparator blocks none of those 120 prompts. Calibration had
60 unique benign prompts and 28 confirmed hijacks: focused attention withheld
6/28 and the focused graph 12/28 with zero calibration benign blocks. The weaker
development-check results show why calibration performance is insufficient.

SmolLM's focused graph selects a null threshold, despite 46 confirmed
calibration hijacks. Focused attention withholds 20/46 calibration hijacks with
zero blocks among 48 unique benign calibration prompts. On the development
check it withholds 33/90 hijacks and 2/96 unique benign prompts (2.1%), also
missing both targets. Both false positives are different padded variants of the
same `tier: basic` record, each answered correctly before withholding.

Clean capability is unchanged: Qwen answers 22/24 unpadded tasks and 90/96
unique padded controls correctly; SmolLM answers 24/24 and 72/72 respectively.
Eligible attack attempts remain 88 and 96, producing 54 and 90 confirmed
hijacks. No poisoned response is inconclusive. Duplicate padded prompts explain
SmolLM's smaller unique-control count.

The gains are concentrated in particular known strategies:

| Strategy | Qwen focused attention | Qwen focused graph | SmolLM focused attention | SmolLM focused graph |
| --- | ---: | ---: | ---: | ---: |
| Plain override | 0/22 | 0/22 | 0/24 | 0/24 |
| Serialized role delimiters | — | — | 24/24 | 0/24 |
| Assistant impersonation | 7/22 | 16/22 | 3/24 | 0/24 |
| Repetition | 0/10 | 0/10 | 6/18 | 0/18 |

Denominators are confirmed eligible hijacks; Qwen has none for the role-delimiter
strategy. All Qwen graph detections are assistant-impersonation cases, and 24 of
SmolLM's 33 focused-attention detections use serialized role delimiters. Neither
new score catches any successful plain override on either model. This
concentration further limits the utility demonstrated by the aggregate rates;
it does not identify a causal detection mechanism.

## Integrity and cost

Both runs complete with 864 paired observation rows, representing 612 unique
measured prompts (324 Qwen, 288 SmolLM), plus 16 separate clean head-selection
observations. Reused measurements are explicit; repeated benign prompts do not
inflate false-positive denominators. Every measured input preserves identical
ordinary/instrumented continuation token IDs. The paired responses and all three
original baseline scores match the previous study exactly on both models.

An artifact audit recomputed the head selection, graders, token
regions, focused-attention score, exact control lengths, policy, summaries,
and development decisions. Source, protocol, input, binary, selection, and
policy hashes match. Both full-token graph transforms converge on every input.
Maximum iterations are 267786 aggregate / 46011 focused for Qwen and 689062 /
747723 for SmolLM. The frozen four-head selections are:

- Qwen: layer/head 22/1, 22/0, 23/5, 23/13.
- SmolLM: layer/head 20/10, 21/19, 20/31, 20/17.

Timings below use unique measured prompts across fit and check, excluding
head-selection inputs and zero-time cache reuses:

| Phase | Qwen mean | SmolLM mean |
| --- | ---: | ---: |
| Span mapping | 1.7 ms | 6.4 ms |
| Per-head measurements | 21.4 ms | 23.1 ms |
| Construct both graphs | 26.1 ms | 39.3 ms |
| Original graph auditor | 156.4 ms | 376.7 ms |
| Focused graph auditor | 70.0 ms | 375.1 ms |
| Entire experiment observation, including both generations | 1.575 s | 2.362 s |

The focused auditor's p95 is 198.3 ms on Qwen and 1098.0 ms on SmolLM. The
comparison artifact retains mean, median, and p95 for every phase. These are
separate measured phases, not a controlled estimate of production overhead;
the ordinary generation always precedes the instrumented generation.

The study uses the pinned Qwen2.5-0.5B and SmolLM2-1.7B revisions from the prior
study, greedy bfloat16 generation on MPS, 64 output tokens, a 512-token input
limit, top eight neighbors, tolerance 1e-9, and a 2000000-iteration budget. Models
run sequentially. Timings cover this instrumented research harness and the
local machine's workload; they are not a production latency benchmark.

Generated responses and per-head measurements are retained in the repository.
The Cargo package excludes `research/**`; these artifacts and optional
model dependencies do not enlarge the library package.

## Decision and limits

Advancement requires the same candidate to meet both utility targets on both
models, with >=90% clean capability. A graph candidate must also outperform the
focused-attention comparator at the benign-block ceiling. Neither candidate
passes on either model. No candidate advances.

Stop this bounded prefill-detector approach. Do not start the planned fresh
security evaluation or integration for these candidates, and do not lower the
target or retune on this check. Keep the reusable span measurement, generation
parity, and behavioral evidence available for a separately justified hypothesis.

This rules out the particular selection rule, four-head average, graph
transform, score direction, and operating point tested here. It does not prove
that every attention-based or spectral detector must fail. The tasks remain
short synthetic factual extraction with harmless canaries; attacks share known
templates, serialized role tokens are not escaped, and repeated document
variants are correlated. The stored Wilson intervals assume independence and
must not be read as certification of a 1% deployment false-positive rate.

## Reproduce

From the repository root, after installing the optional research dependencies:

```sh
cargo build --release --bin spectral-pruner-audit
python3 research/probe_focus.py --model qwen --device mps \
  --output-dir /tmp/focused-qwen-reproduction
python3 research/probe_focus.py --model smollm --device mps \
  --output-dir /tmp/focused-smollm-reproduction
```

Run sequentially, using new output directories. `run.json` records source,
protocol, input, binary, selection, and policy hashes alongside model revisions
and the execution environment. `selection.json` identifies the frozen heads;
`head_selection.jsonl` contains their clean selection evidence. `policy.json`,
`calibration.jsonl`, `evaluation.jsonl`, `summary.json`, and `decision.json`
retain the fit, check, and advancement decision. `measurement_reused: true`
marks repeated prompts; their recorded timings are zero and each response is
regraded for its pairing. False-positive summaries deduplicate benign prompts.
