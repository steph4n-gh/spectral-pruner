## 2026-08-27T22:31:14Z

You are teamwork_preview_worker_m2_1. Your working directory is: /Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_worker_m2_1
Read:
- /Volumes/Storage/bigworkspace/spectral-pruner/ORIGINAL_REQUEST.md
- /Volumes/Storage/bigworkspace/spectral-pruner/AGENTS.md
- /Volumes/Storage/bigworkspace/spectral-pruner/PROJECT.md
- /Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_explorer_m2_1/handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your Task (Milestone 2: Accelerated Eigensolver & Reusable Workspace):
1. You exclusively own writing to:
   - `/Volumes/Storage/bigworkspace/spectral-pruner/src/engine.rs`
   - `/Volumes/Storage/bigworkspace/spectral-pruner/src/lib.rs`
   - `/Volumes/Storage/bigworkspace/spectral-pruner/src/graph.rs`
2. Implement `PrunerWorkspace` in `src/engine.rs` with fields (`v_vec`, `v_m`, `v_prev_m`, `v_next`, `sink_bits`, `island_bits`, `csr_row_ptrs`, `csr_col_indices`, `degrees`, `cursor`) and constructors/reset methods (`new`, `with_capacity`, `reset_for_nodes`).
3. Add `Topology::populate_sink_bitset(&self, sink_bits: &mut BitSet)` and `Topology::to_sink_bitset(&self) -> BitSet` in `src/engine.rs`.
4. Implement `TauSpectralPruner::prune_with_workspace` in `src/engine.rs` implementing:
   - Zero-allocation CSR compilation via `CsrGraph::compile_into`.
   - Small graph ($N < 3$) and all-isolated ($\max(d) == 0.0$) edge-case fast paths.
   - Arrington Clamping initialization ($v_i = 1.0$ for $d_i == 0.0$, $(i \text{ as } f64).\sin()$ for $d_i > 0.0$, sinks set to 0.0).
   - Continuous null-space projection over active non-sink nodes ($\mathbf{v} \leftarrow \mathbf{v} - \text{mean}(\mathbf{v})$).
   - Cache-friendly, auto-vectorizable SpMV over contiguous CSR slices (`col_indices[row_ptrs[i]..row_ptrs[i+1]]`).
   - Heavy-Ball / Polyak momentum acceleration ($\beta = 0.5$).
   - Euclidean normalization and Rayleigh quotient algebraic connectivity calculation ($\lambda_2 = v^T L v$).
   - In-place buffer copies (`v_prev_m.copy_from_slice(&v_m)`, `v_vec.copy_from_slice(&v_next)`).
   - Injected $\tau$-boundary tie-breaking bisection and volume classification.
   - BitSet-based Scale-Invariant Cluster Density Ratio, Instruction Neglect, Single-Token Tripwire, and Telemetry Separation.
5. Update `TauSpectralPruner::prune` to cleanly instantiate a workspace and delegate to `prune_with_workspace`.
6. Export `PrunerWorkspace` in `src/lib.rs`.
7. Add unit tests for `PrunerWorkspace` and verify all existing tests and benchmarks:
   - `cargo check --all-targets`
   - `cargo test --all-targets`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo tree` (must confirm 0 new dependencies)
   - `cargo run --release --example benchmark_suite`
8. Write a complete handoff report to `/Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_worker_m2_1/handoff.md` following the Handoff Protocol.
When done, message your parent with a brief summary and handoff path.
