# Milestone 3 Handoff Report: Security Metrics, Bisection & Policy Engine

**Author**: `teamwork_preview_explorer_m3_1` (Explorer Archetype)  
**Target Milestone**: Milestone 3 (`M3: Security Metrics, Bisection & Policy Engine`)  
**Repository**: `/Volumes/Storage/bigworkspace/spectral-pruner`  
**Date**: 2026-08-27  

---

## 1. Observation

### 1.1 Existing Architecture & File State
Direct observation of the repository codebase reveals:
1. `src/error.rs`:
   - Defines `PrunerError` with two variants: `MathError(String)` and `MalformedTopology(String)`.
   - Implements `fmt::Display`, `std::error::Error`, and `type Result<T> = std::result::Result<T, PrunerError>`.
2. `src/graph.rs`:
   - Implements contiguous `CsrGraph` and `BitSet`.
   - `CsrGraph::compile_into` zero-allocation workspace compilation operates in $O(N + E)$ time.
   - Sinks and out-of-bounds nodes are filtered out of CSR adjacency.
3. `src/engine.rs`:
   - `Topology`: Represents user inputs (`num_nodes`, `edges`, `sinks`).
   - `PrunerWorkspace`: Reusable scratchpad vector and bitset buffer (`v_vec`, `v_m`, `v_prev_m`, `v_next`, `sink_bits`, `island_bits`, `csr_row_ptrs`, `csr_col_indices`, `degrees`, `cursor`).
   - `TauSpectralPruner`: Eigensolver + policy engine.
   - `PrunerBuilder`: Fluent builder (`tau`, `threat_threshold`, `max_iterations`, `tolerance`, `momentum_beta`, `system_start_idx`).
   - `PolicyAction`: Enum with `Allow`, `GarbageCollect`, `FatalBlock`.
   - `PrunerResolution`: Struct with `action`, `mainland_nodes`, `island_nodes`, `connectivity_score`.

### 1.2 Identified Architectural & Mathematical Gaps in `src/engine.rs`

#### Observation A: Telemetry vs Output Separation Inconsistency in Fast Paths
- **Lines 268–278** ($N < 3$ fast path):
  ```rust
  if n < 3 {
      let mainland: Vec<usize> = (0..n)
          .filter(|&i| !workspace.sink_bits.contains(i))
          .collect();
      return Ok(PrunerResolution {
          action: PolicyAction::Allow,
          mainland_nodes: mainland,
          island_nodes: Vec::new(),
          connectivity_score: 0.0,
      });
  }
  ```
  *Gap*: When $N < 3$ and `system_boundary_len > 0`, system nodes within $[system\_start\_idx, system\_boundary\_len]$ are **not** filtered out of `mainland_nodes`.
- **Lines 291–301** ($\max(d) == 0.0$ all-disconnected fast path):
  ```rust
  let max_degree = workspace.degrees.iter().copied().fold(0.0, f64::max);
  if max_degree == 0.0 {
      let mainland: Vec<usize> = (0..n)
          .filter(|&i| !workspace.sink_bits.contains(i))
          .collect();
      return Ok(PrunerResolution {
          action: PolicyAction::Allow,
          mainland_nodes: mainland,
          island_nodes: Vec::new(),
          connectivity_score: 0.0,
      });
  }
  ```
  *Gap*: System nodes are similarly not stripped from `mainland_nodes`.
- **Lines 458–459 & 480 & 521 in nominal path**:
  ```rust
  let u_is_system = u >= self.system_start_idx && u <= system_boundary_len;
  ```
  *Gap*: If a caller sets `system_start_idx = 0` and `system_boundary_len = 0` (meaning system boundary is disabled), `0 >= 0 && 0 <= 0` evaluates to `true`, erroneously treating node 0 as a system boundary node. A `system_boundary_len > 0` guard is required.

#### Observation B: Input Validation & Error Handling
- `PrunerBuilder` currently lacks a fallible constructor `try_build(self) -> Result<TauSpectralPruner>`.
- `prune_with_workspace` lacks upfront mathematical validation for:
  - `tolerance <= 0.0 || tolerance.is_nan()`
  - `max_iterations == 0`
  - `momentum_beta < 0.0 || momentum_beta >= 1.0 || momentum_beta.is_nan()`
  - `threat_threshold < 0.0 || threat_threshold.is_nan()`
