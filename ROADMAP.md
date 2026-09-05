# Roadmap

The ambition is to detect when untrusted context redirects an LLM, early enough
to withhold a compromised response or proposed tool action. The foundation stays
a small, inspectable, zero-dependency Rust graph kernel. New application code
must earn its complexity through measured utility.

## 1. Measure behavior before building an integration

The [paired behavioral harness](research/BEHAVIORAL_EVALUATION.md) generates
responses to clean and poisoned versions of the same task. It distinguishes
successful attacker objectives, resisted attacks, other task failures, and
incomplete outputs. Signals come from the exact generation prefix, before any
output token. Thresholds are fixed on calibration cases before evaluation.

The initial exact-answer fixture checks this machinery. It does not establish
generalization, production safety, or a useful detection rate.

## 2. Establish a useful operating point

Evaluate representative tasks with enough clean task capability to make hijacks
meaningful, across at least two model families. Keep related documents and attack
campaigns within one split. Add valid response-level graders where exact matching
is insufficient. The existing label-only BIPIA adapter is not such a grader.

Compare spectral connectivity with instruction-attention and length baselines
on identical examples. Include matched-length attacks, benign discussion of
attacks, actual task quality, uncertainty, convergence, and end-to-end cost.
Recheck attack success after representation changes. Publish negative findings.

The proposed advancement target is **at least a 50% reduction in successful
hijacks with no more than 1% of clean requests withheld**, on held-out tasks with
enough independent examples to assess those rates. These are targets, not results.
The August pilot's indirect-injection failure remains unresolved.

Done means evidence that the spectral signal improves a useful operating point
over simple baselines. If it cannot, reconsider the signal before adding a product.

## 3. Prove a small integration

First measure counterfactual response withholding, as the harness already does.
If the evidence gate passes, build one narrow host integration that can withhold
a response or proposed action before release. Verify timing, failure handling,
legitimate task quality, and the effect of the intervention itself.

Automatic context deletion or masking requires additional evidence that the
detector localizes harmful content and that editing it preserves useful behavior.
There is no commitment to rebuilding tau-gate, adding a daemon, or shipping a
general-purpose LLM framework.

## 4. Keep the kernel ready to ship

Maintain library, CLI, examples, Python checks, and numerical oracle coverage.
Validate clean installation/package contents, breaking API changes, and the
small-graph conventions in `MIGRATION.md`. Publish the 2.0 candidate only when its
source commit, checks, migration notes, and numerical limits are reviewable together.

Profile model attention collection, graph preparation, solver work, and output
allocation separately. Optimize demonstrated bottlenecks while preserving the
zero-dependency boundary and mathematical invariants. Bindings and other graph
applications remain possible when a concrete caller justifies them.
