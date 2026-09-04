# Mathematical and boundary contract

For weighted adjacency matrix `A`, the engine computes the combinatorial
Laplacian `L = D - A`, projects away the constant-vector null space, and uses
power iteration on a shifted operator to estimate the Fiedler vector and λ₂.
Nodes are partitioned by the caller-injected boundary:

```text
v_i <= tau    versus    v_i > tau
```

The larger side is the mainland. The smaller side is the candidate island.
Protected system nodes participate in the graph, eigensolver, partition, and
metrics. They are removed only from the returned node vectors.

For local island `I` and protected system set `S`, the signature ratio is:

```text
R_signature = (internal_weight * |S|) / (system_weight * |I|)
```

The diagnostics also expose the stricter possible-edge normalization:

```text
internal_density = 2 * internal_weight / (|I| * (|I| - 1))
boundary_density = system_weight / (|I| * |S|)
possible_edge_density_ratio = internal_density / boundary_density
```

Every undirected edge is counted once. `system_node_count` is the number of
actual in-range nodes, not the numeric boundary endpoint. A zero boundary
endpoint disables system policy. A nonzero invalid interval fails closed.

The full policy can trigger on:

- invalid protected intervals, including on empty and small graphs;
- an unconverged estimate when a connectivity threshold is configured;
- calibrated `lambda_2 <= connectivity_threshold`, when configured;
- `density_ratio >= threat_threshold`;
- `instruction_connection < instruction_connection_threshold`;
- the exact single-token tripwire.

Use `.spectral_only()` to disable the three non-spectral policy heuristics for a
clean λ₂ baseline or calibrated connectivity-only deployment.

Check `diagnostics.solver_converged`, `solver_iterations`, and `relative_residual`
before interpreting an estimate. Long paths and weakly coupled graphs can need
more iterations; the CLI exposes `--max-iterations` and `--tolerance`. A calibrated
connectivity policy fails closed on non-convergence when system policy is enabled.
The residual measures eigenpair accuracy, not proof of eigenvalue ordering.
See [MIGRATION.md](../MIGRATION.md) for the 2.0.0 release candidate's compatibility
changes and the small-graph convention.

## Boundary and sink rules

- `system_start_idx..=system_end` is an inclusive protected interval.
- Protected system nodes remain active even if also submitted as sinks; the
  protected interval takes precedence.
- Sinks are excluded from graph processing and classification.
- Every active non-sink node, including degree-zero nodes, is classified.
- Weighted edges must have finite weights greater than zero.

## Interpretation limits

A structural anomaly is a candidate for investigation. Neither a partition nor
a policy verdict establishes malicious intent. Graph construction, weight units,
protected-node selection, and threshold calibration determine what an audit means
in a particular application.

Uniformly rescaling weights rescales combinatorial algebraic connectivity and
instruction connection. It leaves the signature density ratio unchanged, but
it can change an absolute connectivity threshold or the exact single-token
tripwire. Record weight units when calibrating a policy.

Repeated eigenvalues can admit multiple valid Fiedler vectors. Deterministic
initialization makes repeated runs on the same representation reproducible; it
does not promise an identical cut under node relabeling or a unique optimal cut.
