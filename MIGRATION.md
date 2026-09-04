# Migrating from 1.0.0 to 2.0.0-rc.1

The published 1.0.0 API predates weighted topology input and audit diagnostics.
This release candidate uses a major version because adding public fields breaks
existing struct literals. It has not been published by the stabilization work.

## Construction and results

Prefer `Topology::new(n)`, `add_edge`, and `add_weighted_edge` over struct
literals. Existing topology literals must add `weighted_edges: Vec::new()`.
Result literals must provide `diagnostics`; callers should normally consume
the result returned by `prune` or `prune_with_workspace` instead.

`PrunerDiagnostics` now also reports `solver_converged`, `solver_iterations`,
`relative_residual`, and `numerical_failure_triggered`. Use these fields when
consuming connectivity estimates. The residual is
`||L v - lambda v||_2 / max_degree` for the normalized candidate vector.
Convergence requires both vector change and residual below the configured
tolerance. It does not certify the eigenvalue's ordering, nor guarantee partition
stability for repeated eigenvalues or coordinates close to the injected tau.

The small-graph convention remains: fewer than three nodes return no island and
a connectivity score of zero. For connected small graphs, that score is a
placeholder: `solver_converged` is false and the residual is absent. Edgeless
graphs use the exact zero-connectivity convention with zero iterations.

## Validation and policy

- Non-finite tau or tolerance fails `try_build`; `build` retains its documented
  panic behavior on invalid configuration. Finite extreme tau remains supported.
- Individually positive finite edge weights can still overflow their sums or
  derived arithmetic. Such inputs return `PrunerError::MathError`; the CLI exits
  with code 2 and emits no JSON verdict. Handle errors as failed audits.
- An invalid nonzero protected interval always returns `FatalBlock`, with
  `boundary_configuration_valid == false` and preserved partition coverage.
  Upper endpoints extending beyond the graph remain valid when the start exists.
- Endpoint zero continues to disable system policy. Valid edgeless graphs and
  empty local islands retain their existing containment conventions.
- When system policy is enabled, a configured connectivity threshold fails
  closed on non-convergence, even with an empty candidate island. This is
  identified by `numerical_failure_triggered`, separately from a converged score
  crossing the connectivity threshold.
- Without a connectivity threshold, the existing partition heuristics still
  operate on the available estimate. Inspect convergence before treating it as
  quantitative evidence. Increase the iteration budget for slowly mixing graphs.

The injected tau split, positive initialization of isolated nodes, protected
node participation, signature ratio, instruction neglect, and exact single-token
tripwire remain in place. Intentionally infinite density ratios for islands with
no system connection remain valid. To disable density policy, use
`density_ratio_enabled(false)`; an infinite threshold still matches an infinite
ratio.

The scale-independent iteration can change finite-budget partitions and scores.
Recalibrate deployment thresholds and regenerate numerical/performance reports;
the checked-in August pilot describes the earlier binary.

## CLI and research

Audit JSON remains schema version 1 with additive diagnostic fields. Numbers use
round-trip serialization; consumers must parse JSON numerically rather than
matching a fixed number of decimal places. Infinite ratios still serialize as
`null` with an accompanying `infinite` status.

`--max-iterations` and `--tolerance` are available in the audit CLI and evaluator.
For example, a 1,000-node path requires more than the default 10,000 iterations;
the numerical oracle checks both exhaustion and convergence with a 500,000-step
budget. Iteration is bounded by the caller's selected budget.

The evaluator rejects unconverged aggregate or layer graphs. Resume manifests
now include sampling, labels, solver settings, and research source hashes, and
saved row identities are checked against the selected records. Start a fresh
output directory for old manifests or changed settings. Model downloads remain
optional research dependencies, never Rust crate dependencies.