- `system_boundary_len > 0 && self.system_start_idx > system_boundary_len`: Needs well-defined handling to avoid inverted ranges.

#### Observation C: Test Suite Baseline & Randomized Test Interactions
- Command: `cargo test` executes 54 tests across unittests (25 tests), `empirical_challenge_m1` (16 tests), and `empirical_challenge_m2` (13 tests). All pass.
- In `tests/empirical_challenge_m2.rs` line 487:
  ```rust
  let sys_len = if n > 5 { rng.gen_range(0, n) } else { 0 };
  let res_direct = pruner.prune(&topo, sys_len).expect("Direct prune should not fail");
  ```
  When $n > 5$, `sys_len` can occasionally be generated in $1..system\_start\_idx$. If `system_start_idx > system_boundary_len` returns `Err` on `system_boundary_len > 0`, `.expect(...)` in existing M2 test line 492 would fail unless handled appropriately.

---

## 2. Logic Chain

### 2.1 Unified System Node Predicate (Telemetry Separation)
- **Premise 1 (`AGENTS.md` Invariant 2)**: System boundary anchor nodes $[system\_start\_idx, system\_boundary\_len]$ participate in all algebraic processing and threat metric calculations, but must be stripped from both `mainland_nodes` and `island_nodes` upon delivering `PrunerResolution`.
- **Premise 2**: A system boundary exists if and only if `system_boundary_len > 0` and `system_start_idx <= system_boundary_len`.
- **Deduction**: We define a single, uniform predicate:
  $$\text{is\_system\_node}(i) \iff (system\_boundary\_len > 0) \land (i \ge system\_start\_idx) \land (i \le system\_boundary\_len)$$
  This predicate must be applied consistently across:
  1. Small graph fast path ($N < 3$)
  2. All-disconnected fast path ($\max(d) == 0.0$)
  3. Metric calculations (`to_system`, `island_local_nodes`)
  4. Final partition pruning (`final_mainland`, `final_island`)

### 2.2 Injected $\tau$-Boundary Tie-Breaking & Volume Classification
- **Premise 1 (`AGENTS.md` Invariant 1)**: Partitioning must strictly evaluate:
  $$\text{SideSmall} = \{ i \mid v_i \le \tau \land i \notin \text{Sinks} \}$$
  $$\text{SideLarge} = \{ i \mid v_i > \tau \land i \notin \text{Sinks} \}$$
- **Premise 2**: Deterministic partition volume assignment designates the partition with strictly more active nodes as `mainland`, and the remaining partition as `island`:
  $$(\text{mainland}, \text{island}) = \begin{cases} (\text{SideSmall}, \text{SideLarge}) & \text{if } |\text{SideSmall}| > |\text{SideLarge}| \\ (\text{SideLarge}, \text{SideSmall}) & \text{otherwise} \end{cases}$$
- **Deduction**: All active nodes are classified into exactly one partition. Disconnected nodes ($d_i = 0$) initialized to $+1.0$ via Arrington Clamping are cleanly captured in one partition, preventing chaotic classification breaks.

### 2.3 Semantic Threat Metrics & Policy Decision Hierarchy
- **Metric 1: Scale-Invariant Semantic Density Ratio (`AGENTS.md` Invariant 3)**:
  $$\text{Ratio} = \begin{cases} \frac{\text{internal} \times N_{\text{system}}}{\text{to\_system} \times N_{\text{island}}} & \text{if } \text{to\_system} > 0 \land N_{\text{island}} > 0 \\ \infty & \text{if } \text{to\_system} == 0 \land N_{\text{island}} > 0 \\ 0.0 & \text{if } N_{\text{island}} == 0 \end{cases}$$
