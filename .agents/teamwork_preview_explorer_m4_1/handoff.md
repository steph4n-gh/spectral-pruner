# Milestone 4 Handoff Report: Comprehensive Testing, Benchmarking & Fuzzing Architecture

**Agent**: `teamwork_preview_explorer_m4_1`  
**Milestone**: M4 — Comprehensive Testing, Benchmarking & Fuzzing  
**Target Files**:
- `tests/e2e_tier1_features.rs` (Tier 1: Feature Coverage — >=5 test cases per feature for all 21 features)
- `tests/e2e_tier2_boundaries.rs` (Tier 2: Boundary & Extreme Topologies)
- `tests/e2e_tier3_combinatorial.rs` (Tier 3: Combinatorial & Multi-Feature Interactions)
- `tests/e2e_tier4_applications.rs` (Tier 4: Domain Audit Scenarios — LLM, ZK, DeFi, ICS, Supply Chain)
- `tests/fuzz_adversarial.rs` (Adversarial Fuzzer — 10,000+ Random Topologies & Invariant Conservation)
- `examples/benchmark_suite.rs` (High-Resolution Release Benchmark Suite Uplift)
- `TEST_READY.md` (Test Infrastructure Verification & Readiness Manual)

---

## 1. Observation

Direct code examination and execution of the codebase established the following ground truths:

### 1.1 Codebase Structure & Interfaces
1. **Engine Layer (`src/engine.rs`)**:
   - `Topology` builder (`src/engine.rs:8-51`): Supports `num_nodes: usize`, `edges: Vec<(usize, usize)>`, and `sinks: BTreeSet<usize>`. Populates `BitSet` via `populate_sink_bitset` and `to_sink_bitset`.
   - `PrunerWorkspace` (`src/engine.rs:56-139`): Provides contiguous scratchpad buffers (`v_vec`, `v_m`, `v_prev_m`, `v_next`, `sink_bits`, `island_bits`, `csr_row_ptrs`, `csr_col_indices`, `degrees`, `cursor`). Resets via `reset_for_nodes(n)` with zero heap allocations during streaming execution.
   - `TauSpectralPruner` (`src/engine.rs:169-545`):
     - `prune(&self, topology: &Topology, system_boundary_len: usize) -> Result<PrunerResolution, PrunerError>`
     - `prune_with_workspace(&self, topology: &Topology, system_boundary_len: usize, workspace: &mut PrunerWorkspace) -> Result<PrunerResolution, PrunerError>`
     - Fast paths: $N < 3$ (`src/engine.rs:274-284`), all disconnected $\max(d) == 0$ (`src/engine.rs:297-308`).
     - Shifted Laplacian operator $M = I - \alpha L$ with $\alpha = \frac{1}{2 \cdot d_{\max} + 1.1}$ (`src/engine.rs:310-312`).
     - Arrington Clamping: disconnected nodes $d_i = 0.0$ initialized to $1.0$; connected nodes initialized to $\sin(i)$ (`src/engine.rs:314-325`).
     - Continuous Null-Space Projection: active nodes centered by subtracting $\text{mean}(v)$ (`src/engine.rs:340-355`).
     - Heavy-Ball Momentum: $v_{k+1} = v_m + \beta (v_m - v_{\text{prev\_m}})$ (`src/engine.rs:374-382`).
     - Rayleigh Quotient: $\lambda_2 = v^T L v$ (`src/engine.rs:404-419`).
     - Injected $\tau$-Boundary Tie-Breaking: $v_i \le \tau$ vs $v_i > \tau$ (`src/engine.rs:430-450`).
     - Threat Metrics:
       - Scale-Invariant Density Ratio: $\frac{\text{internal} \times N_{\text{system}}}{\text{to\_system} \times N_{\text{island}}}$ (`src/engine.rs:496-502`).
       - Instruction Neglect: $\frac{\text{to\_system}}{N_{\text{island}}} < 0.1 \implies \text{FatalBlock}$ (`src/engine.rs:504-509`).
       - Single-Token Tripwire: $N_{\text{island}} == 1 \land \text{internal} == 0 \land 0 < \text{to\_system} < 2 \implies \text{FatalBlock}$ (`src/engine.rs:512-514`).
     - Telemetry Separation: System boundary $[system\_start\_idx, system\_boundary\_len]$ participate in all algebraic and metric computations, and are stripped only in final `PrunerResolution` (`src/engine.rs:529-536`).
   - `PrunerBuilder` (`src/engine.rs:548-645`): Fluent builder validating `tolerance > 0.0`, `max_iterations > 0`, `momentum_beta \in [0.0, 1.0)`, `threat_threshold >= 0.0`. Provides `try_build()` and `build()`.
