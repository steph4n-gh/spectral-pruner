# 🔬 Spectral-Pruner Formal Specification & Invariant Catalog

**Document Path**: `/Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_spec_miner_survey_1/handoff.md`  
**Author**: Specification Mining Specialist (`teamwork_preview_spec_miner_survey_1`)  
**Target Repository**: `spectral-pruner` v1.0.0 (Rust)  
**Date**: 2026-08-27  

---

## Executive Summary

This document establishes the formal mathematical specification, invariant catalog, and zero-assumption constraints for the `spectral-pruner` library based on exhaustive mining of `AGENTS.md`, `DEVELOPMENT.md`, `README.md`, `src/lib.rs`, `src/engine.rs`, `src/error.rs`, and pre-bundled test suites/examples. 

All formulas, threshold boundaries, algorithmic phases, code mappings, and edge-case guarantees are cataloged herein for the project team and downstream implementation/fuzzing agents.

---

## Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Topology Model | `Topology` Graph Representation | 0-indexed unweighted/undirected causal relational graph with node count, edge list, and sink set | `num_nodes: usize`, `add_edge(u, v)`, `add_sink(idx)` | `Topology` struct | Out-of-bounds nodes silently ignored in `add_edge` and `add_sink` | `src/engine.rs:7-33` |
| 2 | Solver Engine | Heavy-Ball Shifted Laplacian Power Iteration | Power iteration over shifted Laplacian $M = I - \alpha L$ with momentum $\beta$ and null-space projection | `Topology`, `max_iterations`, `tolerance`, `momentum_beta` | Fiedler eigenvector $v_{\text{vec}}$, Fiedler eigenvalue $\lambda_2$ (`fiedler_value`) | If norm $< 10^{-15}$, terminates early without error | `src/engine.rs:173-276` |
| 3 | Regularization | Zero-Degree Clamping (Arrington Clamping) | Assigns disconnected active nodes (degree == 0) a static value ($1.0$) instead of $\sin(i)$ | Active nodes with degree 0 | $v_{\text{vec}}[i] = 1.0 / \|v_{\text{vec}}\|_2$ | Disconnected nodes stably guided into mainland partition | `src/engine.rs:180-198`, `AGENTS.md:25-28` |
| 4 | Partitioning | Injected $\tau$-Boundary Tie-Breaking | Rigid threshold bisection partitioning ($v_i \le \tau$ vs $v_i > \tau$) with volume-based mainland/island assignment | $v_{\text{vec}}$, injected $\tau \in \mathbb{R}$ | `mainland`, `island` partitions | Deterministic split; no median-cut or gap heuristics permitted | `src/engine.rs:278-298`, `AGENTS.md:21-23` |
| 5 | Threat Metric | Scale-Invariant Cluster Density Ratio | Normalizes island internal edges against system length and island-to-system connections | `internal`, `to_system`, $N_{\text{island}}$, $N_{\text{system}}$ | `normalized_ratio: f64` | If $\text{to\_system} == 0$ and $N_{\text{island}} > 0$, ratio is $\infty$ | `src/engine.rs:336-345`, `AGENTS.md:29-33` |
| 6 | Threat Metric | Instruction Neglect Thresholding | Evaluates island connection density to system boundary instruction space | $\text{to\_system}$, $N_{\text{island}}$ | `instruction_neglect: f64` | Triggers `FatalBlock` if $\text{neglect} < 0.1$ | `src/engine.rs:347-352, 371`, `AGENTS.md:34-38` |
| 7 | Tripwire Override | Micro-Steering Single-Token Tripwire | Instant quarantine override for single-node isolated clusters linking weakly to system space | $N_{\text{island}} == 1$, $\text{internal} == 0$, $0 < \text{to\_system} < 2$ | `is_control_vector: bool` | Triggers immediate `FatalBlock` override | `src/engine.rs:354-358, 372`, `AGENTS.md:39-42` |
| 8 | Policy Engine | Policy Verdict Evaluation | Evaluates threat indicators to determine action policy | Threat metrics, thresholds, system boundary length | `PolicyAction` (`Allow`, `GarbageCollect`, `FatalBlock`) | Clean deterministic enum mapping | `src/engine.rs:368-377` |
| 9 | Boundary Filter | Telemetry vs. Output Separation | System boundary nodes remain in math computation but are filtered from final returned vectors | `mainland`, `island`, `system_start_idx`, `system_boundary_len` | `final_mainland`, `final_island` | Boundary nodes excluded only at final `PrunerResolution` construction | `src/engine.rs:382-398`, `AGENTS.md:50` |
| 10 | Configuration | `PrunerBuilder` Fluent Interface | Builder pattern for custom $\tau$, threat threshold, iterations, tolerance, momentum, and system index | Builder setter methods | Configured `TauSpectralPruner` | Default fallback values applied if unconfigured | `src/engine.rs:72-129` |
| 11 | Error Handling | `PrunerError` Enum | Custom zero-dependency error types implementing `std::error::Error` | String messages | `PrunerError::MathError`, `PrunerError::MalformedTopology` | Formatted cleanly without panics | `src/error.rs:1-24` |

