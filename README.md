# spectral-pruner

[![Rust CI](https://github.com/steph4n-gh/spectral-pruner/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/steph4n-gh/spectral-pruner/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/spectral-pruner.svg)](https://crates.io/crates/spectral-pruner)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust dependencies: 0](https://img.shields.io/badge/Rust%20dependencies-0-green.svg)](Cargo.toml)

**Turn a weighted graph into a partition, a policy recommendation, and the
measurements behind it.** `spectral-pruner` is a zero-dependency Rust library and
CLI for auditing graph structure. It estimates algebraic connectivity, partitions
nodes using a caller-supplied threshold, and measures how a candidate island
connects to protected system nodes.

Use it to investigate exported service, dependency, transaction, or constraint
graphs. An audit reports structure; your application decides what to do with it.
The library does not modify networks, remove packages, or execute containment.
LLM attention analysis is a separate, experimental [research workflow](research/README.md).

[CLI reference](docs/cli.md) · [Mathematics](docs/mathematics.md) ·
[Examples](examples/README.md) · [Contributing](CONTRIBUTING.md) · [Roadmap](ROADMAP.md)

## Run your first audit

This checkout is **2.0.0-rc.1**, an unpublished release candidate. The registry
badge above shows the published version, currently 1.0.0; its API differs from
this documentation. See the [migration guide](MIGRATION.md).

With a stable Rust toolchain:

```sh
git clone https://github.com/steph4n-gh/spectral-pruner.git
cd spectral-pruner
cargo run --release --example quick_start
```

Expected output:

```text
Action: FATAL_BLOCK
Mainland: [0, 1, 2, 3]
Island: [4, 5]
Density ratio: 1
Converged: true
```

This deliberately constructed graph has two local island nodes connected to
protected node 6. Its density ratio exceeds the configured threshold. Node 6
participates in the calculation and is filtered from the returned partitions.

To audit the same graph through the CLI:

```sh
cargo run --release --bin spectral-pruner-audit -- \
  --nodes 7 --system-start 6 --system-end 6 --threat-threshold 0.9 \
  tests/fixtures/attention_edges.tsv
```

Input rows are `source target positive_weight`; a path of `-` reads stdin.
The CLI emits one JSON record with the action, partitions, score, and diagnostics.
**Exit status 0 means an audit was emitted, including `FATAL_BLOCK`.** Read the
JSON `action` and convergence fields before using the result. Input or numerical
errors exit with status 2 and emit no verdict. See the [full contract](docs/cli.md).

## Use the Rust library

Until this candidate is published, add the cloned checkout as a path dependency
in your application's `Cargo.toml`:

```toml
[dependencies]
spectral-pruner = { path = "../spectral-pruner" }
```

Adjust the path to your checkout. The complete [quick-start example](examples/quick_start.rs)
is also compiled and executed as a documentation test:

```rust
use spectral_pruner::{PolicyAction, PrunerError, TauSpectralPruner, Topology};

fn main() -> Result<(), PrunerError> {
    let mut graph = Topology::new(7);
    for u in 0..4 {
        for v in u + 1..4 {
            graph.add_edge(u, v);
        }
    }
    graph.add_weighted_edge(4, 5, 0.8);
    graph.add_weighted_edge(4, 6, 0.2);
    graph.add_weighted_edge(5, 6, 0.2);

    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .threat_threshold(0.9)
        .try_build()?;
    let result = pruner.prune(&graph, 6)?;

    assert!(result.diagnostics.solver_converged);
    assert_eq!(result.action, PolicyAction::FatalBlock);
    assert_eq!(result.island_nodes, vec![4, 5]);
    assert!((result.diagnostics.density_ratio - 1.0).abs() < 1e-12);

    println!("Action: {}", result.action);
    println!("Mainland: {:?}", result.mainland_nodes);
    println!("Island: {:?}", result.island_nodes);
    println!("Density ratio: {}", result.diagnostics.density_ratio);
    println!("Converged: {}", result.diagnostics.solver_converged);
    Ok(())
}
```

## What the engine provides

- Weighted, undirected graphs with additive parallel edges and reusable CSR storage.
- A shifted-Laplacian solver with convergence, iteration, and residual diagnostics.
- Deterministic injected-τ partitioning and isolated-node clamping.
- Protected nodes included throughout processing and filtered only at output.
- Density, instruction connection, conductance, and exact single-token diagnostics.
- Configurable policy triggers, optional connectivity thresholds, and versioned JSON.

Check `solver_converged` before interpreting an eigenvalue estimate. Difficult
graphs can need a larger iteration budget. A small residual checks the computed
eigenpair; it does not establish that the eigenvalue is second-smallest. The
[mathematical contract](docs/mathematics.md) covers weighting, boundary rules,
non-convergence, and the small-graph convention.

The iterative solver reuses buffers without heap allocation after sizing.
End-to-end pruning still allocates partition vectors. Benchmarks report latency
alongside convergence; timing an unfinished solve is not an accuracy result.

## Evidence and next steps

The numerical oracle compares the Rust solver with NumPy and analytical spectra,
including tiny and large weights, weak bridges, isolated nodes, and long paths.
Rust tests and offline Python checks run in CI without downloading a model.

The [August 27 attention pilot](research/results/2026-08-27-smollm2-pilot.md)
is historical evidence from the earlier implementation, not a validation of
this release candidate. It reports both positive direct-injection results and
weak indirect-injection results. It does not establish a production defense
against prompt injection; those experiments need rerunning after solver changes.

The LLM goal is measurable utility: identify successful task hijacks
while preserving legitimate answers. The [paired behavioral harness](research/BEHAVIORAL_EVALUATION.md)
measures actual responses and counterfactual withholding with frozen thresholds.
The [roadmap](ROADMAP.md) defines the evidence needed before building a live integration.

The [verified-attack study](research/results/2026-09-04-verified-attacks.md) now
provides 144 confirmed evaluation hijacks across two task-capable model families.
At a 1% calibration false-positive ceiling, the current aggregate
connectivity signal withheld none. Stronger LLM claims require a better signal
and fresh evaluation evidence.

## Develop and verify

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
cargo run --release --example benchmark_suite
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for numerical checks and local setup,
[DEVELOPMENT.md](DEVELOPMENT.md) for implementation invariants, and the
[release checklist](docs/releasing.md) for packaging.

<details>
<summary>Project artwork — the Fiedler bisection wall</summary>

![A conceptual illustration of graph bisection](assets/spectral_pruner_hero.png)

</details>

## License

Licensed under either the [MIT License](LICENSE-MIT) or the
[Apache License 2.0](LICENSE-APACHE), at your option.
