# Project: spectral-pruner Uplift

## Architecture
The `spectral-pruner` crate is a zero-dependency, bare-metal Rust library using Spectral Graph Theory to audit network topologies, trace internal clusters, and isolate structural anomalies.

The architecture comprises:
1. **Graph Representation Layer (`src/graph.rs`)**:
   - `Topology`: User-facing graph builder (`num_nodes`, `edges`, `sinks`).
   - `CsrGraph`: High-performance, zero-dependency contiguous Compressed Sparse Row (CSR) representation with two flat vectors (`row_ptrs: Vec<usize>`, `col_indices: Vec<usize>`) and pre-calculated `degrees: Vec<f64>`, replacing fragmented `Vec<Vec<usize>>`.
   - `BitSet`: Flat `[u64]` bitmasks for $O(1)$ constant-time sink checking and partition classification, replacing heap-allocated `BTreeSet<usize>`.
2. **Spectral Eigensolver Layer (`src/engine.rs`)**:
   - **Regularization**: Arrington Clamping setting degree-0 active nodes to $+1.0$ at initialization.
   - **Continuous Shifted Laplacian Operator**: $M = I - \alpha L$ with $\alpha = \frac{1}{2 \cdot d_{\max} + 1.1}$.
   - **Continuous Null-Space Projection**: Orthogonalization against the null space $\mathbf{1}$ by centering active nodes ($\mathbf{v} \leftarrow \mathbf{v} - \text{mean}(\mathbf{v})$).
   - **Iteration Acceleration**: Memory-contiguous auto-vectorizable SpMV over CSR slices with Heavy-Ball momentum acceleration ($\beta = 0.5$).
   - **Rayleigh Quotient**: $v^T L v$ computation for algebraic connectivity ($\lambda_2$).
   - **PrunerWorkspace**: Reusable memory scratchpad enabling true zero-heap-allocation calls across streaming workloads.
3. **Partitioning & Threat Classification Layer (`src/engine.rs`)**:
   - **$\tau$-Boundary Tie-Breaking**: Rigid numerical split ($v_i \le \tau$ vs $v_i > \tau$) with volume-based mainland/island assignment.
   - **Scale-Invariant Semantic Density Ratio**: $\frac{\text{internal} \times N_{\text{system}}}{\text{to\_system} \times N_{\text{island}}}$.
   - **Instruction Neglect**: $\frac{\text{to\_system}}{N_{\text{island}}} < 0.1 \implies \text{FatalBlock}$.
   - **Micro-Steering Single-Token Tripwire**: $N_{\text{island}} == 1 \land \text{internal} == 0 \land 0 < \text{to\_system} < 2 \implies \text{FatalBlock}$.
   - **Telemetry vs Output Separation**: System boundary nodes $[system\_start\_idx, system\_boundary\_len]$ participate in all algebraic and metric computations and are stripped only at output resolution across all paths.
4. **Validation & Error Handling Layer (`src/error.rs`, `src/engine.rs`)**:
   - Validates boundary indices, non-empty active nodes, positive tolerances, non-zero iterations, momentum beta, and provides structured `PrunerError` variants.

---

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | `Topology` Graph Builder | User API for nodes, edges, and sinks | M1 (DONE) | `src/engine.rs` |
| 2 | Contiguous `CsrGraph` | Cache-friendly CSR 2-vector layout eliminating $N+1$ allocations | M1 (DONE) | 2026 Research |
| 3 | Fast `BitSet` Bitmasks | Flat bitmask for $O(1)$ sink & island queries replacing `BTreeSet` | M1 (DONE) | 2026 Research |
| 4 | Edge-Case Graph Handling | Small graph ($N < 3$) and all-disconnected ($\max(d)=0$) fast paths | M1 (DONE) | `AGENTS.md` |
| 5 | Arrington Clamping | $v_i = 1.0$ initialization for disconnected nodes ($d_i = 0$) | M2 (DONE) | `AGENTS.md` |
| 6 | Shifted Laplacian SpMV | Auto-vectorizable matrix-vector multiplication over CSR slices | M2 (DONE) | 2026 Research |
| 7 | Null-Space Projection | Active node centering against all-ones null-space vector | M2 (DONE) | `src/engine.rs` |
| 8 | Momentum Acceleration | Polyak / Heavy-Ball accelerated convergence | M2 (DONE) | 2026 Research |
| 9 | Rayleigh Quotient $\lambda_2$ | Algebraic connectivity computation | M2 (DONE) | `src/engine.rs` |
| 10 | Reusable `PrunerWorkspace` | Zero-allocation streaming execution API | M2 (DONE) | 2026 Research |
| 11 | Injected $\tau$-Boundary Split | Rigid $v_i \le \tau$ vs $v_i > \tau$ bisection | M3 (DONE) | `AGENTS.md` |
| 12 | Scale-Invariant Density Ratio | $\frac{\text{internal} \times N_{\text{system}}}{\text{to\_system} \times N_{\text{island}}}$ threat metric | M3 (DONE) | `AGENTS.md` |
| 13 | Instruction Neglect | $\frac{\text{to\_system}}{N_{\text{island}}} < 0.1 \implies \text{FatalBlock}$ | M3 (DONE) | `AGENTS.md` |
| 14 | Single-Token Tripwire | $N_{\text{island}}==1 \land \text{internal}==0 \land 0 < \text{to\_system} < 2$ tripwire | M3 (DONE) | `AGENTS.md` |
| 15 | Policy Verdict Mapping | `Allow`, `GarbageCollect`, `FatalBlock` resolution | M3 (DONE) | `src/engine.rs` |
| 16 | Telemetry Separation | Strips boundary nodes only at final `PrunerResolution` delivery | M3 (DONE) | `AGENTS.md` |
| 17 | Configuration & Validation | `PrunerBuilder` validation and `PrunerError` propagation | M3 (DONE) | `src/error.rs` |
| 18 | Invariant Baseline Tests | 7 existing unit tests pass unmodified | M4 (DONE) | `src/lib.rs` |
| 19 | E2E & Property Tests | Tiers 1-4 comprehensive test suite (256 test cases) | M4 (DONE) | `TEST_INFRA.md` |
| 20 | Fuzzing & Adversarial Harness | Random graph and boundary adversarial fuzz testing (15,000+ topol.) | M4 (DONE) | `TEST_INFRA.md` |
| 21 | Benchmark Suite Uplift | Comparative throughput/latency benchmarks for all topologies | M4 (DONE) | `examples/` |