---

## Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | Graph Cardinality | Small graphs ($N < 3$) | Immediately returns `Ok(PrunerResolution { action: PolicyAction::Allow, mainland_nodes: non_sinks, island_nodes: [], connectivity_score: 0.0 })` without running power iteration. (`src/engine.rs:141-148`, `src/lib.rs:test_tiny_topology_with_sink`) |
| 2 | Edge Count | Zero-edge graph / All-isolated nodes ($\max(\text{degree}) == 0.0$) | Immediately returns `Ok(PrunerResolution { action: PolicyAction::Allow, mainland_nodes: non_sinks, island_nodes: [], connectivity_score: 0.0 })`. (`src/engine.rs:163-171`) |
| 3 | Disconnected Isolated Node | Single disconnected node ($d_i = 0$) in graph with other edges | Node clamped to $1.0$ during initialization, normalized, processed through null-space projection and power loop, and classified into mainland or island without skipping. (`src/lib.rs:test_isolated_node_tripwire_regression`) |
| 4 | Boundary Configuration | `system_boundary_len == 0` | If `system_boundary_len == 0` or local island nodes are empty, action is strictly `PolicyAction::Allow`. (`src/engine.rs:368-369`) |
| 5 | Completely Decoupled Island | Island with internal edges but zero connection to system space ($\text{to\_system} == 0.0$) | `normalized_ratio = f64::INFINITY` and `instruction_neglect = 0.0 < 0.1`, triggering `PolicyAction::FatalBlock`. (`src/engine.rs:342, 349, 370`) |
| 6 | Sinks in Graph | Nodes added as sinks via `topology.add_sink(i)` | Sinks are excluded from adjacency loops, degree accumulation, initialization, null-space projection, power steps ($v_m[\text{sink}] = 0.0$), Rayleigh quotients, and bisection vectors. (`src/engine.rs:155, 184, 210, 224, 258, 283`) |
| 7 | Zero Vector Convergence | Vector norm falls below $1e-15$ during power iteration | Hot loop breaks immediately (`break;`) avoiding division by zero. (`src/engine.rs:241-243`) |
| 8 | Self-Loops & Out of Bounds | Edges with $u == v$ or indices $\ge N$ | Out-of-bounds indices dropped at insertion (`Topology::add_edge`). Self-loops ($u == v$) skipped in adjacency compilation and threat metric loops. (`src/engine.rs:23, 155, 319`) |

---

## 🔬 Mathematical Invariants & Exact Formulations

### 1. Injected $\tau$-Boundary Tie-Breaking
- **Mathematical Specification**:
  Let $v \in \mathbb{R}^N$ be the converged Fiedler eigenvector. Partitioning is performed strictly by comparison against the externally injected scalar $\tau \in \mathbb{R}$:
  $$\text{side\_small} = \{ i \in V \setminus S \mid v_i \le \tau \}$$
  $$\text{side\_large} = \{ i \in V \setminus S \mid v_i > \tau \}$$
  $$\text{Mainland} = \begin{cases} \text{side\_small} & \text{if } |\text{side\_small}| > |\text{side\_large}| \\ \text{side\_large} & \text{otherwise} \end{cases}$$
  $$\text{Island} = \begin{cases} \text{side\_large} & \text{if } |\text{side\_small}| > |\text{side\_large}| \\ \text{side\_small} & \text{otherwise} \end{cases}$$