2. **Graph Layer (`src/graph.rs`)**:
   - `BitSet` (`src/graph.rs:8-185`): Flat `[u64]` bitmasks with `insert`, `remove`, `contains`, `clear`, `reset_with_len`, `count_ones` (POPCNT), and `iter_ones` iterator.
   - `CsrGraph` (`src/graph.rs:194-361`): Contiguous 2-vector CSR representation (`row_ptrs`, `col_indices`, `degrees`). Compiles via `from_topology` and `compile_into` in 2 linear passes ($O(N + E)$).
3. **Error Layer (`src/error.rs`)**:
   - `PrunerError`: `MathError(String)`, `MalformedTopology(String)`.
4. **Current Test Status**:
   - Baseline unit tests in `src/lib.rs` (37 tests), `tests/empirical_challenge_m1.rs` (16 tests), `tests/empirical_challenge_m2.rs` (13 tests), `tests/empirical_challenge_m3.rs` (26 tests), `tests/empirical_challenge_m3_2.rs` (10 tests) all pass cleanly (total 102 tests).
   - Zero compilation warnings under `cargo test` and `cargo check`.
5. **Benchmark Status (`examples/benchmark_suite.rs`)**:
   - Profiles Clique, Star, and Decoupled clusters across $N \in [10, 100, 500]$. Runs with zero external benchmarking crates in under 1 second.

---

## 2. Logic Chain

### 2.1 Mapping PROJECT.md Features to Test Suites

| # | Feature Name | Core Invariant / Mechanism | Target Test File | Required Cases |
|---|--------------|----------------------------|------------------|:--------------:|
| 1 | `Topology` Graph Builder | In-bounds/OOB edge insertion, sinks, bitset conversion | `tests/e2e_tier1_features.rs` | >= 5 |
| 2 | Contiguous `CsrGraph` | 2-pass compilation, contiguous neighbor slicing, degrees | `tests/e2e_tier1_features.rs` | >= 5 |
| 3 | Fast `BitSet` Bitmasks | 64-bit boundaries, POPCNT, iter_ones, reset_with_len | `tests/e2e_tier1_features.rs` | >= 5 |
| 4 | Edge-Case Graph Handling | $N < 3$ fast paths, all-disconnected fast path | `tests/e2e_tier1_features.rs` | >= 5 |
| 5 | Arrington Clamping | Isolated node ($d_i=0$) clamped to 1.0 at init | `tests/e2e_tier1_features.rs` | >= 5 |
| 6 | Shifted Laplacian SpMV | $M = I - \alpha L$, $\alpha = 1/(2 d_{\max} + 1.1)$ SpMV | `tests/e2e_tier1_features.rs` | >= 5 |
| 7 | Null-Space Projection | Center active non-sink nodes $v \leftarrow v - \text{mean}(v)$ | `tests/e2e_tier1_features.rs` | >= 5 |
| 8 | Momentum Acceleration | Heavy-Ball $\beta$ step $v_{m} + \beta (v_m - v_{prev})$ | `tests/e2e_tier1_features.rs` | >= 5 |
| 9 | Rayleigh Quotient $\lambda_2$ | Algebraic connectivity $v^T L v$ computation | `tests/e2e_tier1_features.rs` | >= 5 |
| 10 | Reusable `PrunerWorkspace` | Zero-allocation streaming execution and capacity reuse | `tests/e2e_tier1_features.rs` | >= 5 |
| 11 | Injected $\tau$-Boundary Split | Rigid $v_i \le \tau$ vs $v_i > \tau$ bisection | `tests/e2e_tier1_features.rs` | >= 5 |
| 12 | Scale-Invariant Density Ratio | $\frac{\text{internal} \times N_{\text{system}}}{\text{to\_system} \times N_{\text{island}}}$ threat metric | `tests/e2e_tier1_features.rs` | >= 5 |
| 13 | Instruction Neglect | $\frac{\text{to\_system}}{N_{\text{island}}} < 0.1 \implies \text{FatalBlock}$ | `tests/e2e_tier1_features.rs` | >= 5 |
| 14 | Single-Token Tripwire | $N=1, \text{internal}=0, 0 < \text{to\_system} < 2 \implies \text{FatalBlock}$ | `tests/e2e_tier1_features.rs` | >= 5 |
| 15 | Policy Verdict Mapping | `Allow`, `GarbageCollect`, `FatalBlock` resolution | `tests/e2e_tier1_features.rs` | >= 5 |
| 16 | Telemetry Separation | Strip system nodes $[system\_start\_idx, system\_boundary\_len]$ at output | `tests/e2e_tier1_features.rs` | >= 5 |
| 17 | Configuration & Validation | `PrunerBuilder` validation and `PrunerError` errors | `tests/e2e_tier1_features.rs` | >= 5 |
| 18 | Invariant Baseline Tests | Parity with 7 baseline unit tests | `tests/e2e_tier1_features.rs` | >= 5 |
| 19 | E2E & Property Tests | Disjoint union partition conservation & determinism | `tests/e2e_tier1_features.rs` | >= 5 |
| 20 | Fuzzing & Adversarial Harness | Random graph generation, degree conservation, zero panic | `tests/e2e_tier1_features.rs` | >= 5 |
| 21 | Benchmark Suite Uplift | Latency/throughput verification on diverse topologies | `tests/e2e_tier1_features.rs` | >= 5 |

