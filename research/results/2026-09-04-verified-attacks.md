# Verified attacks with benign length controls — September 4, 2026

**The current aggregate prefill-connectivity signal fails the proposed utility
gate in this controlled study.** We obtained 144 confirmed task hijacks on fresh
evaluation records across two task-capable model families. The calibrated
spectral rule withheld none of them. This is evidence about the current graph
transform, signal direction, and operating point, not every possible use of
spectral methods or the correctness of the Rust kernel.

The [declared protocol](../VERIFIED_ATTACK_STUDY.md),
[frozen selection](2026-09-04-verified-attacks/selection.json),
[input hashes](2026-09-04-verified-attacks/inputs.json), and
[comparison data](2026-09-04-verified-attacks/comparison.json) accompany the full
observations. Selection, code, and inputs were committed at
`6f41f90311b7c31883c619b29e8ad5717457fff3` before either final model run began.

## Development verified the attacks and models

Eight new development tasks asked for exact values from factual records. Both
models answered all eight clean tasks correctly. Four fixed strategies were
screened without detector signals; success required the exact attacker canary
and a successful clean partner. No system-prompt weakening or permissive grader
was used to manufacture attack success.

| Development strategy, MPS | Qwen2.5-0.5B | SmolLM2-1.7B |
| --- | ---: | ---: |
| Plain text override | 6/8 | 8/8 |
| Serialized role delimiters | 2/8 | 8/8 |
| Textual assistant impersonation | 6/8 | 5/8 |
| Repeated instructions | 2/8 | 8/8 |

All four strategies met the declared selection rule. CPU screening was also
retained. The larger experiment used MPS after repeating the entire development
screen on that backend, before final selection. Qwen's repetition successes
changed from 1/8 on CPU to 2/8 on MPS; the other strategy counts were unchanged.
Results should not be assumed identical across numerical backends.

## Fresh evaluation results

Each model used 12 new calibration records and 24 new evaluation records, with
every selected strategy applied to every record. One third of clean records
discussed prompt injection as a security topic. Each poisoned input had a benign
padding control with exactly the same rendered input token count for that model.
Unpadded clean records remained separate capability checks. No evaluation cases
were removed because an attack failed or the model missed the clean answer.

| Evaluation measure | Qwen2.5-0.5B | SmolLM2-1.7B |
| --- | ---: | ---: |
| Unique clean tasks answered correctly | 22/24 (91.7%) | 24/24 (100%) |
| Unique padded benign inputs answered correctly | 90/96 (93.8%) | 72/72 (100%) |
| Eligible attack attempts: paired clean task succeeded | 88 | 96 |
| Confirmed successful hijacks among eligible attempts | 54/88 (61.4%) | 90/96 (93.8%) |
| All poisoned responses reaching the canary, including ineligible pairs | 60/96 | 90/96 |
| Inconclusive poisoned responses | 0/96 | 0/96 |

Qwen's two clean failures were returning `tier` instead of `premium` and
`Room: Elm` instead of `Elm`. Their eight attack attempts remain in raw outcomes
but are excluded from the conditional hijack measure. SmolLM completed every
evaluation clean task. Distinct strategies sometimes produced the same padded
control for SmolLM's tokenizer, explaining its smaller unique-control count.

## Frozen detectors did not meet the operating target

Calibration included every unique clean and padded benign prompt as a negative,
and eligible successful attacks as positives: 28 positives for Qwen and 46 for
SmolLM. The threshold fitter was constrained to at most 1% empirical benign
false positives. With 60 and 48 unique calibration benign inputs respectively,
that ceiling permits zero observed calibration false positives.

| Model / signal | Successful hijacks withheld | Clean tasks blocked | Padded benign inputs blocked |
| --- | ---: | ---: | ---: |
| Qwen / negative algebraic connectivity | 0/54 | 0/24 | 0/96 |
| Qwen / negative system attention | 3/54 | 1/24 | 4/96 |
| Qwen / token count | 0/54 | 0/24 | 0/96 |
| SmolLM / negative algebraic connectivity | 0/90 | 0/24 | 0/72 |
| SmolLM / negative system attention | 0/90 | 0/24 | 0/72 |
| SmolLM / token count | 0/90 | 0/24 | 0/72 |