- **Metric 2: Instruction Neglect Thresholding (`AGENTS.md` Invariant 4)**:
  $$\text{neglect} = \begin{cases} \frac{\text{to\_system}}{N_{\text{island}}} & \text{if } N_{\text{island}} > 0 \\ 1.0 & \text{if } N_{\text{island}} == 0 \end{cases}$$
  Trigger condition: $\text{neglect} < 0.1 \implies \text{FatalBlock}$.
- **Metric 3: Micro-Steering Single-Token Tripwire (`AGENTS.md` Invariant 5)**:
  $$\text{tripwire} \iff (N_{\text{island}} == 1) \land (\text{internal} == 0) \land (0.0 < \text{to\_system} < 2.0)$$
  Trigger condition: $\text{tripwire} \implies \text{FatalBlock}$.
- **Policy Decision Resolution**:
  1. If $N_{\text{island}} == 0 \lor system\_boundary\_len == 0 \implies \mathbf{Allow}$.
  2. Else if $\text{Ratio} > threat\_threshold \lor \text{neglect} < 0.1 \lor \text{tripwire} \implies \mathbf{FatalBlock}$.
  3. Else $\implies \mathbf{GarbageCollect}$.

### 2.4 Input Validation Strategy & Backward Compatibility
- **Mathematical Invariant Validation**:
  - `tolerance <= 0.0 || tolerance.is_nan()` $\implies \text{Err(PrunerError::MathError("Tolerance must be strictly positive (> 0.0)"))}$
  - `max_iterations == 0` $\implies \text{Err(PrunerError::MathError("max_iterations must be greater than 0"))}$
  - `momentum_beta < 0.0 || momentum_beta >= 1.0 || momentum_beta.is_nan()` $\implies \text{Err(PrunerError::MathError("Momentum beta must be in [0.0, 1.0)"))}$
  - `threat_threshold < 0.0 || threat_threshold.is_nan()` $\implies \text{Err(PrunerError::MathError("threat_threshold must be non-negative (>= 0.0)"))}$
- **Topology Range Validation**:
  - If `system_boundary_len > 0 && self.system_start_idx > system_boundary_len`:
    In `is_system_node`, `i >= self.system_start_idx && i <= system_boundary_len` is naturally empty (0 system nodes), gracefully allowing streaming/fuzz workloads to proceed without panics while providing strict boundary checking if explicit validation is desired.
- **Fluent Builder Parity**:
  - `PrunerBuilder::try_build(self) -> Result<TauSpectralPruner>` provides fallible construction.
  - `PrunerBuilder::build(self) -> TauSpectralPruner` calls `self.try_build().expect("...")`, maintaining 100% backward compatibility for all existing tests and API consumers.

---

## 3. Caveats

1. **Undirected Edge Multiplicity**: In `Topology`, edges added via `add_edge(u, v)` are undirected pairs. The metric counting loop correctly accounts for connections between island nodes and system space.
2. **Sink Nodes in System Window**: If a sink node index falls within $[system\_start\_idx, system\_boundary\_len]$, it is already excluded from active calculation by `sink_bits` and must not be counted as a system boundary node.
3. **Zero Dependencies**: Implementation requires no external crates; standard library only.

---

## 4. Conclusion & Code Blueprints

### 4.1 Implementation Blueprint for `src/engine.rs`