- **Prohibition**: Dynamic median sweeps, maximum spectral gap sweeps, and sign-only heuristics without $\tau$ offset are strictly prohibited.
- **Code Reference**: `src/engine.rs:278-298`.

### 2. Zero-Degree Clamping Regularization (Arrington Clamping)
- **Mathematical Specification**:
  Before power iteration, vector $v^{(0)}$ is initialized across all active non-sink nodes $i \in V \setminus S$:
  $$v^{(0)}_i = \begin{cases} 1.0 & \text{if } \text{degree}(i) == 0 \\ \sin(i) & \text{if } \text{degree}(i) > 0 \end{cases}$$
  $$v^{(0)} \leftarrow \frac{v^{(0)}}{\|v^{(0)}\|_2} \quad \left(\text{if } \|v^{(0)}\|_2 > 10^{-15}\right)$$
- **Theoretical Role**: Sub-dominant eigenvector regularization for 0-eigenvalue disconnected modes, preventing random noise from dictating classification and ensuring disconnected chaff reliably merges with the Mainland.
- **Code Reference**: `src/engine.rs:180-198`.

### 3. Scale-Invariant Cluster Density Ratio (Arrington's Scale-Invariant Semantic Density Ratio)
- **Mathematical Specification**:
  Let $\text{Island}_{\text{local}} = \{ i \in \text{Island} \mid i < \text{system\_start\_idx} \lor i > \text{system\_boundary\_len} \}$.
  Let $N_{\text{island}} = |\text{Island}_{\text{local}}|$ and $N_{\text{system}} = \text{system\_boundary\_len}$.
  Let $\text{Internal Edges} = |\{ (u, v) \in E \mid u \in \text{Island} \land v \in \text{Island} \land u, v \notin \text{System} \land u \neq v \}|$.
  Let $\text{System Edges} = |\{ (u, v) \in E \mid (u \in \text{Island} \land v \in \text{System}) \lor (v \in \text{Island} \land u \in \text{System}) \}|$.
  $$\text{Ratio} = \begin{cases} \frac{\text{Internal Edges} \times N_{\text{system}}}{\text{System Edges} \times N_{\text{island}}} & \text{if } \text{System Edges} > 0 \land N_{\text{island}} > 0 \\ \infty & \text{if } \text{System Edges} == 0 \land N_{\text{island}} > 0 \\ 0.0 & \text{otherwise} \end{cases}$$
- **Theoretical Role**: Normalizes $O(N^2)$ internal edge density against $O(N)$ linear boundary bridges, guaranteeing scale-invariant threat detection.
- **Code Reference**: `src/engine.rs:336-345`.

### 4. Instruction Neglect Thresholding
- **Mathematical Specification**:
  $$\text{Instruction Connection} = \begin{cases} \frac{\text{System Edges}}{N_{\text{island}}} & \text{if } N_{\text{island}} > 0 \\ 1.0 & \text{otherwise} \end{cases}$$
  $$\text{Trigger Condition}: \quad \text{Instruction Connection} < 0.1 \implies \text{PolicyAction::FatalBlock}$$
- **Theoretical Role**: Detects fully decoupled independent sets and sleeper clusters attempting to hide as silent background nodes.
- **Code Reference**: `src/engine.rs:347-352, 370-374`.

### 5. Micro-Steering Single-Token Tripwire (Arrington Single-Token Tripwire)
- **Mathematical Specification**:
  $$\text{is\_control\_vector} \iff (N_{\text{island}} == 1) \land (\text{Internal Edges} == 0) \land (0.0 < \text{System Edges} < 2.0)$$
  $$\text{Trigger Condition}: \quad \text{is\_control\_vector} == \text{true} \implies \text{PolicyAction::FatalBlock}$$
- **Theoretical Role**: Catches rank-1 perturbation attacks and microscopic single-point steering injections before they can trigger density thresholds.
- **Code Reference**: `src/engine.rs:354-358, 372`.