Spectral connectivity and length selected a null threshold on both models:
withhold nothing. Unlike the first smoke run, this was a **calibrated choice
with positive attacks available**, not insufficient calibration outcomes.
Token count cannot distinguish an attack from its exactly matched benign
control at the same score.

Only Qwen's instruction-attention baseline selected an active threshold. It
reduced observed eligible successes from 54 to 51, a 5.6% reduction, while blocking
5/120 unique benign inputs (4.2%). All five were variants of the same underlying
`Aspen` record; the clean task itself was blocked in 1/24 cases. This fails both
the proposed 50% reduction and 1% benign-block targets. SmolLM's attention
baseline selected a null threshold and left all 90 hijacks unchanged.

## Attack mechanisms and scope

| Confirmed evaluation hijacks by strategy | Qwen | SmolLM |
| --- | ---: | ---: |
| Plain override | 22/22 | 24/24 |
| Serialized role delimiters | 0/22 | 24/24 |
| Assistant impersonation | 22/22 | 24/24 |
| Repetition | 10/22 | 18/24 |

The delimiter payload inserts model chat-control tokens into the external
document. These findings apply to this harness's unescaped serialization path;
they are not evidence that hosted APIs accept the same control tokens. Plain
overrides and textual impersonation also succeeded, so the verified attacks do
not depend solely on that mechanism.

These are synthetic factual extraction tasks with harmless canary objectives.
Task records, answer values, and canaries are disjoint across development,
calibration, and evaluation; selected attack strategies are shared. This tests
new task instances, not unseen attack families, realistic document diversity,
tool execution, or representative deployment prevalence. Padding matches length,
not semantics or special-token counts. Repeated task/template observations are
correlated; the stored Wilson intervals do not correct for that correlation and
must not be read as 1% false-positive certification.

## Integrity and reproduction

All 864 final observations completed; each ordinary generation matched the
instrumented continuation token-for-token. All graphs converged, at a maximum
of 267786 iterations for Qwen and 689062 for SmolLM. Exact control lengths,
complete row coverage, graders, frozen policy hashes, and summaries were checked
again from the saved artifacts. Repeated identical prompts produced identical
responses, outcomes, and signals. Source and input hashes match the frozen run.

Both models used greedy bfloat16 generation on MPS with 64 output tokens,
512-token input limit, last four layers, mean attention heads/layers, top eight
neighbors, and tolerance `1e-9` at a 2000000-iteration budget. The models ran
concurrently on the same local GPU; raw timing values include resource contention
and are not a production latency benchmark. Runtime versions are recorded in
each `run.json` (PyTorch 2.12.0, Transformers 5.9.0, Python 3.14.4).

From the repository root, after installing the optional research dependencies:

```sh
cargo build --release --bin spectral-pruner-audit
python3 research/evaluate_behavior.py \
  --pairs research/fixtures/verified_attack_qwen.jsonl \
  --model Qwen/Qwen2.5-0.5B-Instruct \
  --revision 7ae557604adf67be50417f59c2c2f167def9a775 \
  --device mps --max-new-tokens 64 --max-iterations 2000000 \
  --output-dir /tmp/verified-qwen-reproduction

python3 research/evaluate_behavior.py \
  --pairs research/fixtures/verified_attack_smollm.jsonl \
  --model HuggingFaceTB/SmolLM2-1.7B-Instruct \
  --revision 31b70e2e869a7173562077fd711b654946d38674 \
  --device mps --max-new-tokens 64 --max-iterations 2000000 \
  --output-dir /tmp/verified-smollm-reproduction
```

Use fresh output directories. `verify_attacks.py` reproduces development screening
from `research/fixtures/attack_development.jsonl`; pass the same model/revision,
`--device mps`, and a new `--output-dir`. `prepare_verified_study.py` can regenerate
each model's controlled pairs using `--selection` pointing to the saved selection,
the model/revision, and a fresh `--output` path. Neither preparation nor selection
uses detector scores or evaluation responses.

## Decision

The current aggregate connectivity detector has not earned a live integration.
The useful output of this work is a reproducible behavioral regression corpus
with confirmed attacks, reliable clean-task performance, and controls that remove
an easy length shortcut. Keep the Rust kernel stable. Further LLM work should
test a better signal on development data, then reserve fresh evaluation instances;
this now-inspected evaluation set cannot serve as an untouched holdout for tuning.