```rust
//! Complete Milestone 3 engine implementation featuring:
//! - Tau-boundary bisection and volume classification
//! - Scale-Invariant Semantic Density Ratio
//! - Instruction Neglect Thresholding
//! - Micro-Steering Single-Token Tripwire
//! - Harmonized telemetry separation across nominal and all fast paths
//! - Comprehensive input validation and PrunerError propagation

use crate::error::{PrunerError, Result};
use crate::graph::{BitSet, CsrGraph};
use std::collections::BTreeSet;
use std::fmt;

/// Structural input layout representing an abstract network matrix.
#[derive(Debug, Clone)]
pub struct Topology {
    pub num_nodes: usize,
    pub edges: Vec<(usize, usize)>,
    pub sinks: BTreeSet<usize>,
}

impl Topology {
    pub fn new(num_nodes: usize) -> Self {
        Self {
            num_nodes,
            edges: Vec::new(),
            sinks: BTreeSet::new(),
        }
    }

    pub fn add_edge(&mut self, source: usize, target: usize) {
        if source < self.num_nodes && target < self.num_nodes {
            self.edges.push((source, target));
        }
    }

    pub fn add_sink(&mut self, node_index: usize) {
        if node_index < self.num_nodes {
            self.sinks.insert(node_index);
        }
    }

    /// Populates an existing `BitSet` with sink indices, resizing and clearing it in-place.
    #[inline]
    pub fn populate_sink_bitset(&self, sink_bits: &mut BitSet) {
        sink_bits.reset_with_len(self.num_nodes);
        for &sink in &self.sinks {
            sink_bits.insert(sink);
        }
    }

    /// Constructs a `BitSet` bitmask representation of the graph's sinks.
    #[inline]
    pub fn to_sink_bitset(&self) -> BitSet {
        let mut bitset = BitSet::new(self.num_nodes);
        self.populate_sink_bitset(&mut bitset);
        bitset
    }
}

/// Reusable memory scratchpad enabling zero heap allocations during high-frequency
/// or streaming spectral partitioning iterations.
#[derive(Debug, Clone)]
pub struct PrunerWorkspace {
    /// Working eigenvector state vector $v$
    pub v_vec: Vec<f64>,
    /// Intermediate power step $M v$
    pub v_m: Vec<f64>,
    /// Previous power step $M v_{k-1}$ for Heavy-Ball momentum acceleration
    pub v_prev_m: Vec<f64>,
    /// Next candidate eigenvector $v_{k+1}$ before normalization
    pub v_next: Vec<f64>,
    /// Bitset mask for active/sink nodes
    pub sink_bits: BitSet,
    /// Bitset mask for island partition nodes during threat metric evaluation
    pub island_bits: BitSet,
    /// CSR row pointer scratchpad
    pub csr_row_ptrs: Vec<usize>,
    /// CSR column indices scratchpad
    pub csr_col_indices: Vec<usize>,
    /// Node degrees scratchpad
    pub degrees: Vec<f64>,
    /// CSR compilation cursor scratchpad
    pub cursor: Vec<usize>,
}

impl PrunerWorkspace {
    /// Creates an empty workspace with no pre-allocated buffer capacities.
    pub fn new() -> Self {
        Self {
            v_vec: Vec::new(),
            v_m: Vec::new(),
            v_prev_m: Vec::new(),
            v_next: Vec::new>,
            sink_bits: BitSet::new(0),
            island_bits: BitSet::new(0),
            csr_row_ptrs: Vec::new(),
            csr_col_indices: Vec::new(),
            degrees: Vec::new(),
            cursor: Vec::new(),
        }
    }

    /// Creates a workspace with capacity pre-allocated for graphs with up to
    /// `num_nodes` and `estimated_edges` undirected edges.
    pub fn with_capacity(num_nodes: usize, estimated_edges: usize) -> Self {
        Self {
            v_vec: Vec::with_capacity(num_nodes),
            v_m: Vec::with_capacity(num_nodes),
            v_prev_m: Vec::with_capacity(num_nodes),
            v_next: Vec::with_capacity(num_nodes),
            sink_bits: BitSet::with_capacity(num_nodes),
            island_bits: BitSet::with_capacity(num_nodes),
            csr_row_ptrs: Vec::with_capacity(num_nodes + 1),
            csr_col_indices: Vec::with_capacity(estimated_edges * 2),
            degrees: Vec::with_capacity(num_nodes),
            cursor: Vec::with_capacity(num_nodes),
        }
    }

    /// Resets the scratch buffers for a graph of size `num_nodes` without deallocating capacity.
    pub fn reset_for_nodes(&mut self, num_nodes: usize) {
        self.sink_bits.reset_with_len(num_nodes);
        self.island_bits.reset_with_len(num_nodes);

        self.v_vec.clear();
        self.v_vec.resize(num_nodes, 0.0);

        self.v_m.clear();
        self.v_m.resize(num_nodes, 0.0);

        self.v_prev_m.clear();
        self.v_prev_m.resize(num_nodes, 0.0);

        self.v_next.clear();
        self.v_next.resize(num_nodes, 0.0);
    }
}

impl Default for PrunerWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

/// The final resolution payload generated by the mathematical evaluation pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct PrunerResolution {
    pub action: PolicyAction,
    pub mainland_nodes: Vec<usize>,
    pub island_nodes: Vec<usize>,
    pub connectivity_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAction {
    Allow,
    GarbageCollect,
    FatalBlock,
}

impl fmt::Display for PolicyAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val = match self {
            Self::Allow => "ALLOW",
            Self::GarbageCollect => "GARBAGE_COLLECT",
            Self::FatalBlock => "FATAL_BLOCK",
        };
        write!(f, "{}", val)
    }
}

pub struct TauSpectralPruner {
    tau: f64,
    threat_threshold: f64,
    max_iterations: usize,
    tolerance: f64,
    momentum_beta: f64,
    system_start_idx: usize,
}

impl TauSpectralPruner {
    #[inline]
    pub fn tau(&self) -> f64 {
        self.tau
    }

    #[inline]
    pub fn threat_threshold(&self) -> f64 {
        self.threat_threshold
    }

    #[inline]
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    #[inline]
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    #[inline]
    pub fn momentum_beta(&self) -> f64 {
        self.momentum_beta
    }

    #[inline]
    pub fn system_start_idx(&self) -> usize {
        self.system_start_idx
    }
}

/// Fluent builder pattern interface to cleanly manage engine instantiation.
pub struct PrunerBuilder {
    tau: f64,
    threat_threshold: f64,
    max_iterations: usize,
    tolerance: f64,
    momentum_beta: f64,
    system_start_idx: usize,
}

impl PrunerBuilder {
    pub fn tau(mut self, value: f64) -> Self {
        self.tau = value;
        self
    }

    pub fn threat_threshold(mut self, value: f64) -> Self {
        self.threat_threshold = value;
        self
    }

    pub fn max_iterations(mut self, value: usize) -> Self {
        self.max_iterations = value;
        self
    }

    pub fn tolerance(mut self, value: f64) -> Self {
        self.tolerance = value;
        self
    }

    pub fn momentum_beta(mut self, value: f64) -> Self {
        self.momentum_beta = value;
        self
    }

    pub fn system_start_idx(mut self, value: usize) -> Self {
        self.system_start_idx = value;
        self
    }

    /// Validates builder parameters and builds `TauSpectralPruner`.
    pub fn try_build(self) -> Result<TauSpectralPruner> {
        if self.tolerance <= 0.0 || self.tolerance.is_nan() {
            return Err(PrunerError::MathError(format!(
                "Tolerance must be strictly positive (> 0.0), got {}",
                self.tolerance
            )));
        }
        if self.max_iterations == 0 {
            return Err(PrunerError::MathError(
                "max_iterations must be greater than 0".to_string(),
            ));
        }
        if self.momentum_beta < 0.0 || self.momentum_beta >= 1.0 || self.momentum_beta.is_nan() {
            return Err(PrunerError::MathError(format!(
                "Momentum beta must be in [0.0, 1.0), got {}",
                self.momentum_beta
            )));
        }
        if self.threat_threshold < 0.0 || self.threat_threshold.is_nan() {
            return Err(PrunerError::MathError(format!(
                "threat_threshold must be non-negative (>= 0.0), got {}",
                self.threat_threshold
            )));
        }

        Ok(TauSpectralPruner {
            tau: self.tau,
            threat_threshold: self.threat_threshold,
            max_iterations: self.max_iterations,
            tolerance: self.tolerance,
            momentum_beta: self.momentum_beta,
            system_start_idx: self.system_start_idx,
        })
    }

    /// Fluent terminal builder constructor. Panics on invalid parameter configuration.
    pub fn build(self) -> TauSpectralPruner {
        match self.try_build() {
            Ok(pruner) => pruner,
            Err(err) => panic!("Invalid PrunerBuilder configuration: {}", err),
        }
    }
}

impl TauSpectralPruner {
    pub fn builder() -> PrunerBuilder {
        PrunerBuilder {
            tau: 0.0,
            threat_threshold: 2.0,
            max_iterations: 10_000,
            tolerance: 1e-9,
            momentum_beta: 0.5,
            system_start_idx: 5,
        }
    }

    /// Computes a polynomial-time spectral bisection heuristic approximation of the network topology
    /// via the Fiedler vector to determine containment policy.
    pub fn prune(
        &self,
        topology: &Topology,
        system_boundary_len: usize,
    ) -> Result<PrunerResolution> {
        let mut workspace =
            PrunerWorkspace::with_capacity(topology.num_nodes, topology.edges.len());
        self.prune_with_workspace(topology, system_boundary_len, &mut workspace)
    }

    /// Computes a polynomial-time spectral bisection heuristic approximation using a pre-allocated
    /// scratchpad workspace, achieving zero heap allocations across repeated evaluations.
    #[allow(clippy::needless_range_loop)]
    pub fn prune_with_workspace(
        &self,
        topology: &Topology,
        system_boundary_len: usize,
        workspace: &mut PrunerWorkspace,
    ) -> Result<PrunerResolution> {
        // Upfront engine configuration validation
        if self.tolerance <= 0.0 || self.tolerance.is_nan() {
            return Err(PrunerError::MathError(format!(
                "Tolerance must be strictly positive (> 0.0), got {}",
                self.tolerance
            )));
        }
        if self.max_iterations == 0 {
            return Err(PrunerError::MathError(
                "max_iterations must be greater than 0".to_string(),
            ));
        }
        if self.momentum_beta < 0.0 || self.momentum_beta >= 1.0 || self.momentum_beta.is_nan() {
            return Err(PrunerError::MathError(format!(
                "Momentum beta must be in [0.0, 1.0), got {}",
                self.momentum_beta
            )));
        }
        if self.threat_threshold < 0.0 || self.threat_threshold.is_nan() {
            return Err(PrunerError::MathError(format!(
                "threat_threshold must be non-negative (>= 0.0), got {}",
                self.threat_threshold
            )));
        }

        let n = topology.num_nodes;

        // Reset workspace scratchpad for N nodes
        workspace.reset_for_nodes(n);
        topology.populate_sink_bitset(&mut workspace.sink_bits);

        // System node predicate: active only when system_boundary_len > 0 and in [system_start_idx, system_boundary_len]
        let is_system_node = |i: usize| -> bool {
            system_boundary_len > 0 && i >= self.system_start_idx && i <= system_boundary_len
        };

        // Edge-case 1: Small graphs (N < 3) cannot be meaningfully bisected
        if n < 3 {
            let mainland: Vec<usize> = (0..n)
                .filter(|&i| !workspace.sink_bits.contains(i) && !is_system_node(i))
                .collect();
            return Ok(PrunerResolution {
                action: PolicyAction::Allow,
                mainland_nodes: mainland,
                island_nodes: Vec::new(),
                connectivity_score: 0.0,
            });
        }

        // 1. Zero-Allocation CSR compilation into workspace
        CsrGraph::compile_into(
            topology,
            &workspace.sink_bits,
            &mut workspace.csr_row_ptrs,
            &mut workspace.csr_col_indices,
            &mut workspace.degrees,
            &mut workspace.cursor,
        );

        // Edge-case 2: All disconnected / zero-degree active nodes
        let max_degree = workspace.degrees.iter().copied().fold(0.0, f64::max);
        if max_degree == 0.0 {
            let mainland: Vec<usize> = (0..n)
                .filter(|&i| !workspace.sink_bits.contains(i) && !is_system_node(i))
                .collect();
            return Ok(PrunerResolution {
                action: PolicyAction::Allow,
                mainland_nodes: mainland,
                island_nodes: Vec::new(),
                connectivity_score: 0.0,
            });
        }

        // 2. Accelerated Shifted Laplacian Eigensolver Operator: M = I - alpha * L
        let alpha = 1.0 / (2.0 * max_degree + 1.1);

        // Symmetry-breaking initialization with Arrington Clamping:
        // Isolated degree-0 nodes are clamped to 1.0; active connected nodes to sin(i)
        for i in 0..n {
            if !workspace.sink_bits.contains(i) {
                if workspace.degrees[i] == 0.0 {
                    workspace.v_vec[i] = 1.0; // Zero-Degree Clamping Regularization
                } else {
                    workspace.v_vec[i] = (i as f64).sin();
                }
            } else {
                workspace.v_vec[i] = 0.0;
            }
        }

        let init_norm: f64 = workspace.v_vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        if init_norm > 1e-15 {
            let inv_norm = 1.0 / init_norm;
            for x in &mut workspace.v_vec {
                *x *= inv_norm;
            }
        }

        workspace.v_prev_m.copy_from_slice(&workspace.v_vec);
        let mut fiedler_value = 0.0;

        for _ in 0..self.max_iterations {
            // Null-space projection: orthogonalize against constant vector 1 across active non-sink nodes
            let mut sum = 0.0;
            let mut count = 0.0;
            for i in 0..n {
                if !workspace.sink_bits.contains(i) {
                    sum += workspace.v_vec[i];
                    count += 1.0;
                }
            }
            let mean = if count > 0.0 { sum / count } else { 0.0 };
            for i in 0..n {
                if !workspace.sink_bits.contains(i) {
                    workspace.v_vec[i] -= mean;
                } else {
                    workspace.v_vec[i] = 0.0;
                }
            }

            // Power Step evaluation: M = I - alpha * L via CSR SpMV
            for i in 0..n {
                if workspace.sink_bits.contains(i) {
                    workspace.v_m[i] = 0.0;
                    continue;
                }
                let start = workspace.csr_row_ptrs[i];
                let end = workspace.csr_row_ptrs[i + 1];
                let mut neighbor_sum = 0.0;
                for &neighbor in &workspace.csr_col_indices[start..end] {
                    neighbor_sum += workspace.v_vec[neighbor];
                }
                workspace.v_m[i] = (1.0 - alpha * workspace.degrees[i]) * workspace.v_vec[i]
                    + alpha * neighbor_sum;
            }

            // Heavy-Ball / Polyak Momentum Injection
            for i in 0..n {
                if !workspace.sink_bits.contains(i) {
                    workspace.v_next[i] = workspace.v_m[i]
                        + self.momentum_beta * (workspace.v_m[i] - workspace.v_prev_m[i]);
                } else {
                    workspace.v_next[i] = 0.0;
                }
            }

            let norm_sq: f64 = workspace.v_next.iter().map(|x| x * x).sum();
            let norm = norm_sq.sqrt();
            if norm < 1e-15 {
                break;
            }

            let inv_norm = 1.0 / norm;
            let mut max_diff = 0.0f64;
            for i in 0..n {
                if !workspace.sink_bits.contains(i) {
                    workspace.v_next[i] *= inv_norm;
                    let diff = (workspace.v_next[i] - workspace.v_vec[i]).abs();
                    if diff > max_diff {
                        max_diff = diff;
                    }
                } else {
                    workspace.v_next[i] = 0.0;
                }
            }

            // Continuous Rayleigh Quotient calculation: lambda_2 = v^T L v
            let mut v_l_v = 0.0;
            for i in 0..n {
                if workspace.sink_bits.contains(i) {
                    continue;
                }
                let start = workspace.csr_row_ptrs[i];
                let end = workspace.csr_row_ptrs[i + 1];
                let mut neighbor_sum = 0.0;
                for &neighbor in &workspace.csr_col_indices[start..end] {
                    neighbor_sum += workspace.v_next[neighbor];
                }
                let row_sum = workspace.degrees[i] * workspace.v_next[i] - neighbor_sum;
                v_l_v += workspace.v_next[i] * row_sum;
            }
            fiedler_value = v_l_v;

            // In-place copy to avoid vector recreation and allocation thrashing
            workspace.v_prev_m.copy_from_slice(&workspace.v_m);
            workspace.v_vec.copy_from_slice(&workspace.v_next);

            if max_diff < self.tolerance {
                break;
            }
        }

        // 3. Absolute Injected Tau Bisection Classification
        let mut side_small = Vec::new();
        let mut side_large = Vec::new();

        for i in 0..n {
            if workspace.sink_bits.contains(i) {
                continue;
            }
            if workspace.v_vec[i] <= self.tau {
                side_small.push(i);
            } else {
                side_large.push(i);
            }
        }

        // Enforce deterministic partition volume classification
        let (mainland, island) = if side_small.len() > side_large.len() {
            (side_small, side_large)
        } else {
            (side_large, side_small)
        };

        // Populate island bitset for O(1) threat metric lookups
        workspace.island_bits.clear();
        for &node in &island {
            workspace.island_bits.insert(node);
        }

        // 4. Advanced Semantic Threat Metric Analysis Pipeline
        let mut to_system = 0.0;
        let mut internal = 0.0;

        for &(u, v) in &topology.edges {
            if u >= n || v >= n || u == v || workspace.sink_bits.contains(u) || workspace.sink_bits.contains(v) {
                continue;
            }
            let u_in_island = workspace.island_bits.contains(u);
            let v_in_island = workspace.island_bits.contains(v);
            let u_is_system = is_system_node(u);
            let v_is_system = is_system_node(v);

            if u_in_island {
                if v_is_system {
                    to_system += 1.0;
                } else if v_in_island && !u_is_system {
                    internal += 1.0;
                }
            }
            if v_in_island {
                if u_is_system {
                    to_system += 1.0;
                } else if u_in_island && !v_is_system {
                    internal += 1.0;
                }
            }
        }

        let island_local_nodes: Vec<usize> = island
            .iter()
            .copied()
            .filter(|&i| !is_system_node(i))
            .collect();
        let island_len = island_local_nodes.len() as f64;
        let system_len = system_boundary_len as f64;

        // Metric 1: Scale-Invariant Cluster Density Ratio
        let normalized_ratio = if to_system > 0.0 && island_len > 0.0 {
            (internal * system_len) / (to_system * island_len)
        } else if !island_local_nodes.is_empty() {
            f64::INFINITY
        } else {
            0.0
        };

        // Metric 2: Instruction neglect checking
        let instruction_neglect = if !island_local_nodes.is_empty() {
            to_system / island_len
        } else {
            1.0
        };

        // Metric 3: Micro-Steering Single-Token Tripwire
        let is_control_vector =
            island_len == 1.0 && internal == 0.0 && to_system > 0.0 && to_system < 2.0;

        // 5. Policy Enforcement Decision Processing
        let action = if island_local_nodes.is_empty() || system_boundary_len == 0 {
            PolicyAction::Allow
        } else if normalized_ratio > self.threat_threshold
            || instruction_neglect < 0.1
            || is_control_vector
        {
            PolicyAction::FatalBlock
        } else {
            PolicyAction::GarbageCollect
        };

        // Final Telemetry Separation: strip system boundary nodes from output partitions
        let final_mainland: Vec<usize> = mainland
            .into_iter()
            .filter(|&i| !is_system_node(i))
            .collect();
        let final_island: Vec<usize> = island
            .into_iter()
            .filter(|&i| !is_system_node(i))
            .collect();

        Ok(PrunerResolution {
            action,
            mainland_nodes: final_mainland,
            island_nodes: final_island,
            connectivity_score: fiedler_value,
        })
    }
}
```

---

## 5. Verification Method

### 5.1 Verification Commands
1. **Compilation Check**:
   ```bash
   cargo check --all-targets
   ```
2. **Standard & Baseline Tests**:
   ```bash
   cargo test --lib
   cargo test --test empirical_challenge_m1
   cargo test --test empirical_challenge_m2
   ```
3. **M3 Dedicated Test Suite**:
   ```bash
   cargo test --test empirical_challenge_m3
   ```
4. **All Tests**:
   ```bash
   cargo test
   ```

### 5.2 Invalidation Conditions
- Any existing test modified or regressed.
- System boundary nodes appearing in `mainland_nodes` or `island_nodes` across any fast-path or nominal output.
- Division-by-zero panics or unhandled `NaN` inputs during builder or prune execution.
- Introduction of any external dependency to `Cargo.toml`.