---

### 2.2 Blueprint: `tests/e2e_tier1_features.rs`
**Objective**: Direct feature verification with >= 5 test cases per feature across all 21 PROJECT.md features (total >= 105 test cases).

**Structure**:
1. **Module 1: `feature_01_topology_builder`** (5 tests):
   - `test_f01_empty_and_capacity_construction`: Validates `num_nodes`, empty edge list, empty sinks.
   - `test_f01_valid_edges_and_out_of_bounds_filtering`: Tests edge insertion and drops for $u \ge N$ or $v \ge N$.
   - `test_f01_sink_addition_and_bounds`: Tests `add_sink` within bounds and OOB drops.
   - `test_f01_to_sink_bitset_conversion`: Tests exact parity between `sinks: BTreeSet` and generated `BitSet`.
   - `test_f01_populate_sink_bitset_in_place_reuse`: Tests in-place mutation and resizing of existing `BitSet`.
2. **Module 2: `feature_02_csr_graph`** (5 tests):
   - `test_f02_from_topology_exact_prefix_sums`: Validates linear 2-pass `row_ptrs` and `col_indices`.
   - `test_f02_neighbor_slicing_and_out_of_bounds`: Tests `csr.neighbors(u)` returns contiguous slices and `&[]` for $u \ge N$.
   - `test_f02_degree_tracking_and_max_degree`: Tests `csr.degree(u)` and `csr.max_degree()` match graph edges.
   - `test_f02_self_loop_and_sink_filtering`: Verifies that self-loops and sink-connected edges are excluded from CSR storage.
   - `test_f02_compile_into_workspace_zero_alloc`: Tests in-place CSR compilation into pre-allocated vectors.
3. **Module 3: `feature_03_bitset_masks`** (5 tests):
   - `test_f03_word_boundaries_64_128_192`: Verifies bit operations around 64-bit boundaries ($0, 63, 64, 127, 128$).
   - `test_f03_insert_remove_contains_idempotence`: Tests multiple inserts/removes.
   - `test_f03_clear_and_reset_with_len`: Tests buffer reuse without deallocation.
   - `test_f03_count_ones_hardware_popcnt`: Tests `count_ones()` matches manual count.
   - `test_f03_iter_ones_exhaustive_traversal`: Verifies `iter_ones()` yields indices in ascending order.
4. **Module 4: `feature_04_edge_case_fast_paths`** (5 tests):
   - `test_f04_n0_empty_graph_fast_path`: Resolves to `Allow`, empty mainland/island.
   - `test_f04_n1_single_node_fast_path`: Resolves to `Allow`, mainland `[0]`, empty island.
   - `test_f04_n2_two_node_fast_path`: Resolves to `Allow`, mainland `[0, 1]`.
   - `test_f04_n2_with_sink_fast_path`: Resolves to `Allow`, mainland `[0]`, sink 1 excluded.
   - `test_f04_all_disconnected_max_degree_zero`: 10 isolated nodes, returns `Allow`, mainland `0..10`.
5. **Module 5: `feature_05_arrington_clamping`** (5 tests):
   - `test_f05_single_isolated_node_clamped_to_1_0`: Validates degree-0 node is clamped to 1.0 and classified.
   - `test_f05_multiple_isolated_nodes_clamped`: 5 connected nodes + 3 isolated nodes.
   - `test_f05_isolated_node_with_sinks`: Isolated nodes alongside sink nodes.
   - `test_f05_isolated_nodes_with_system_boundary`: Isolated nodes with system boundary len > 0.
   - `test_f05_clamping_determinism_across_restarts`: Re-running produces identical partition output.
6. **Module 6: `feature_06_shifted_laplacian_spmv`** (5 tests):
   - `test_f06_alpha_scaling_by_max_degree`: $\alpha = 1 / (2 d_{\max} + 1.1)$ on star, path, and clique.
   - `test_f06_spmv_csr_slice_multiplication`: Manual SpMV vs workspace SpMV step.
   - `test_f06_sink_rows_zeroed_in_spmv`: Sinks are zeroed out during power step.
   - `test_f06_spmv_energy_preservation`: Eigenvector norm remains bounded during power iteration.
   - `test_f06_linear_convergence_rate`: Verifies $v_{k+1} - v_k \to 0$ monotonically.