---

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | CSR Graph & BitSet Data Structures | Implement contiguous `CsrGraph`, `BitSet`, zero-alloc adjacency compilation, preserving small graph & sink handling | none | DONE |
| M2 | Accelerated Eigensolver & Reusable Workspace | Implement auto-vectorized SpMV, Arrington clamping, null-space projection, momentum, Rayleigh quotient, and `PrunerWorkspace` | M1 | DONE |
| M3 | Security Metrics, Bisection & Policy Engine | Implement $\tau$-boundary bisection, Scale-Invariant Density Ratio, Instruction Neglect, Single-Token Tripwire, Telemetry Separation, and Input Validation | M2 | DONE |
| M4 | Comprehensive Testing, Benchmarking & Fuzzing | Implement Tier 1-4 E2E test suite, property-based tests, adversarial fuzzing, and comparative release benchmarks | M3 | DONE |

---

## Interface Contracts

### Full Public Interface
```rust
pub struct Topology {
    pub num_nodes: usize,
    pub edges: Vec<(usize, usize)>,
    pub sinks: BTreeSet<usize>,
}

pub struct CsrGraph {
    pub row_ptrs: Vec<usize>,
    pub col_indices: Vec<usize>,
    pub degrees: Vec<f64>,
}

pub struct BitSet {
    pub words: Vec<u64>,
    pub len: usize,
}

pub struct PrunerWorkspace {
    pub v_vec: Vec<f64>,
    pub v_m: Vec<f64>,
    pub v_prev_m: Vec<f64>,
    pub v_next: Vec<f64>,
    pub sink_bits: BitSet,
    pub island_bits: BitSet,
    pub csr_row_ptrs: Vec<usize>,
    pub csr_col_indices: Vec<usize>,
    pub degrees: Vec<f64>,
    pub cursor: Vec<usize>,
}

pub struct TauSpectralPruner {
    // Configured via PrunerBuilder
}

impl TauSpectralPruner {
    pub fn prune(&self, topology: &Topology, system_boundary_len: usize) -> Result<PrunerResolution, PrunerError>;
    pub fn prune_with_workspace(
        &self,
        topology: &Topology,
        system_boundary_len: usize,
        workspace: &mut PrunerWorkspace,
    ) -> Result<PrunerResolution, PrunerError>;
    pub fn tau(&self) -> f64;
    pub fn threat_threshold(&self) -> f64;
    pub fn max_iterations(&self) -> usize;
    pub fn tolerance(&self) -> f64;
    pub fn momentum_beta(&self) -> f64;
    pub fn system_start_idx(&self) -> usize;
}

pub struct PrunerBuilder { ... }
impl PrunerBuilder {
    pub fn tau(mut self, value: f64) -> Self;
    pub fn threat_threshold(mut self, value: f64) -> Self;
    pub fn max_iterations(mut self, value: usize) -> Self;
    pub fn tolerance(mut self, value: f64) -> Self;
    pub fn momentum_beta(mut self, value: f64) -> Self;
    pub fn system_start_idx(mut self, value: usize) -> Self;
    pub fn try_build(self) -> Result<TauSpectralPruner, PrunerError>;
    pub fn build(self) -> TauSpectralPruner;
}
```
