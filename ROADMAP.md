# Roadmap

The near-term product is a small, inspectable graph-auditing library and CLI.
Its value is measurable topology, explicit policy, and reproducible behavior.
The attention-based detector remains a research application of that kernel.

## 1. Finish the 2.0 release candidate

- Keep library, CLI, documentation examples, Python checks, and numerical oracle
  passing on the release candidate.
- Validate installation and package contents from a clean checkout.
- Review the breaking API changes and small-graph conventions in `MIGRATION.md`.
- Publish a candidate only after its source commit, checks, migration notes, and
  numerical limitations are reviewable together.

Done means a new user can install the intended version, reproduce the quick start,
interpret every verdict, and handle an unsuccessful audit correctly.

## 2. Demonstrate one real graph workflow end to end

Start with an exported weighted service or dependency graph. Preserve the mapping
from graph IDs to domain objects, choose a protected interval explicitly, and
review the resulting islands with a domain owner. Compare a normal snapshot with
a documented structural change. Existing application examples are synthetic.

Done means a reproducible input, explainable measurements, and a useful human
decision. Add an integration only when this workflow establishes what it needs.

## 3. Test indirect-injection generalization

Use paired clean and poisoned task contexts across at least two model families.
Pin revisions and preserve pairs across calibration/evaluation splits. Measure
actual attack success and benign task quality alongside detector TPR/FPR,
confidence intervals, length baselines, layer/head stability, and total latency.

Calibrate on training data, hold the evaluation data untouched, and compare
methods on identical examples. Representation mutations count as successful
attacks only after functional success is rechecked. Publish negative findings.

Done means evidence showing whether spectral measurements improve a useful
operating point on unseen tasks. The August pilot's indirect-injection failure
is an open research result, not a solved defense.

## 4. Optimize demonstrated bottlenecks

Profile graph preparation, solver iterations, output allocation, and (for LLM
experiments) model inference separately. Report convergence beside latency.
Optimize only the limiting stage at an established workload and accuracy target.
Preserve the zero-dependency Rust boundary and mathematical invariants.

There is no commitment to a new solver, binding, service, or deployment framework
until measurements or a concrete caller justify it.