7. **Module 7: `feature_07_null_space_projection`** (5 tests):
   - `test_f07_zero_mean_invariant`: After projection, $\sum_{i \in active} v_i < 1e-12$.
   - `test_f07_sink_exclusion_from_mean`: Sinks are not counted in active mean calculation.
   - `test_f07_all_ones_null_space_invariance`: Projection removes all-ones component.
   - `test_f07_odd_vs_even_active_nodes`: Mean centering works identically on odd and even sizes.
   - `test_f07_single_active_node_projection`: Degenerate case produces $v_0 = 0.0$.
8. **Module 8: `feature_08_momentum_acceleration`** (5 tests):
   - `test_f08_beta_zero_standard_power_iteration`: Momentum disabled behaves as pure power iteration.
   - `test_f08_beta_default_0_5_accelerates_convergence`: Faster convergence with $\beta=0.5$ vs $\beta=0.0$.
   - `test_f08_beta_sweep_0_1_to_0_9`: Verifies stability across full parameter range $\beta \in [0.1, 0.9]$.
   - `test_f08_momentum_reset_in_workspace`: `v_prev_m` initialized to initial normalized vector.
   - `test_f08_momentum_on_dense_cliques`: Dense graphs converge within few iterations under momentum.
9. **Module 9: `feature_09_rayleigh_quotient`** (5 tests):
   - `test_f09_complete_clique_connectivity`: $\lambda_2$ matches expected algebraic connectivity.
   - `test_f09_star_graph_connectivity`: Hub-and-spoke algebraic connectivity.
   - `test_f09_path_graph_connectivity`: Path graph low $\lambda_2$.
   - `test_f09_disconnected_graph_zero_connectivity`: Disconnected graph $\lambda_2 = 0.0$.
   - `test_f09_monotonic_decrease_with_weak_bridge`: Weakening bridge decreases $\lambda_2$.
10. **Module 10: `feature_10_reusable_workspace`** (5 tests):
    - `test_f10_with_capacity_allocation_bounds`: Buffer capacities match requested sizes.
    - `test_f10_reset_for_nodes_clearing`: Scratch vectors and bitsets reset cleanly.
    - `test_f10_prune_and_prune_with_workspace_exact_parity`: Identical `PrunerResolution` results.
    - `test_f10_workspace_capacity_growth_resilience`: Growing graph sizes reallocate smoothly.
    - `test_f10_workspace_zero_alloc_streaming_throughput`: Multi-iteration loop maintains zero allocations.
11. **Module 11: `feature_11_injected_tau_bisection`** (5 tests):
    - `test_f11_default_tau_zero_split`: Split at $v_i \le 0.0$ vs $v_i > 0.0$.
    - `test_f11_negative_tau_sweep`: Split at $\tau = -0.5, -1.0$.
    - `test_f11_positive_tau_sweep`: Split at $\tau = +0.5, +1.0$.
    - `test_f11_volume_based_mainland_island_assignment`: Ensures $|mainland| \ge |island|$.
    - `test_f11_exact_boundary_value_tie`: $v_i == \tau$ assigned to small partition.
12. **Module 12: `feature_12_scale_invariant_density_ratio`** (5 tests):
    - `test_f12_dense_island_high_threat`: Ratio $> threat\_threshold \implies FatalBlock$.
    - `test_f12_sparse_island_low_threat`: Ratio $\le threat\_threshold \implies GarbageCollect$.
    - `test_f12_scale_invariance_proportional_growth`: Scaling both island and system preserves ratio.
    - `test_f12_zero_to_system_infinite_ratio`: `to_system == 0` yields `INFINITY` $\implies FatalBlock$.
    - `test_f12_empty_island_zero_ratio`: Empty island local nodes yields ratio $0.0$.
13. **Module 13: `feature_13_instruction_neglect`** (5 tests):
    - `test_f13_zero_system_edges_neglect`: $\text{to\_system} = 0 \implies \text{FatalBlock}$.
    - `test_f13_sub_threshold_connection_neglect`: $\text{to\_system}/N_{island} = 0.05 < 0.1 \implies \text{FatalBlock}$.
    - `test_f13_exact_threshold_connection`: $\text{to\_system}/N_{island} = 0.1 \implies$ evaluated by density ratio.
    - `test_f13_healthy_system_connection`: $\text{to\_system}/N_{island} = 1.0 \ge 0.1 \implies$ not blocked by neglect.
    - `test_f13_multi_node_cluster_neglect`: Multi-node island with weak system link.
