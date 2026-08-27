## 2026-08-27T22:29:38Z

You are teamwork_preview_explorer_m2_1. Your working directory is: /Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_explorer_m2_1
Read:
- /Volumes/Storage/bigworkspace/spectral-pruner/ORIGINAL_REQUEST.md
- /Volumes/Storage/bigworkspace/spectral-pruner/AGENTS.md
- /Volumes/Storage/bigworkspace/spectral-pruner/PROJECT.md
- /Volumes/Storage/bigworkspace/spectral-pruner/src/graph.rs
- /Volumes/Storage/bigworkspace/spectral-pruner/src/engine.rs
- /Volumes/Storage/bigworkspace/spectral-pruner/src/lib.rs

Your task is to plan the exact implementation for Milestone 2: Accelerated Eigensolver & Reusable Workspace:
1. Design `PrunerWorkspace` with scratch vectors (`v_vec`, `v_m`, `v_prev_m`, `v_next`, `sink_bits`, `island_bits`, `csr_row_ptrs`, `csr_col_indices`, `degrees`, `cursor`) and zero-allocation methods (`new`, `with_capacity`, `reset_for_nodes`).
2. Design the accelerated Shifted Laplacian eigensolver on top of `CsrGraph`:
   - Shift parameter $\alpha = 1.0 / (2.0 \cdot d_{\max} + 1.1)$.
   - Arrington Clamping initialization ($v_i = 1.0$ for $d_i == 0.0$, $\sin(i)$ for $d_i > 0.0$).
   - Null-space projection over active non-sink nodes.
   - Cache-friendly SpMV over contiguous `csr.col_indices[csr.row_ptrs[i]..csr.row_ptrs[i+1]]`.
   - Polyak/Nesterov momentum acceleration ($\beta = 0.5$).
   - Euclidean normalization and Rayleigh quotient algebraic connectivity ($\lambda_2$).
3. Design `prune_with_workspace` API in `TauSpectralPruner` and update `prune` to delegate cleanly to `prune_with_workspace`.
4. Ensure 100% backward compatibility for all existing tests and public methods.
5. Provide complete, verified code blueprints in /Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_explorer_m2_1/handoff.md following the Handoff Protocol.
When done, message your parent with a brief summary and handoff path.
