# Examples

Run examples from the repository root with `cargo run --release --example NAME`.
The domain scenarios use constructed graphs and print recommendations. They do
not connect to a service mesh, compiler, blockchain, industrial network, or LLM.
Several deliberately exit **1** on a simulated `FATAL_BLOCK`; this differs from
the audit CLI, which exits **0** whenever it emits a verdict.

| Example | What it demonstrates |
|---|---|
| `quick_start` | Small weighted graph, protected node, diagnostics, and assertions; start here. |
| `attention_tsv_benchmark` | Latency and convergence on your own weighted TSV graph. |
| `benchmark_suite` | Allocating versus workspace-reusing calls on synthetic graph families. |
| `service_mesh_audit` | Synthetic service calls and protected control-plane nodes. |
| `ics_segmentation` | Synthetic network segments and protected boundary nodes. |
| `supply_chain` | Small synthetic dependency topology and a policy decision. |
| `dependency_audit` | Named synthetic packages; reads local package names but does not reconstruct a real lockfile graph. |
| `defi_mempool_mev` | Synthetic transactions mapped to a shared-resource graph. |
| `zk_circuit_backdoor` | Synthetic constraint connectivity; does not validate proof soundness. |
| `llm_steerage_guard` | Role-coded artificial attention; does not run a model. |

For real model attention, use the [research extractor](../research/README.md).
For a graph exported by your own application, use the [audit CLI](../docs/cli.md).
Keep a separate mapping from numeric node IDs to your application's entities.

## Benchmark an exported graph

```sh
cargo run --release --example attention_tsv_benchmark -- \
  --nodes 7 --system-start 6 --system-end 6 \
  --warmup 10 --runs 100 --max-iterations 10000 --tolerance 1e-9 \
  tests/fixtures/attention_edges.tsv
```

File reading and graph construction happen before the measured loop. The loop
measures `prune_with_workspace`, including returned partitions, and reports the
solver settings and converged-run count alongside the latency percentiles.
The score checksum uses `null` if its accumulated sum exceeds the finite range.
Require `converged_runs == runs` before treating timing as the cost of a completed
solve. Record hardware, build profile, graph source, and settings when publishing
results. The synthetic suite uses fixed budgets and can report unconverged runs;
its streaming section also includes graph construction.