14. **Module 14: `feature_14_single_token_tripwire`** (5 tests):
    - `test_f14_exact_tripwire_trigger_1_edge`: $N=1, internal=0, to\_system=1.0 \implies FatalBlock$.
    - `test_f14_tripwire_bypass_2_edges`: $to\_system=2.0 \ge 2.0 \implies$ tripwire does not trigger.
    - `test_f14_tripwire_bypass_internal_edges`: $internal > 0$ (not single node) $\implies$ not single-token.
    - `test_f14_tripwire_bypass_multi_node`: $N=2 \implies$ not single-token.
    - `test_f14_tripwire_with_varying_system_lengths`: Functions correctly across system lengths $1..100$.
15. **Module 15: `feature_15_policy_verdict_mapping`** (5 tests):
    - `test_f15_allow_on_zero_system_boundary`: $system\_boundary\_len = 0 \implies Allow$.
    - `test_f15_allow_on_empty_island`: $island\_nodes.is\_empty() \implies Allow$.
    - `test_f15_garbage_collect_on_benign_sub_threshold`: Non-malicious cluster $\implies GarbageCollect$.
    - `test_f15_fatal_block_on_high_density`: High density cluster $\implies FatalBlock$.
    - `test_f15_policy_action_display_formatting`: "ALLOW", "GARBAGE_COLLECT", "FATAL_BLOCK".
16. **Module 16: `feature_16_telemetry_separation`** (5 tests):
    - `test_f16_system_nodes_excluded_from_mainland_and_island`: Output vectors contain 0 system nodes.
    - `test_f16_system_nodes_participate_in_eigensolver`: Eigensolver uses system nodes during SpMV.
    - `test_f16_custom_system_start_idx`: System boundary $[start, len]$ properly respected.
    - `test_f16_inverted_system_range_no_stripping`: $start > len$ strips 0 nodes.
    - `test_f16_telemetry_separation_with_sinks`: Sinks within system range do not cause panics.
17. **Module 17: `feature_17_config_validation`** (5 tests):
    - `test_f17_builder_defaults`: $\tau=0.0, threat=2.0, max\_iter=10000, tol=1e-9, beta=0.5, start=5$.
    - `test_f17_invalid_tolerance_errors`: $\le 0.0$ or NaN returns `PrunerError::MathError`.
    - `test_f17_invalid_max_iterations_error`: $0$ returns `PrunerError::MathError`.
    - `test_f17_invalid_momentum_beta_errors`: $< 0.0, \ge 1.0$, NaN return `PrunerError::MathError`.
    - `test_f17_invalid_threat_threshold_errors`: $< 0.0$ or NaN return `PrunerError::MathError`.
18. **Module 18: `feature_18_invariant_baseline`** (5 tests):
    - `test_f18_baseline_nominal_flow`: Replicates `test_basic_nominal_flow`.
    - `test_f18_baseline_control_vector_override`: Replicates `test_control_vector_override`.
    - `test_f18_baseline_isolated_node_regression`: Replicates `test_isolated_node_tripwire_regression`.
    - `test_f18_baseline_custom_system_boundary`: Replicates `test_custom_system_boundary_framing`.
    - `test_f18_baseline_tiny_topology_with_sink`: Replicates `test_tiny_topology_with_sink`.
19. **Module 19: `feature_19_e2e_property_tests`** (5 tests):
    - `test_f19_partition_conservation_disjoint_union`: $V_{mainland} \uplus V_{island} = V_{active\_non\_system}$.
    - `test_f19_partition_disjointness`: $V_{mainland} \cap V_{island} = \emptyset$.
    - `test_f19_sink_non_inclusion`: Sinks never present in output partitions.
    - `test_f19_deterministic_repeated_execution`: 50 repeated runs yield bitwise identical output.
    - `test_f19_connectivity_score_non_negative`: $\lambda_2 \ge 0.0$ for all topologies.
20. **Module 20: `feature_20_fuzzing_adversarial`** (5 tests):
    - `test_f20_lcg_pseudo_random_generator`: Period, range uniformity, reproducibility.
    - `test_f20_random_graph_degree_conservation`: Degree counts match CSR edge counts.
    - `test_f20_csr_vs_adjacency_differential`: CSR neighbors match adjacency list.
    - `test_f20_random_sink_masking`: Sink filtering in CSR matches bitset mask.
    - `test_f20_random_topology_zero_panic`: 500 random topologies execute with 0 panics.
