# Audit CLI reference

`spectral-pruner-audit` reads one weighted undirected graph and writes one JSON
audit. It uses the same engine as the Rust API and makes no changes to external
systems.

## Install from the current checkout

The weighted API is on the `2.0.0-rc.1` development line. The registry's `1.0.0`
release predates it; use the repository checkout until the candidate is published.

```sh
git clone https://github.com/steph4n-gh/spectral-pruner.git
cd spectral-pruner
cargo install --path . --locked --bin spectral-pruner-audit
spectral-pruner-audit --version
```

For local development, `cargo build --release --bin spectral-pruner-audit`
creates `target/release/spectral-pruner-audit` without installing anything.

## Input

```text
spectral-pruner-audit --nodes N --system-start START --system-end END [OPTIONS] FILE
```

Each non-comment row has three whitespace-separated columns:

```text
# source  target  positive_weight
0         1       0.8
0         2       0.2
1         2       0.2
```

- Nodes are zero-indexed integers below `N`. The CLI rejects invalid endpoints.
- Each row describes an undirected edge. Submit it once; duplicate or reversed
  rows add weight rather than replacing an edge.
- Weights must be positive and finite. Self-loops are ignored by graph processing.
- Empty lines and lines starting with `#` are ignored. Inline comments are not supported.
- A file path of `-` reads stdin. The CLI buffers one complete input graph before auditing;
  it does not process an unbounded sequence of graphs.

```sh
spectral-pruner-audit --nodes 7 --system-start 6 --system-end 6 \
  --threat-threshold 0.9 tests/fixtures/attention_edges.tsv
```

The fixture produces `FATAL_BLOCK`, local island `[4,5]`, and a density ratio of `1`.

## Options

| Option | Default | Meaning |
|---|---|---|
| `--nodes N` | Required | Total node count, including protected nodes and sinks. |
| `--system-start N` | Required | First protected node, inclusive. |
| `--system-end N` | Required | Last protected node, inclusive; `0` disables system policy. |
| `--sink N` | None | Exclude a node; repeat to add sinks. Protected nodes override sink membership. |
| `--tau F` | `0.0` | Finite, externally injected partition threshold. |
| `--threat-threshold F` | `2.0` | Signature density-ratio threshold. |
| `--instruction-threshold F` | `0.1` | Minimum island-to-system weight per local island node. |
| `--connectivity-threshold F` | Disabled | Block at or below this nonnegative finite connectivity score; fail closed if it is unconverged. |
| `--max-iterations N` | `10000` | Positive solver iteration budget. |
| `--tolerance F` | `1e-9` | Positive finite convergence tolerance. |
| `--spectral-only` | Off | Disable density, instruction neglect, and single-token policy triggers. |
| `--disable-density` | Off | Disable only the density trigger. |
| `--disable-neglect` | Off | Disable only the instruction-neglect trigger. |
| `--disable-tripwire` | Off | Disable only the single-token trigger. |
| `--version`, `-V` | — | Print package version and exit. |
| `--help`, `-h` | — | Print usage and exit. |

Finite extreme tau values are supported. An infinite density threshold is not
equivalent to disabling the density trigger: an infinite ratio still matches it.
Use the dedicated disable option.

## Output and process status

**Exit code `0` means an audit was emitted, including a `FATAL_BLOCK` audit.**
Inspect the JSON `action`; do not use process success as an authorization decision.
Exit code `2` means an input, configuration, I/O, or arithmetic error. The error is
written to stderr and no JSON verdict is emitted. Help and version also exit `0`.

| Action | Meaning |
|---|---|
| `ALLOW` | No containment action selected under the configured policy. |
| `GARBAGE_COLLECT` | A local island was identified without a blocking trigger. The caller chooses a response. |
| `FATAL_BLOCK` | A blocking trigger, invalid protected interval, or configured numerical failure condition fired. |

The library and CLI recommend actions; they do not remove nodes, block network
traffic, edit files, or prove whether a graph is malicious.

The top-level JSON fields are `schema_version`, `action`, `connectivity_score`,
`mainland_nodes`, `island_nodes`, and `diagnostics`. Version `1` allows additive
fields; consumers should tolerate fields they do not recognize.

Diagnostics group into:

- **Numerical quality:** `solver_converged`, `solver_iterations`, `relative_residual`.
- **Boundary and size:** `boundary_configuration_valid`, `island_node_count`, `system_node_count`.
- **Weights:** `internal_weight`, `system_weight`, `partition_cut_weight`, `island_volume`.
- **Normalized measurements:** `conductance`, `internal_density`, `boundary_density`,
  `density_ratio`, `possible_edge_density_ratio`, `instruction_connection`.
- **Policy explanations:** `connectivity_triggered`, `numerical_failure_triggered`,
  `density_triggered`, `instruction_neglect_triggered`, `single_token_triggered`.

Infinite density ratios serialize as `null`, with the corresponding
`density_ratio_status` or `possible_edge_density_ratio_status` set to `infinite`.
An unavailable small-graph residual is also `null`. Parse numbers as JSON numbers,
not fixed-width strings.

For integration, handle process errors first, then invalid configuration,
convergence requirements, and the policy action. Treat `GARBAGE_COLLECT` according
to your application; it does not automatically mean the island is safe to delete.

## Numerical and boundary conventions

Protected nodes participate throughout processing and are removed only from the
returned node lists. An upper endpoint beyond the graph is clipped to actual
nodes when the start exists. A nonzero interval with an invalid start fails closed.

Fewer than three nodes keep the documented small-graph partition convention.
For connected small graphs the zero score is a placeholder and convergence is
false. Edgeless graphs use exact zero connectivity. See [migration notes](../MIGRATION.md).

Long paths may need a higher iteration budget. `solver_converged` requires both
vector change and normalized eigenpair residual below tolerance; it does not
certify which eigenvalue the solver found. A configured connectivity policy blocks
non-convergence when system policy is enabled. Other heuristics still use the
available partition when no connectivity threshold is configured.