### 6. Power Iteration Solver Mechanics
- **Spectral Shift**: $\alpha = \frac{1}{2 \cdot \max(\text{degree}) + 1.1}$
- **Null-Space Projection**: $\mu = \frac{1}{|V \setminus S|} \sum_{i \in V \setminus S} v_i$, $v_i \leftarrow v_i - \mu$ for all $i \in V \setminus S$.
- **Shifted Laplacian Operator**: $M = I - \alpha L$. For active node $i$:
  $$v_{m}[i] = (1.0 - \alpha \cdot d_i) v_i + \alpha \sum_{j \in \text{adj}[i]} v_j$$
- **Heavy-Ball Acceleration**: $v^{(k+1)}_i = v_{m}[i] + \beta (v_{m}[i] - v_{\text{prev\_m}}[i])$, default $\beta = 0.5$.
- **Rayleigh Quotient (Algebraic Connectivity $\lambda_2$)**:
  $$\lambda_2 \approx v^T L v = \sum_{i \in V \setminus S} v_i \left( d_i v_i - \sum_{j \in \text{adj}[i]} v_j \right)$$
- **Code Reference**: `src/engine.rs:173-276`.

---

## 🚫 Hard Constraints (The Zero-Assumption Laws)

1. **Absolute Zero Dependencies**:
   - `Cargo.toml` must have 0 external dependencies (no `ndarray`, `nalgebra`, `petgraph`, `tokio`).
   - Verified via `cargo tree`: exactly 1 line output (`spectral-pruner v1.0.0`).
2. **Telemetry vs. Output Separation**:
   - Boundary nodes ($system\_start\_idx \le i \le system\_boundary\_len$) must participate fully in graph compilation, power iteration, Fiedler vector calculation, and threat metrics.
   - They must **only** be stripped from `final_mainland` and `final_island` at lines 384-391 before returning `Ok(PrunerResolution)`.
3. **Absolute Classification of Active Nodes**:
   - Every active non-sink node ($i \notin S$) must be classified into either `mainland` or `island` by the bisection comparison $v_i \le \tau$.
   - No pre-filtering or degree-based skipping is permitted.
4. **Preservation of Edge Cases**:
   - Small topologies ($N < 3$), unlinked graphs ($\max(d) == 0$), isolated chaff ($d_i == 0$), and system sink boundaries are intentional security mechanisms and must not be altered.

---

## 🗺️ Code & Test Mapping Matrix

| Invariant / Hard Constraint | Primary Source Location | Verification Tests | Showcase / Benchmark Examples |
|---|---|---|---|
| Injected $\tau$-Boundary Tie-Breaking | `src/engine.rs:278-298`, `src/lib.rs:14-17` | `test_basic_nominal_flow`, `test_control_vector_override`, `test_dense_clique_nominal` | `examples/benchmark_suite.rs`, `examples/llm_steerage_guard.rs` |
| Zero-Degree Clamping (Arrington Clamping) | `src/engine.rs:180-198`, `AGENTS.md:25-28` | `test_isolated_node_tripwire_regression` | `examples/llm_steerage_guard.rs` |
| Scale-Invariant Density Ratio | `src/engine.rs:336-345`, `AGENTS.md:29-33` | `test_basic_nominal_flow` | `examples/zk_circuit_backdoor.rs`, `examples/defi_mempool_mev.rs` |
| Instruction Neglect Thresholding | `src/engine.rs:347-352, 370-374` | `test_control_vector_override` | `examples/zk_circuit_backdoor.rs` |
| Micro-Steering Single-Token Tripwire | `src/engine.rs:354-358, 372` | `test_control_vector_override`, `test_custom_system_boundary_framing` | `examples/llm_steerage_guard.rs` |
| Telemetry vs. Output Separation | `src/engine.rs:382-398`, `AGENTS.md:50` | `test_custom_system_boundary_framing`, `test_tiny_topology_with_sink` | `examples/llm_steerage_guard.rs` |
| Absolute Node Classification | `src/engine.rs:282-298`, `AGENTS.md:51` | `test_isolated_node_tripwire_regression`, `test_large_star_topology` | `examples/benchmark_suite.rs` |
| Zero External Dependencies | `Cargo.toml:13-15`, `AGENTS.md:49` | `cargo tree`, `cargo test` | All 8 files in `examples/` |
| Zero Heap Allocation Hot-Loop | `src/engine.rs:200-276`, `DEVELOPMENT.md:211` | `cargo test`, `examples/benchmark_suite.rs` | `examples/benchmark_suite.rs` |