21. **Module 21: `feature_21_benchmark_throughput`** (5 tests):
    - `test_f21_small_topology_latency`: $N=10$ latency $< 50\mu s$.
    - `test_f21_medium_topology_throughput`: $N=100$ throughput $> 5,000$ graphs/sec.
    - `test_f21_dense_clique_scaling`: $K_{50}$ convergence.
    - `test_f21_star_graph_scaling`: Star 100 convergence.
    - `test_f21_zero_allocation_streaming_workspace`: Streaming workspace allocation verification.

---

### 2.3 Blueprint: `tests/e2e_tier2_boundaries.rs`
**Objective**: Comprehensive boundary, numerical limit, and extreme topological testing.

**Test Matrix**:
1. **Empty & Minimal Graphs**:
   - `test_boundary_n0_various_system_boundaries`: $N=0$ with $system\_boundary\_len \in [0, 1, 5]$.
   - `test_boundary_n1_isolated_and_sink`: $N=1$ as active vs as sink.
   - `test_boundary_n2_bridge_sink_and_system`: $N=2$ with/without edge, node 0 system, node 1 sink.
2. **Disconnected & Isolated Extremes**:
   - `test_boundary_n100_all_isolated_nodes`: 100 isolated nodes, degree=0.
   - `test_boundary_half_isolated_half_clique`: 50 nodes in clique, 50 isolated nodes.
3. **Extreme Degree Topologies**:
   - `test_boundary_massive_star_graph_n1000`: Central hub degree 999, 999 leaves degree 1.
   - `test_boundary_double_star_graph`: Two central hubs connected by bridge.
4. **Dense Cliques**:
   - `test_boundary_dense_clique_k100`: Fully connected 100-node graph.
   - `test_boundary_dense_clique_k200`: Fully connected 200-node graph.
   - `test_boundary_dense_clique_k300`: Fully connected 300-node graph.
5. **Linear & Cyclic Paths**:
   - `test_boundary_long_path_graph_n200`: Path graph $0-1-2-...-199$.
   - `test_boundary_large_cycle_graph_n200`: Ring graph $0-1-...-199-0$.
6. **Barbell & Bottleneck Graphs**:
   - `test_boundary_barbell_k50_bridge_k50`: Two $K_{50}$ cliques joined by 1 bridge.
   - `test_boundary_barbell_long_path_bridge`: Two $K_{20}$ cliques joined by path of 50 nodes.
7. **System Boundary Extremes**:
   - `test_boundary_system_start_idx_zero`: $system\_start\_idx = 0$, all nodes $[0..len]$ system.
   - `test_boundary_system_boundary_len_equals_n`: Entire graph is system domain.
   - `test_boundary_system_boundary_len_greater_than_n`: Boundary exceeds $N$.
   - `test_boundary_inverted_system_start_greater_than_len`: $start = 10, len = 5$.
8. **Sink Distribution Extremes**:
   - `test_boundary_all_nodes_are_sinks`: All $N$ nodes marked as sinks $\implies$ empty active set.
   - `test_boundary_alternating_sinks`: Even nodes active, odd nodes sinks.
9. **Numerical & Floating Point Limits**:
   - `test_boundary_extreme_tolerances`: $1e-15, 1e-12, 1e-6, 1e-2$.
   - `test_boundary_extreme_max_iterations`: $1, 2, 5, 100_000$.
   - `test_boundary_extreme_momentum_beta`: $0.0, 0.0001, 0.9999$.
   - `test_boundary_extreme_tau_values`: $-1e6, -100.0, 0.0, +100.0, +1e6$.

---

### 2.4 Blueprint: `tests/e2e_tier3_combinatorial.rs`
**Objective**: Pairwise and combinatorial feature interactions under stressful configurations.

**Test Matrix**:
1. **Workspace + Sinks + Isolated Nodes + System Boundaries**:
   - `test_combo_workspace_streaming_varying_sink_and_system_masks`: Reusing a single `PrunerWorkspace` across 500 iterations where graph size, sink set, and system boundary fluctuate dynamically.
2. **Custom Tau + Single-Token Tripwire + Telemetry Separation**:
   - `test_combo_custom_tau_with_single_token_tripwire`: Verifies that single-token tripwire triggers reliably across $\tau \in [-1.0, 0.0, 1.0]$ when the isolated token lands on the island side.
3. **Scale-Invariant Density Ratio + Variable Momentum + Tolerances**:
   - `test_combo_density_ratio_under_momentum_and_tolerance_variations`: Ensures threat verdict is invariant to eigensolver convergence settings ($\beta \in [0.0, 0.9]$, tolerance $\in [1e-4, 1e-9]$).
4. **Instruction Neglect + Dense Backdoor Islands + Sinks on Bridge**:
   - `test_combo_dense_backdoor_with_sink_severed_bridge`: Sinks placed on bridge edges isolate a backdoor island, triggering instruction neglect.
