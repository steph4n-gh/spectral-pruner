## 2026-08-27T22:23:41Z

You are teamwork_preview_explorer_m1_1. Your working directory is: /Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_explorer_m1_1
First, read:
- /Volumes/Storage/bigworkspace/spectral-pruner/ORIGINAL_REQUEST.md
- /Volumes/Storage/bigworkspace/spectral-pruner/AGENTS.md
- /Volumes/Storage/bigworkspace/spectral-pruner/PROJECT.md
- /Volumes/Storage/bigworkspace/spectral-pruner/src/engine.rs

Your task is to plan the exact implementation for Milestone 1: CSR Graph & BitSet Data Structures:
1. Design `CsrGraph` (contiguous `row_ptrs: Vec<usize>`, `col_indices: Vec<usize>`, and `degrees: Vec<f64>`) and `BitSet` (`words: Vec<u64>`, `len: usize`) in pure standard-library Rust with zero dependencies.
2. Ensure `CsrGraph::from_topology(topo: &Topology, sink_bits: &BitSet)` compiles undirected edges in 2 linear passes with zero per-node heap allocations.
3. Verify handling of self-loops, out-of-bounds indices, sinks, and zero-degree disconnected nodes.
4. Verify that existing public interfaces (`Topology`, `PrunerBuilder`, `TauSpectralPruner`, `PrunerResolution`) remain completely backward compatible and all existing tests continue passing.
5. Provide complete, verified code blueprints and test strategies in /Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_explorer_m1_1/handoff.md following the Handoff Protocol.
When done, message your parent with a brief summary and handoff path.
