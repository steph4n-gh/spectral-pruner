# spectral-pruner development guide

## Design boundary

The published Rust crate has zero external dependencies. Model integrations,
dataset readers, dense numerical oracles, and report generation belong under
`research/`; they must not leak into `Cargo.toml`.

The implementation deliberately stays small:

```text
Topology
  -> weighted CSR compilation
  -> shifted-Laplacian iteration with null-space projection
  -> injected-tau partition
  -> weighted boundary and partition diagnostics
  -> independently auditable policy triggers
```

## Non-negotiable invariants

1. Every active non-sink node is classified, including degree-zero nodes.
2. Degree-zero nodes retain deterministic positive initialization clamping.
3. Partitioning uses the injected `tau`; do not substitute a median, sweep, or
   maximum-gap cut.
4. Protected system nodes stay active through CSR compilation, iteration,
   partitioning, and metrics. Filter them only from returned node vectors.
5. The signature density ratio remains
   `(internal_weight * system_node_count) /
   (system_weight * island_node_count)`.
6. The single-token tripwire remains exact: one local island node, zero internal
   weight, and system weight strictly between zero and two.
7. Do not add Rust dependencies.

## Graph input

`Topology::add_edge` creates an undirected unit-weight edge.
`Topology::add_weighted_edge` creates an undirected weighted edge. The pruning
boundary rejects zero, negative, infinite, and NaN weights even if a caller
mutates the public vector directly.

Parallel submitted edges are additive. Self-loops, invalid endpoints, and edges
touching sinks are excluded from CSR processing. Protected system nodes override
sink membership and remain active.

`system_start_idx..=system_end` is inclusive. `system_end == 0` disables system
policy. A nonzero interval with no valid start fails closed and is surfaced by
`boundary_configuration_valid`.

## Diagnostics and policy

`PrunerResolution::connectivity_score` is the λ₂ estimate. The returned
`PrunerDiagnostics` includes:

- actual island and system node counts;
- internal, system-boundary, and partition-cut weights;
- island volume and weighted conductance;
- internal and boundary possible-edge densities;
- signature and possible-edge density ratios;
- instruction connection;
- one boolean for every policy trigger.

The optional `.connectivity_threshold(value)` supports a calibrated λ₂ policy.
Use `.spectral_only()` to disable density, neglect, and tripwire triggers while
retaining any configured connectivity threshold.

## Workspace and allocation scope

`PrunerWorkspace` reuses eigensolver vectors, bitsets, CSR rows, weighted CSR
entries, degrees, and cursors. Pre-size it with:

```rust
let workspace = PrunerWorkspace::with_capacity(nodes, topology.edge_count());
```

The iterative eigensolver loop performs no heap allocation after these buffers
are sized. End-to-end `prune_with_workspace` still allocates temporary and
returned partition vectors. Do not describe the entire call as allocation-free.

## Audit interchange

`spectral-pruner-audit` reads three-column weighted TSV and emits schema-versioned
JSON. A path of `-` accepts TSV through stdin, allowing another runtime to feed
the Rust core without temporary files. The CLI buffers one complete graph before
auditing. See the [CLI reference](docs/cli.md) for process status and verdict handling.

The CLI exposes the τ boundary, signature-density threshold, instruction
threshold, optional connectivity threshold, all heuristic-disable switches, and
repeatable sinks. Keep new output fields additive unless intentionally bumping
the schema version.

## Verification

Before handing off a change, run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
cargo run --quiet --example quick_start
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo build --release --bin spectral-pruner-audit
python3 -m py_compile research/*.py
python3 -m unittest discover -s research -p 'test_*.py' -v
python3 research/numerical_oracle.py
```

For measured performance, use `examples/attention_tsv_benchmark.rs` on an
extracted graph and record the model revision, node/edge counts, warmup, runs,
host, release profile, iteration budget, tolerance, and converged-run count.

## Research workflow

`research/README.md` is the operating guide. The intended evidence chain is:

1. Extract real aggregate and per-layer attention graphs.
2. Record exact model revision and prompt hash.
3. Compare λ₂ with the dense NumPy oracle on synthetic weighted graphs.
4. Evaluate untouched public splits with conductance, density, instruction
   connection, token count, and layer-trajectory baselines.
5. Choose operating thresholds only on the calibration split.
6. Report mechanism-disabled ablations and cross-domain failures.
7. Keep raw benchmark text out of prediction artifacts.

Do not present a simulated example, synthetic role label, or test-split-tuned
threshold as real security validation.