5. **Shifting System Boundaries across Heterogeneous Topologies**:
   - `test_combo_sliding_system_window_on_barbell`: Moving $[system\_start\_idx, system\_boundary\_len]$ from left clique, across bridge, to right clique.
6. **Multi-Tenant Concurrent Streaming Simulation**:
   - `test_combo_multi_tenant_workspace_isolation`: Multiple threads/workspaces processing disjoint topologies with zero cross-talk.

---

### 2.5 Blueprint: `tests/e2e_tier4_applications.rs`
**Objective**: High-level real-world domain audit scenarios based on spectral graph theory.

**Scenarios**:
1. **Streaming LLM Attention Steering / Jailbreak Guard**:
   - `test_app_llm_single_token_steering_jailbreak_blocked`: System prompt tokens at indices $[10..15]$. User prompt $[0..9]$. An adversarial injection token (node 16) has 1 weak attention link to system prompt node 10. Triggered by Arrington Single-Token Tripwire $\implies FatalBlock$.
   - `test_app_llm_dense_subversive_jailbreak_cluster_blocked`: User injection contains a 5-token dense jailbreak payload with 0 attention to system prompt instructions. Triggered by Instruction Neglect $\implies FatalBlock$.
   - `test_app_llm_benign_user_prompt_allowed`: Benign user prompt with healthy bidirectional attention to system instructions $\implies Allow$ / $GarbageCollect$.
2. **ZK-SNARK R1CS Constraint Backdoor Audit**:
   - `test_app_zk_snark_isolated_constraint_backdoor_blocked`: Circuit public inputs in system space $[20..25]$. Core arithmetic constraints in mainland $[0..19]$. Malicious private constraint loop (nodes 26..30) forms a dense backdoor cluster with no public input ties. Triggered by Scale-Invariant Density Ratio $\implies FatalBlock$.
   - `test_app_zk_snark_sound_circuit_allowed`: Sound circuit with uniform constraint propagation to public inputs $\implies Allow$.
3. **DeFi Mempool MEV Sandwich & Arbitrage Loop Audit**:
   - `test_app_defi_mev_sandwich_attack_bundle_blocked`: Liquidity pools & block builder anchors at system indices $[30..35]$. User swap txs in mainland $[0..29]$. Front-run and back-run bots create a tight 4-node sandwich cycle around victim tx with minimal pool anchor connection $\implies FatalBlock$.
   - `test_app_defi_benign_multi_hop_arbitrage_allowed`: Benign DEX aggregator routing across authorized liquidity pools $\implies Allow$ / $GarbageCollect$.
4. **ICS / OT Industrial Control System Network Segmentation Audit**:
   - `test_app_ics_ot_air_gapped_controller_compromise_blocked`: SCADA master & safety PLC at system indices $[12..15]$. Field sensors in mainland $[0..11]$. Rogue PLC subnet (nodes 16..19) completely severed from SCADA $\implies FatalBlock$ via Instruction Neglect.
   - `test_app_ics_ot_compliant_segmented_substation_allowed`: Authorized substation network with supervised telemetry links $\implies Allow$.
5. **Microservice Supply Chain Transitive Dependency Ring Audit**:
   - `test_app_supply_chain_transitive_dependency_backdoor_blocked`: Framework root & auth services in system space $[10..12]$. Core business services in mainland $[0..9]$. Malicious third-party package ring (nodes 13..17) with dense cyclic calls and 0 framework auth links $\implies FatalBlock$.
   - `test_app_supply_chain_benign_tree_allowed`: Clean tree-structured dependency graph $\implies Allow$.

---

### 2.6 Blueprint: `tests/fuzz_adversarial.rs`
**Objective**: High-throughput property-based adversarial fuzzing harness testing 10,000+ random configurations with pure-Rust deterministic PRNG.

**Architecture**:
- **Deterministic PRNG**: 64-bit Linear Congruential Generator (LCG) with seed `0xDEADBEEFCAFE1337`.
- **Fuzzing Parameters**:
  - Topologies: 10,000 randomized graphs.
  - Node count: $N \in [0, 200]$.
  - Edge count: $E \in [0, 2000]$.
  - Sink probability: $p \in [0.0, 0.4]$.
  - System boundary length: $len \in [0, N + 10]$.
  - System start index: $start \in [0, N + 5]$.
  - $\tau \in [-2.0, +2.0]$.
  - Momentum $\beta \in [0.0, 0.95]$.