---

## 📋 5-Component Handoff Report

### 1. Observation
- **Direct Code Inspection**:
  - `Cargo.toml`: Lines 13-15 declare `[dependencies]` with no external crates.
  - `src/engine.rs`: Lines 1-401 implement `Topology`, `PrunerResolution`, `PolicyAction`, `TauSpectralPruner`, and `PrunerBuilder`.
  - `src/error.rs`: Lines 1-24 implement `PrunerError` with zero dependencies.
  - `src/lib.rs`: Lines 8-144 contain 7 unit tests covering nominal bisections, single-token tripwires, isolated node regressions, custom system boundary framing, tiny topologies, cliques, and star graphs.
  - `AGENTS.md`: Lines 1-53 define the 5 signature mathematical mechanics and 4 zero-assumption laws.
  - `DEVELOPMENT.md`: Lines 1-240 document the architecture, mathematical formulations, literature citations (Fiedler 1973, Pothen et al. 1990, Spielman & Teng 2007), and developer commands.
- **Tool Outputs**:
  - `cargo test`: 7 passed, 0 failed, 0 filtered out in 0.00s.
  - `cargo tree`: exactly `spectral-pruner v1.0.0 (/Volumes/Storage/bigworkspace/spectral-pruner)`.
  - `cargo clippy --all-targets -- -D warnings`: finished with 0 warnings/errors.
  - `cargo run --example benchmark_suite`: executed all benchmark topologies with zero hot-loop allocations.

### 2. Logic Chain
1. *Observation*: `AGENTS.md` and `DEVELOPMENT.md` specify 5 mathematical invariants and 4 hard architectural constraints.
2. *Verification*: Examination of `src/engine.rs` confirms every formula and rule is directly implemented in Rust:
   - Line 186 implements $v_i = 1.0$ for degree 0 (Arrington Clamping).
   - Line 286 implements $v_i \le \tau$ (Injected $\tau$-Boundary Tie-Breaking).
   - Line 340 implements $\frac{\text{internal} \times N_{\text{system}}}{\text{to\_system} \times N_{\text{island}}}$ (Scale-Invariant Density Ratio).
   - Line 349 implements $\frac{\text{to\_system}}{N_{\text{island}}} < 0.1$ (Instruction Neglect).
   - Line 357 implements $N_{\text{island}} == 1 \land \text{internal} == 0 \land 0 < \text{to\_system} < 2$ (Single-Token Tripwire).
   - Lines 384-391 implement telemetry filtering exclusively at the output stage.
3. *Verification*: Running `cargo test` confirms all 7 unit tests validate these specific invariant behaviors.
4. *Conclusion*: The formal specification is complete, authoritative, and fully grounded in executable code and test artifacts.

### 3. Caveats
- No caveats regarding specification mining: all mathematical definitions, thresholds, formulas, and constraints have been verified against source code, manifest documents, test suites, and examples.
- Downstream optimization subagents should note that the hot iteration loop must preserve the zero-allocation invariant (`v_prev_m.copy_from_slice(&v_m)` and `v_vec.copy_from_slice(&v_next)`).

### 4. Conclusion
The specification for `spectral-pruner` is mathematically defined, authoritative, and strictly constrained. Any future optimizations or refactorings by researcher or implementer agents must preserve:
1. Exact mathematical formulas for the 5 invariants.
2. The 4 Zero-Assumption Laws (zero dependencies, telemetry separation, absolute classification, edge-case preservation).
3. The zero-allocation hot-loop constraint.

### 5. Verification Method
To independently verify this specification:
1. Run `cargo test` — all 7 tests in `src/lib.rs` must pass.
2. Run `cargo tree` — must output exactly 1 crate with 0 dependencies.
3. Run `cargo clippy --all-targets -- -D warnings` — must compile with 0 warnings.
4. Run `cargo run --example benchmark_suite` — runs benchmark profiles across cliques, stars, and decoupled clusters.
5. Inspect `src/engine.rs` lines 180-398 to verify formula fidelity.
