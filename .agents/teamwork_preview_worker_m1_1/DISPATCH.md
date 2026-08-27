## 2026-08-27T22:26:02Z

You are teamwork_preview_worker_m1_1. Your working directory is: /Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_worker_m1_1
First, read:
- /Volumes/Storage/bigworkspace/spectral-pruner/ORIGINAL_REQUEST.md
- /Volumes/Storage/bigworkspace/spectral-pruner/AGENTS.md
- /Volumes/Storage/bigworkspace/spectral-pruner/PROJECT.md
- /Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_explorer_m1_1/handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your Task (Milestone 1: CSR Graph & BitSet Data Structures):
1. You exclusively own writing to:
   - `/Volumes/Storage/bigworkspace/spectral-pruner/src/graph.rs`
   - `/Volumes/Storage/bigworkspace/spectral-pruner/src/lib.rs`
   - `/Volumes/Storage/bigworkspace/spectral-pruner/src/engine.rs`
2. Implement `BitSet` and `CsrGraph` in `src/graph.rs` per the blueprints in the Explorer handoff.
3. Update `src/lib.rs` to declare `pub mod graph;` and export `BitSet`, `CsrGraph`.
4. Add `Topology::to_sink_bitset(&self)` helper in `src/engine.rs`.
5. Add unit tests for `BitSet` and `CsrGraph` in `src/graph.rs` covering edge cases, sinks, self-loops, out-of-bounds, and disconnected nodes.
6. Verify your implementation by running:
   - `cargo check --all-targets`
   - `cargo test` (all 7 existing tests + new graph tests must pass)
   - `cargo tree` (must confirm 0 new dependencies)
   - `cargo clippy --all-targets -- -D warnings`
7. Write a complete handoff report to `/Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_worker_m1_1/handoff.md` following the Handoff Protocol (Observation, Logic Chain, Caveats, Conclusion, Verification Method).
When done, message your parent with a brief summary and handoff path.