- **Invariants Checked on Every Iteration**:
  1. **Partition Conservation**: $V_{mainland} \uplus V_{island} = V_{active\_non\_system}$.
  2. **Disjointness**: $V_{mainland} \cap V_{island} = \emptyset$.
  3. **No Sinks in Output**: $\forall s \in Sinks, s \notin V_{mainland} \land s \notin V_{island}$.
  4. **No System Nodes in Output**: $\forall i \in [system\_start\_idx, system\_boundary\_len], i \notin V_{mainland} \land i \notin V_{island}$.
  5. **Score Bound**: $\lambda_2 \ge 0.0 \land !\lambda_2.\text{is\_nan}()$.
  6. **Zero Allocation Parity**: `prune()` and `prune_with_workspace()` produce bitwise identical `PrunerResolution`.
  7. **Zero Panics**: No unexpected unwrap failures or index out of bounds.

---

### 2.7 Blueprint: `examples/benchmark_suite.rs` Uplift
**Objective**: Comprehensive, high-resolution comparative benchmark suite demonstrating throughput, latency percentiles, and zero-allocation streaming execution.

**Enhancements**:
1. **Benchmark Topologies**:
   - Small topologies ($N=10, 25$)
   - Medium topologies ($N=100, 250$)
   - Large topologies ($N=500, 1000$)
   - Sparse random / Barbell / Dense Clique topologies
2. **Metrics Measured**:
   - Microsecond Latency: Min, Mean, Median (P50), P95, P99, Max.
   - Throughput: Graphs evaluated per second (FPS/GPS).
   - Speedup: Zero-allocation `prune_with_workspace` vs allocating `prune`.
3. **Streaming Continuous Stress Run**:
   - 10,000 continuous evaluations through a single `PrunerWorkspace` to demonstrate steady-state memory behavior and sustained throughput.
4. **Zero External Dependencies**: Pure Rust `std::time::Instant`, clean ANSI color output formatting.

---

### 2.8 Blueprint: `TEST_READY.md`
**Objective**: Formal sign-off and verification manual documenting test coverage, test hierarchy, execution commands, and verification criteria.

---

## 3. Caveats

1. **Zero-Dependency Constraint**: Standard Rust testing crates (`criterion`, `proptest`, `quickcheck`, `rand`) cannot be added to `Cargo.toml`. All randomized property tests and fuzzers must use deterministic internal PRNG implementations (e.g. LCG / Xorshift in <20 lines).
2. **Fast-Path Edge Masking**: For $N < 3$ or all-isolated nodes ($\max(d)=0$), the engine uses early returns. E2E tests must test both fast-path and full eigensolver paths to prevent regressions in eigensolver mechanics.
3. **Platform Timer Resolution**: Microsecond measurements in benchmarks depend on the operating system clock resolution (`std::time::Instant`). Sufficient warmup iterations (>= 100) and sample sizes (>= 50 runs) ensure low-jitter statistical measurements.
4. **No Caveats on Implementation Completeness**: The blueprints cover all 21 features from `PROJECT.md` and satisfy all invariant and constraint rules from `AGENTS.md`.

---

## 4. Conclusion

The comprehensive plan and exact blueprints for Milestone 4 (Testing, Benchmarking & Fuzzing) are complete, rigorous, and verified against the existing Rust codebase:
- **`tests/e2e_tier1_features.rs`**: 21 feature modules, >= 5 test cases each (105+ tests).
- **`tests/e2e_tier2_boundaries.rs`**: 20+ extreme boundary and numerical limit tests.
- **`tests/e2e_tier3_combinatorial.rs`**: 6+ deep multi-feature interaction tests.
- **`tests/e2e_tier4_applications.rs`**: 5 real-world domain audit scenarios (LLM, ZK, DeFi, ICS, Supply Chain).
- **`tests/fuzz_adversarial.rs`**: 10,000+ iteration deterministic adversarial fuzzer with partition conservation invariants.
- **`examples/benchmark_suite.rs`**: High-resolution latency percentile & streaming throughput benchmark.
- **`TEST_READY.md`**: Complete verification guide and test execution roadmap.

All proposed test blueprints strictly honor the zero-dependency footprint and preserve all mathematical invariants defined in `AGENTS.md`.

---

## 5. Verification Method

To verify the test suite, run:
```bash
# 1. Run all unit and integration test suites
cargo test --all-targets

# 2. Run specific E2E test tiers
cargo test --test e2e_tier1_features
cargo test --test e2e_tier2_boundaries
cargo test --test e2e_tier3_combinatorial
cargo test --test e2e_tier4_applications
cargo test --test fuzz_adversarial

# 3. Run high-resolution release benchmark suite
cargo run --release --example benchmark_suite

# 4. Strict lint and warning check
cargo clippy --all-targets -- -D warnings
```

