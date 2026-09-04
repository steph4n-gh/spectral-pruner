use crate::error::{PrunerError, Result};
use crate::graph::{BitSet, WeightedCsrGraph};
use std::collections::BTreeSet;
use std::fmt;

/// Structural input layout representing an abstract network matrix.
#[derive(Debug, Clone)]
pub struct Topology {
    pub num_nodes: usize,
    pub edges: Vec<(usize, usize)>,
    /// Positive finite weighted edges. Unweighted edges in [`Self::edges`]
    /// are interpreted as having weight `1.0`.
    pub weighted_edges: Vec<(usize, usize, f64)>,
    pub sinks: BTreeSet<usize>,
}

impl Topology {
    pub fn new(num_nodes: usize) -> Self {
        Self {
            num_nodes,
            edges: Vec::new(),
            weighted_edges: Vec::new(),
            sinks: BTreeSet::new(),
        }
    }

    pub fn add_edge(&mut self, source: usize, target: usize) {
        if source < self.num_nodes && target < self.num_nodes {
            self.edges.push((source, target));
        }
    }

    /// Adds a weighted undirected edge.
    ///
    /// Endpoint validation mirrors [`Self::add_edge`]. Weight validation is
    /// deliberately performed by the pruning boundary so direct mutation of
    /// the public edge vectors cannot bypass it.
    pub fn add_weighted_edge(&mut self, source: usize, target: usize, weight: f64) {
        if source < self.num_nodes && target < self.num_nodes {
            self.weighted_edges.push((source, target, weight));
        }
    }

    /// Returns the total number of submitted unweighted and weighted edges.
    #[inline]
    pub fn edge_count(&self) -> usize {
        self.edges.len() + self.weighted_edges.len()
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

/// Reusable numeric and CSR scratchpad for high-frequency or streaming
/// spectral partitioning. Returned and temporary partition vectors still allocate.
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
    /// CSR edge-weight scratchpad aligned with `csr_col_indices`
    pub csr_weights: Vec<f64>,
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
            v_next: Vec::new(),
            sink_bits: BitSet::new(0),
            island_bits: BitSet::new(0),
            csr_row_ptrs: Vec::new(),
            csr_col_indices: Vec::new(),
            csr_weights: Vec::new(),
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
            csr_weights: Vec::with_capacity(estimated_edges * 2),
            degrees: Vec::with_capacity(num_nodes),
            cursor: Vec::with_capacity(num_nodes),
        }
    }

    /// Resets the scratch buffers for a graph of size `num_nodes` without deallocating capacity.
    ///
    /// Clears and resizes the numeric scratch vectors (`v_vec`, `v_m`, `v_prev_m`, `v_next`),
    /// and resets the `sink_bits` and `island_bits` bitsets.
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
    /// Auditable measurements used to derive `action`.
    pub diagnostics: PrunerDiagnostics,
}

/// Machine-readable measurements and individual policy-trigger states.
#[derive(Debug, Clone, PartialEq)]
pub struct PrunerDiagnostics {
    pub boundary_configuration_valid: bool,
    /// Whether the iteration and eigenpair residual met the requested tolerance.
    /// This is a convergence check, not a proof that the eigenpair is the Fiedler pair.
    pub solver_converged: bool,
    pub solver_iterations: usize,
    /// `||L v - lambda v||_2 / max_degree` for the unit eigenvector; unavailable
    /// when the legacy small-graph path does not compute an eigenpair.
    pub relative_residual: Option<f64>,
    /// A configured connectivity policy could not use a converged estimate.
    pub numerical_failure_triggered: bool,
    pub island_node_count: usize,
    pub system_node_count: usize,
    pub internal_weight: f64,
    pub system_weight: f64,
    /// Total edge weight crossing from the local island to its complement.
    pub partition_cut_weight: f64,
    /// Weighted degree volume of the local island.
    pub island_volume: f64,
    /// Standard weighted conductance of the local island partition.
    pub conductance: f64,
    pub internal_density: f64,
    pub boundary_density: f64,
    /// Ratio of possible-edge-normalized internal and system-boundary density.
    pub possible_edge_density_ratio: f64,
    /// Signature scale-normalized ratio: `(internal * system_nodes) /
    /// (system_weight * island_nodes)`.
    pub density_ratio: f64,
    pub instruction_connection: f64,
    pub connectivity_triggered: bool,
    pub density_triggered: bool,
    pub instruction_neglect_triggered: bool,
    pub single_token_triggered: bool,
}

impl Default for PrunerDiagnostics {
    fn default() -> Self {
        Self {
            boundary_configuration_valid: true,
            solver_converged: false,
            solver_iterations: 0,
            relative_residual: None,
            numerical_failure_triggered: false,
            island_node_count: 0,
            system_node_count: 0,
            internal_weight: 0.0,
            system_weight: 0.0,
            partition_cut_weight: 0.0,
            island_volume: 0.0,
            conductance: 0.0,
            internal_density: 0.0,
            boundary_density: 0.0,
            possible_edge_density_ratio: 0.0,
            density_ratio: 0.0,
            instruction_connection: 0.0,
            connectivity_triggered: false,
            density_triggered: false,
            instruction_neglect_triggered: false,
            single_token_triggered: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAction {
    /// No containment action under the configured policy; not proof of benign input.
    Allow,
    /// A candidate island without a blocking trigger. The caller decides what to do.
    GarbageCollect,
    /// A blocking trigger or fail-closed condition. No external action is executed.
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

#[derive(Debug, Clone)]
pub struct TauSpectralPruner {
    tau: f64,
    threat_threshold: f64,
    max_iterations: usize,
    tolerance: f64,
    momentum_beta: f64,
    system_start_idx: usize,
    connectivity_threshold: Option<f64>,
    instruction_connection_threshold: f64,
    density_ratio_enabled: bool,
    instruction_neglect_enabled: bool,
    single_token_tripwire_enabled: bool,
}

impl TauSpectralPruner {
    pub fn builder() -> PrunerBuilder {
        PrunerBuilder::default()
    }

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

    /// Optional calibrated upper bound on algebraic connectivity. When set,
    /// an island with `lambda_2 <= threshold` triggers containment.
    #[inline]
    pub fn connectivity_threshold(&self) -> Option<f64> {
        self.connectivity_threshold
    }

    #[inline]
    pub fn instruction_connection_threshold(&self) -> f64 {
        self.instruction_connection_threshold
    }

    #[inline]
    pub fn density_ratio_enabled(&self) -> bool {
        self.density_ratio_enabled
    }

    #[inline]
    pub fn instruction_neglect_enabled(&self) -> bool {
        self.instruction_neglect_enabled
    }

    #[inline]
    pub fn single_token_tripwire_enabled(&self) -> bool {
        self.single_token_tripwire_enabled
    }

    /// Computes a polynomial-time spectral bisection heuristic approximation of the network topology
    /// via the Fiedler vector to determine containment policy.
    ///
    /// Allocates an internal `PrunerWorkspace`. For hot streaming loops, use
    /// [`prune_with_workspace`](Self::prune_with_workspace) to reuse numeric and CSR buffers.
    ///
    /// `system_boundary_len` is the inclusive final protected node index, despite
    /// its historical name. Zero disables system policy. The first protected
    /// index is set by [`PrunerBuilder::system_start_idx`].
    pub fn prune(
        &self,
        topology: &Topology,
        system_boundary_len: usize,
    ) -> Result<PrunerResolution> {
        let mut workspace = PrunerWorkspace::new();
        self.prune_with_workspace(topology, system_boundary_len, &mut workspace)
    }

    /// Computes a polynomial-time spectral bisection heuristic approximation using a pre-allocated
    /// scratchpad workspace. The eigensolver and CSR buffers are reused, while partition outputs allocate.
    #[allow(clippy::needless_range_loop)]
    pub fn prune_with_workspace(
        &self,
        topology: &Topology,
        system_boundary_len: usize,
        workspace: &mut PrunerWorkspace,
    ) -> Result<PrunerResolution> {
        // Configuration is validated once by the builder; fields are private.
        for &(u, v, weight) in &topology.weighted_edges {
            if !weight.is_finite() || weight <= 0.0 {
                return Err(PrunerError::MathError(format!(
                    "Weighted edge ({}, {}) must have a positive finite weight, got {}",
                    u, v, weight
                )));
            }
        }

        let n = topology.num_nodes;

        // Reset workspace scratchpad for N nodes
        workspace.reset_for_nodes(n);
        topology.populate_sink_bitset(&mut workspace.sink_bits);

        // System node predicate: active only when system_boundary_len > 0 and in [system_start_idx, system_boundary_len]
        let is_system_node = |i: usize| -> bool {
            system_boundary_len > 0 && i >= self.system_start_idx && i <= system_boundary_len
        };

        // Protected boundary anchors are always active. If a caller also marks
        // one as a sink, the protected-system invariant takes precedence.
        for i in 0..n {
            if is_system_node(i) {
                workspace.sink_bits.remove(i);
            }
        }

        let boundary_configuration_valid = system_boundary_len == 0
            || (self.system_start_idx <= system_boundary_len && self.system_start_idx < n);
        let system_node_count = (0..n).filter(|&i| is_system_node(i)).count();

        // Validate aggregate arithmetic even on the small-graph path. Finite
        // individual edges can overflow a degree or the total graph volume.
        WeightedCsrGraph::compile_into(
            topology,
            &workspace.sink_bits,
            &mut workspace.csr_row_ptrs,
            &mut workspace.csr_col_indices,
            &mut workspace.csr_weights,
            &mut workspace.degrees,
            &mut workspace.cursor,
        );
        let total_volume: f64 = workspace.degrees.iter().sum();
        if !total_volume.is_finite() {
            return Err(PrunerError::MathError(
                "Accumulated graph weight exceeds the finite numeric range".to_string(),
            ));
        }
        let max_degree = workspace.degrees.iter().copied().fold(0.0, f64::max);

        // Edge-case 1: Preserve the small-graph partition convention. A connected
        // small graph has no computed eigenpair and cannot authorize a calibrated policy.
        if n < 3 {
            let solver_converged = max_degree == 0.0;
            let numerical_failure_triggered =
                self.connectivity_threshold.is_some() && !solver_converged;
            let mainland: Vec<usize> = (0..n)
                .filter(|&i| !workspace.sink_bits.contains(i) && !is_system_node(i))
                .collect();
            return Ok(PrunerResolution {
                action: if !boundary_configuration_valid
                    || (system_boundary_len > 0 && numerical_failure_triggered)
                {
                    PolicyAction::FatalBlock
                } else {
                    PolicyAction::Allow
                },
                mainland_nodes: mainland,
                island_nodes: Vec::new(),
                connectivity_score: 0.0,
                diagnostics: PrunerDiagnostics {
                    boundary_configuration_valid,
                    solver_converged,
                    relative_residual: solver_converged.then_some(0.0),
                    numerical_failure_triggered,
                    system_node_count,
                    ..PrunerDiagnostics::default()
                },
            });
        }

        // Edge-case 2: All disconnected / zero-degree active nodes
        if max_degree == 0.0 {
            let mainland: Vec<usize> = (0..n)
                .filter(|&i| !workspace.sink_bits.contains(i) && !is_system_node(i))
                .collect();
            return Ok(PrunerResolution {
                action: if boundary_configuration_valid {
                    PolicyAction::Allow
                } else {
                    PolicyAction::FatalBlock
                },
                mainland_nodes: mainland,
                island_nodes: Vec::new(),
                connectivity_score: 0.0,
                diagnostics: PrunerDiagnostics {
                    boundary_configuration_valid,
                    solver_converged: true,
                    relative_residual: Some(0.0),
                    system_node_count,
                    ..PrunerDiagnostics::default()
                },
            });
        }

        // 2. Shifted operator M = I - (L / max_degree) / 2.
        // Divide each weight before multiplying, so a uniform rescaling does
        // not change convergence or overflow the shift's denominator.
        if workspace
            .csr_weights
            .iter()
            .any(|weight| weight / max_degree == 0.0)
        {
            return Err(PrunerError::MathError(
                "Edge weight underflows the normalized operator".to_string(),
            ));
        }

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
        let mut solver_iterations = 0;
        let mut solver_converged = false;
        let mut relative_residual = None;

        for iteration in 0..self.max_iterations {
            solver_iterations = iteration + 1;
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

            // Power Step evaluation: M = I - alpha * L
            // Memory-contiguous SpMV over CSR row slices
            for i in 0..n {
                if workspace.sink_bits.contains(i) {
                    workspace.v_m[i] = 0.0;
                    continue;
                }
                let start = workspace.csr_row_ptrs[i];
                let end = workspace.csr_row_ptrs[i + 1];
                let mut neighbor_sum = 0.0;
                for edge_idx in start..end {
                    let neighbor = workspace.csr_col_indices[edge_idx];
                    neighbor_sum +=
                        (workspace.csr_weights[edge_idx] / max_degree) * workspace.v_vec[neighbor];
                }
                workspace.v_m[i] = (1.0 - 0.5 * (workspace.degrees[i] / max_degree))
                    * workspace.v_vec[i]
                    + 0.5 * neighbor_sum;
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
            if !norm.is_finite() {
                return Err(PrunerError::MathError(
                    "Non-finite eigenvector norm".to_string(),
                ));
            }
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

            // Edge-energy form of the Rayleigh quotient avoids subtracting
            // nearly equal degree and adjacency terms. Reuse v_vec for L v
            // after its previous contents have been used by max_diff.
            let mut v_l_v = 0.0;
            for i in 0..n {
                if workspace.sink_bits.contains(i) {
                    continue;
                }
                let start = workspace.csr_row_ptrs[i];
                let end = workspace.csr_row_ptrs[i + 1];
                let mut row_sum = 0.0;
                for edge_idx in start..end {
                    let neighbor = workspace.csr_col_indices[edge_idx];
                    let difference = workspace.v_next[i] - workspace.v_next[neighbor];
                    let weight = workspace.csr_weights[edge_idx] / max_degree;
                    row_sum += weight * difference;
                    v_l_v += 0.5 * weight * difference * difference;
                }
                workspace.v_vec[i] = row_sum;
            }
            let residual = workspace
                .v_vec
                .iter()
                .zip(&workspace.v_next)
                .map(|(lv, v)| (lv - v_l_v * v).powi(2))
                .sum::<f64>()
                .sqrt();
            fiedler_value = v_l_v * max_degree;
            if v_l_v > 0.0 && fiedler_value == 0.0 {
                return Err(PrunerError::MathError(
                    "Connectivity score underflow".to_string(),
                ));
            }
            if !fiedler_value.is_finite() || !residual.is_finite() {
                return Err(PrunerError::MathError(
                    "Non-finite eigenpair calculation".to_string(),
                ));
            }
            relative_residual = Some(residual);

            // In-place copy to avoid vector recreation and heap allocation thrashing
            workspace.v_prev_m.copy_from_slice(&workspace.v_m);
            workspace.v_vec.copy_from_slice(&workspace.v_next);

            if max_diff < self.tolerance && residual < self.tolerance {
                solver_converged = true;
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
        let mut system_weight = 0.0;
        let mut internal_weight = 0.0;
        let mut partition_cut_weight = 0.0;

        let mut score_edge = |u: usize, v: usize, weight: f64| {
            if u >= n
                || v >= n
                || u == v
                || workspace.sink_bits.contains(u)
                || workspace.sink_bits.contains(v)
            {
                return;
            }
            let u_in_island = workspace.island_bits.contains(u);
            let v_in_island = workspace.island_bits.contains(v);
            let u_is_system = is_system_node(u);
            let v_is_system = is_system_node(v);
            let u_in_local_island = u_in_island && !u_is_system;
            let v_in_local_island = v_in_island && !v_is_system;

            if u_in_local_island != v_in_local_island {
                partition_cut_weight += weight;
            }

            if (u_in_local_island && v_is_system) || (v_in_local_island && u_is_system) {
                system_weight += weight;
            } else if u_in_local_island && v_in_local_island {
                // Each submitted undirected edge is counted exactly once.
                internal_weight += weight;
            }
        };

        for &(u, v) in &topology.edges {
            score_edge(u, v, 1.0);
        }
        for &(u, v, weight) in &topology.weighted_edges {
            score_edge(u, v, weight);
        }

        let island_local_nodes: Vec<usize> = island
            .iter()
            .copied()
            .filter(|&i| !is_system_node(i))
            .collect();
        let island_len = island_local_nodes.len() as f64;
        let system_len = system_node_count as f64;
        let island_volume: f64 = island_local_nodes
            .iter()
            .map(|&node| workspace.degrees[node])
            .sum();
        let complement_volume = (total_volume - island_volume).max(0.0);
        let smaller_volume = island_volume.min(complement_volume);
        let conductance = if smaller_volume > 0.0 {
            partition_cut_weight / smaller_volume
        } else {
            0.0
        };

        // Metric 1: ratio of possible-edge-normalized island density to
        // island-to-system boundary density. This is dimensionless and remains
        // stable when equivalent graph densities are reproduced at new scales.
        let internal_density = if island_len > 1.0 {
            (2.0 * internal_weight) / (island_len * (island_len - 1.0))
        } else {
            0.0
        };
        let boundary_density = if island_len > 0.0 && system_len > 0.0 {
            system_weight / (island_len * system_len)
        } else {
            0.0
        };
        let possible_edge_density_ratio = if boundary_density > 0.0 {
            internal_density / boundary_density
        } else if internal_density > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        let normalized_ratio = if system_weight > 0.0 && island_len > 0.0 {
            // Algebraically identical signature ratio, with division before
            // multiplication so an overflowing denominator cannot become zero.
            (internal_weight / system_weight) * (system_len / island_len)
        } else if internal_weight > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        // Infinity is meaningful only for a positive internal weight with no
        // system connection. Overflow with a positive denominator is an error.
        if !internal_density.is_finite()
            || !boundary_density.is_finite()
            || !conductance.is_finite()
            || (internal_weight > 0.0 && internal_density == 0.0)
            || (system_weight > 0.0 && boundary_density == 0.0 && system_len > 0.0)
            || (system_weight > 0.0
                && (!normalized_ratio.is_finite() || !possible_edge_density_ratio.is_finite()))
            || (internal_weight > 0.0
                && system_weight > 0.0
                && system_len > 0.0
                && (normalized_ratio == 0.0 || possible_edge_density_ratio == 0.0))
        {
            return Err(PrunerError::MathError(
                "Partition metric exceeds the supported numeric range".to_string(),
            ));
        }

        // Metric 2: Instruction neglect checking
        let instruction_neglect = if !island_local_nodes.is_empty() {
            system_weight / island_len
        } else {
            1.0
        };

        // Metric 3: Micro-Steering Single-Token Tripwire
        let is_control_vector = island_len == 1.0
            && internal_weight == 0.0
            && system_weight > 0.0
            && system_weight < 2.0;

        let density_triggered = self.density_ratio_enabled
            && normalized_ratio > 0.0
            && normalized_ratio >= self.threat_threshold;
        let instruction_neglect_triggered = self.instruction_neglect_enabled
            && instruction_neglect < self.instruction_connection_threshold;
        let single_token_triggered = self.single_token_tripwire_enabled && is_control_vector;
        let connectivity_triggered = matches!(
            self.connectivity_threshold,
            Some(threshold) if solver_converged && fiedler_value <= threshold
        );
        let numerical_failure_triggered =
            self.connectivity_threshold.is_some() && !solver_converged;

        // 5. Policy Enforcement Decision Processing
        let action = if !boundary_configuration_valid {
            // A configured boundary that maps to no valid interval is a policy
            // misconfiguration. Fail closed rather than silently allowing it.
            PolicyAction::FatalBlock
        } else if system_boundary_len == 0 {
            PolicyAction::Allow
        } else if numerical_failure_triggered {
            PolicyAction::FatalBlock
        } else if island_local_nodes.is_empty() {
            PolicyAction::Allow
        } else if connectivity_triggered
            || density_triggered
            || instruction_neglect_triggered
            || single_token_triggered
        {
            PolicyAction::FatalBlock
        } else {
            PolicyAction::GarbageCollect
        };

        // Exclude system boundary nodes from the final returned vectors,
        // but keep them in the classification internally for correct threat metrics.
        let final_mainland: Vec<usize> = mainland
            .into_iter()
            .filter(|&i| !is_system_node(i))
            .collect();
        let final_island: Vec<usize> = island.into_iter().filter(|&i| !is_system_node(i)).collect();

        Ok(PrunerResolution {
            action,
            mainland_nodes: final_mainland,
            island_nodes: final_island,
            connectivity_score: fiedler_value,
            diagnostics: PrunerDiagnostics {
                boundary_configuration_valid,
                solver_converged,
                solver_iterations,
                relative_residual,
                numerical_failure_triggered,
                island_node_count: island_local_nodes.len(),
                system_node_count,
                internal_weight,
                system_weight,
                partition_cut_weight,
                island_volume,
                conductance,
                internal_density,
                boundary_density,
                possible_edge_density_ratio,
                density_ratio: normalized_ratio,
                instruction_connection: instruction_neglect,
                connectivity_triggered,
                density_triggered,
                instruction_neglect_triggered,
                single_token_triggered,
            },
        })
    }
}

/// Fluent builder pattern interface to cleanly manage engine instantiation.
#[derive(Debug, Clone)]
pub struct PrunerBuilder {
    tau: f64,
    threat_threshold: f64,
    max_iterations: usize,
    tolerance: f64,
    momentum_beta: f64,
    system_start_idx: usize,
    connectivity_threshold: Option<f64>,
    instruction_connection_threshold: f64,
    density_ratio_enabled: bool,
    instruction_neglect_enabled: bool,
    single_token_tripwire_enabled: bool,
}

impl Default for PrunerBuilder {
    fn default() -> Self {
        Self {
            tau: 0.0,
            threat_threshold: 2.0,
            max_iterations: 10_000,
            tolerance: 1e-9,
            momentum_beta: 0.5,
            system_start_idx: 5,
            connectivity_threshold: None,
            instruction_connection_threshold: 0.1,
            density_ratio_enabled: true,
            instruction_neglect_enabled: true,
            single_token_tripwire_enabled: true,
        }
    }
}

impl PrunerBuilder {
    /// Injects the finite partition boundary; defaults to `0.0`.
    pub fn tau(mut self, value: f64) -> Self {
        self.tau = value;
        self
    }

    /// Sets the signature density-ratio trigger threshold; defaults to `2.0`.
    pub fn threat_threshold(mut self, value: f64) -> Self {
        self.threat_threshold = value;
        self
    }

    /// Sets a positive iteration budget; defaults to `10_000`.
    /// Exhaustion is reported in [`PrunerDiagnostics::solver_converged`].
    pub fn max_iterations(mut self, value: usize) -> Self {
        self.max_iterations = value;
        self
    }

    /// Sets a positive finite convergence tolerance; defaults to `1e-9`.
    pub fn tolerance(mut self, value: f64) -> Self {
        self.tolerance = value;
        self
    }

    pub fn momentum_beta(mut self, value: f64) -> Self {
        self.momentum_beta = value;
        self
    }

    /// Sets the first protected node index; defaults to `5`.
    /// The end supplied to [`TauSpectralPruner::prune`] is inclusive.
    pub fn system_start_idx(mut self, value: usize) -> Self {
        self.system_start_idx = value;
        self
    }

    /// Enables a calibrated algebraic-connectivity policy trigger.
    pub fn connectivity_threshold(mut self, value: f64) -> Self {
        self.connectivity_threshold = Some(value);
        self
    }

    /// Configures the instruction-connection level below which neglect fires.
    pub fn instruction_connection_threshold(mut self, value: f64) -> Self {
        self.instruction_connection_threshold = value;
        self
    }

    /// Enables or disables the normalized density-ratio policy trigger.
    pub fn density_ratio_enabled(mut self, value: bool) -> Self {
        self.density_ratio_enabled = value;
        self
    }

    /// Enables or disables the low instruction-connection policy trigger.
    pub fn instruction_neglect_enabled(mut self, value: bool) -> Self {
        self.instruction_neglect_enabled = value;
        self
    }

    /// Enables or disables the single-token policy trigger.
    pub fn single_token_tripwire_enabled(mut self, value: bool) -> Self {
        self.single_token_tripwire_enabled = value;
        self
    }

    /// Disables density, instruction-neglect, and single-token triggers.
    /// Boundary validation and any configured connectivity threshold remain active.
    /// This supports reproducible baseline and ablation experiments.
    pub fn spectral_only(mut self) -> Self {
        self.density_ratio_enabled = false;
        self.instruction_neglect_enabled = false;
        self.single_token_tripwire_enabled = false;
        self
    }

    /// Validates builder parameters and builds `TauSpectralPruner`.
    pub fn try_build(self) -> Result<TauSpectralPruner> {
        if !self.tau.is_finite() {
            return Err(PrunerError::MathError("Tau must be finite".to_string()));
        }
        if self.tolerance <= 0.0 || !self.tolerance.is_finite() {
            return Err(PrunerError::MathError(format!(
                "Tolerance must be strictly positive (> 0.0) and finite, got {}",
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
        if matches!(
            self.connectivity_threshold,
            Some(threshold) if !threshold.is_finite() || threshold < 0.0
        ) {
            return Err(PrunerError::MathError(
                "connectivity_threshold must be a non-negative finite value".to_string(),
            ));
        }
        if !self.instruction_connection_threshold.is_finite()
            || self.instruction_connection_threshold < 0.0
        {
            return Err(PrunerError::MathError(format!(
                "instruction_connection_threshold must be non-negative and finite, got {}",
                self.instruction_connection_threshold
            )));
        }

        Ok(TauSpectralPruner {
            tau: self.tau,
            threat_threshold: self.threat_threshold,
            max_iterations: self.max_iterations,
            tolerance: self.tolerance,
            momentum_beta: self.momentum_beta,
            system_start_idx: self.system_start_idx,
            connectivity_threshold: self.connectivity_threshold,
            instruction_connection_threshold: self.instruction_connection_threshold,
            density_ratio_enabled: self.density_ratio_enabled,
            instruction_neglect_enabled: self.instruction_neglect_enabled,
            single_token_tripwire_enabled: self.single_token_tripwire_enabled,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pruner_workspace_lifecycle() {
        let mut ws = PrunerWorkspace::new();
        assert_eq!(ws.v_vec.len(), 0);
        assert_eq!(ws.v_m.len(), 0);
        assert_eq!(ws.v_prev_m.len(), 0);
        assert_eq!(ws.v_next.len(), 0);
        assert_eq!(ws.sink_bits.len(), 0);
        assert_eq!(ws.island_bits.len(), 0);

        ws.reset_for_nodes(50);
        assert_eq!(ws.v_vec.len(), 50);
        assert_eq!(ws.v_m.len(), 50);
        assert_eq!(ws.v_prev_m.len(), 50);
        assert_eq!(ws.v_next.len(), 50);
        assert_eq!(ws.sink_bits.len(), 50);
        assert_eq!(ws.island_bits.len(), 50);

        let ws_cap = PrunerWorkspace::with_capacity(100, 200);
        assert!(ws_cap.v_vec.capacity() >= 100);
        assert!(ws_cap.csr_row_ptrs.capacity() >= 101);
        assert!(ws_cap.csr_col_indices.capacity() >= 400);
    }

    #[test]
    fn test_pruner_workspace_streaming_reuse() {
        let mut ws = PrunerWorkspace::with_capacity(20, 50);
        let pruner = TauSpectralPruner::builder().build();

        for size in 3..15 {
            let mut topo = Topology::new(size);
            for i in 0..size - 1 {
                topo.add_edge(i, i + 1);
            }
            let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
            assert_eq!(res.action, PolicyAction::Allow);
            assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), size);
        }
    }

    #[test]
    fn test_prune_with_workspace_parity_with_prune() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);
        topo.add_edge(3, 5);

        let mut ws = PrunerWorkspace::new();
        let res_workspace = pruner.prune_with_workspace(&topo, 5, &mut ws).unwrap();
        let res_prune = pruner.prune(&topo, 5).unwrap();

        assert_eq!(res_workspace, res_prune);
        assert_eq!(res_workspace.action, PolicyAction::FatalBlock);
        assert_eq!(res_workspace.island_nodes, vec![3]);
    }

    #[test]
    fn test_topology_populate_sink_bitset() {
        let mut topo = Topology::new(10);
        topo.add_sink(2);
        topo.add_sink(5);
        topo.add_sink(8);

        let mut bitset = BitSet::new(0);
        topo.populate_sink_bitset(&mut bitset);

        assert_eq!(bitset.len(), 10);
        assert_eq!(bitset.count_ones(), 3);
        assert!(bitset.contains(2));
        assert!(bitset.contains(5));
        assert!(bitset.contains(8));
        assert!(!bitset.contains(0));
        assert!(!bitset.contains(1));

        let bitset2 = topo.to_sink_bitset();
        assert_eq!(bitset, bitset2);
    }

    #[test]
    fn test_small_graph_fast_paths_n0_n1_n2() {
        let pruner = TauSpectralPruner::builder().build();
        let mut ws = PrunerWorkspace::new();

        // N = 0
        let topo0 = Topology::new(0);
        let res0 = pruner.prune_with_workspace(&topo0, 0, &mut ws).unwrap();
        assert_eq!(res0.action, PolicyAction::Allow);
        assert!(res0.mainland_nodes.is_empty());
        assert!(res0.island_nodes.is_empty());

        // N = 1
        let topo1 = Topology::new(1);
        let res1 = pruner.prune_with_workspace(&topo1, 0, &mut ws).unwrap();
        assert_eq!(res1.action, PolicyAction::Allow);
        assert_eq!(res1.mainland_nodes, vec![0]);
        assert!(res1.island_nodes.is_empty());

        // N = 2 with sink
        let mut topo2 = Topology::new(2);
        topo2.add_sink(1);
        let res2 = pruner.prune_with_workspace(&topo2, 0, &mut ws).unwrap();
        assert_eq!(res2.action, PolicyAction::Allow);
        assert_eq!(res2.mainland_nodes, vec![0]);
        assert!(res2.island_nodes.is_empty());
    }

    #[test]
    fn test_all_isolated_fast_path() {
        let pruner = TauSpectralPruner::builder().build();
        let mut ws = PrunerWorkspace::new();

        let topo = Topology::new(10); // 10 nodes, 0 edges
        let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len(), 10);
        assert!(res.island_nodes.is_empty());
        assert_eq!(res.connectivity_score, 0.0);
    }

    #[test]
    fn test_workspace_default() {
        let ws: PrunerWorkspace = Default::default();
        assert_eq!(ws.v_vec.len(), 0);
        assert_eq!(ws.sink_bits.len(), 0);
    }

    #[test]
    fn test_display_policy_action() {
        assert_eq!(format!("{}", PolicyAction::Allow), "ALLOW");
        assert_eq!(
            format!("{}", PolicyAction::GarbageCollect),
            "GARBAGE_COLLECT"
        );
        assert_eq!(format!("{}", PolicyAction::FatalBlock), "FATAL_BLOCK");
    }

    #[test]
    fn test_pruner_builder_getters_and_defaults() {
        let pruner = TauSpectralPruner::builder()
            .tau(0.125)
            .threat_threshold(3.5)
            .max_iterations(500)
            .tolerance(1e-6)
            .momentum_beta(0.4)
            .system_start_idx(4)
            .build();

        assert_eq!(pruner.tau(), 0.125);
        assert_eq!(pruner.threat_threshold(), 3.5);
        assert_eq!(pruner.max_iterations(), 500);
        assert_eq!(pruner.tolerance(), 1e-6);
        assert_eq!(pruner.momentum_beta(), 0.4);
        assert_eq!(pruner.system_start_idx(), 4);
    }

    #[test]
    fn test_pruner_builder_try_build_validation_errors() {
        // Tolerance <= 0.0 or NaN
        assert!(TauSpectralPruner::builder()
            .tolerance(0.0)
            .try_build()
            .is_err());
        assert!(TauSpectralPruner::builder()
            .tolerance(-1.0)
            .try_build()
            .is_err());
        assert!(TauSpectralPruner::builder()
            .tolerance(f64::NAN)
            .try_build()
            .is_err());

        // Max iterations == 0
        assert!(TauSpectralPruner::builder()
            .max_iterations(0)
            .try_build()
            .is_err());

        // Momentum beta < 0.0 or >= 1.0 or NaN
        assert!(TauSpectralPruner::builder()
            .momentum_beta(-0.1)
            .try_build()
            .is_err());
        assert!(TauSpectralPruner::builder()
            .momentum_beta(1.0)
            .try_build()
            .is_err());
        assert!(TauSpectralPruner::builder()
            .momentum_beta(1.5)
            .try_build()
            .is_err());
        assert!(TauSpectralPruner::builder()
            .momentum_beta(f64::NAN)
            .try_build()
            .is_err());

        // Threat threshold < 0.0 or NaN
        assert!(TauSpectralPruner::builder()
            .threat_threshold(-0.01)
            .try_build()
            .is_err());
        assert!(TauSpectralPruner::builder()
            .threat_threshold(f64::NAN)
            .try_build()
            .is_err());
    }

    #[test]
    #[should_panic(expected = "Invalid PrunerBuilder configuration")]
    fn test_pruner_builder_build_panics_on_invalid() {
        let _ = TauSpectralPruner::builder().tolerance(0.0).build();
    }

    #[test]
    fn test_telemetry_separation_small_graph_fast_path() {
        // N < 3 with system_boundary_len > 0
        let pruner = TauSpectralPruner::builder().system_start_idx(1).build();
        let topo = Topology::new(2); // nodes 0, 1; node 1 is system

        let res = pruner.prune(&topo, 1).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        // Node 1 is a system node and must be stripped from mainland_nodes
        assert_eq!(res.mainland_nodes, vec![0]);
        assert!(res.island_nodes.is_empty());
    }

    #[test]
    fn test_telemetry_separation_all_disconnected_fast_path() {
        // Disconnected graph with system_boundary_len > 0
        let pruner = TauSpectralPruner::builder().system_start_idx(3).build();
        let topo = Topology::new(5); // 5 isolated nodes; nodes 3, 4 are system

        let res = pruner.prune(&topo, 4).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        // System nodes 3 and 4 must be stripped from mainland_nodes
        assert_eq!(res.mainland_nodes, vec![0, 1, 2]);
        assert!(res.island_nodes.is_empty());
    }

    #[test]
    fn test_telemetry_separation_zero_boundary_length_does_not_strip() {
        // When system_boundary_len == 0, system_start_idx = 0 must NOT strip node 0
        let pruner = TauSpectralPruner::builder().system_start_idx(0).build();
        let topo = Topology::new(3);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes, vec![0, 1, 2]);
    }

    #[test]
    fn test_telemetry_separation_inverted_start_idx() {
        // When system_start_idx > system_boundary_len > 0, system range is empty
        let pruner = TauSpectralPruner::builder().system_start_idx(10).build();
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);

        let res = pruner.prune(&topo, 2).unwrap();
        // Since system range [10, 2] is empty, no nodes are stripped as system
        let mut total_nodes = res.mainland_nodes.clone();
        total_nodes.extend(&res.island_nodes);
        total_nodes.sort();
        assert_eq!(total_nodes, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_policy_action_allow_when_boundary_is_zero() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        // Island cluster (3, 4) disconnected from mainland (0, 1, 2)
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);
        topo.add_edge(3, 4);

        // With system_boundary_len == 0, policy is always Allow
        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_policy_action_fatal_block_density_ratio() {
        // Internal dense island with high internal edges compared to weak system edges
        let pruner = TauSpectralPruner::builder()
            .threat_threshold(1.5)
            .system_start_idx(7)
            .build();
        let mut topo = Topology::new(9);

        // Mainland cluster (0, 1, 2, 3)
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(3, 0);

        // Island cluster (4, 5, 6) with high internal connectivity: 3 internal edges
        topo.add_edge(4, 5);
        topo.add_edge(5, 6);
        topo.add_edge(6, 4);

        // Connect island node 4 to system node 7 and 8 (to_system = 2)
        topo.add_edge(4, 7);
        topo.add_edge(4, 8);

        // system_len = 8, island_len = 3
        // internal = 3 * 2 = 6, to_system = 2
        // ratio = (6 * 8) / (2 * 3) = 48 / 6 = 8.0 > 1.5 threshold
        let res = pruner.prune(&topo, 8).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }

    #[test]
    fn test_policy_action_fatal_block_instruction_neglect() {
        // Island completely decoupled from system space (to_system == 0)
        let pruner = TauSpectralPruner::builder()
            .threat_threshold(100.0) // high threshold to ensure neglect triggers it
            .system_start_idx(5)
            .build();
        let mut topo = Topology::new(7);

        // Mainland (0, 1, 2, 3)
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(3, 0);

        // Island (4) with 0 connections to system nodes 5, 6
        topo.add_edge(0, 5);
        topo.add_edge(1, 6);

        let res = pruner.prune(&topo, 6).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }

    #[test]
    fn test_policy_action_garbage_collect_benign_cluster() {
        // Benign cluster with healthy system connections and low internal ratio
        let pruner = TauSpectralPruner::builder()
            .threat_threshold(10.0)
            .system_start_idx(5)
            .build();
        let mut topo = Topology::new(7);

        // Mainland (0, 1, 2, 3)
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);
        topo.add_edge(3, 0);

        // Island (4) connected heavily to system node 5
        topo.add_edge(4, 5);
        // Connect mainland to system as well
        topo.add_edge(0, 5);

        let res = pruner.prune(&topo, 5).unwrap();
        // Since island has 1 node and 1 system edge (0 internal), this triggers single-token tripwire.
        assert_eq!(res.action, PolicyAction::FatalBlock);

        // Let's create an island with 2 nodes (4, 6) connected to system (5) and between each other
        let mut topo2 = Topology::new(8);
        topo2.add_edge(0, 1);
        topo2.add_edge(1, 2);
        topo2.add_edge(2, 3);
        topo2.add_edge(3, 0);

        // Island (4, 6): 1 internal edge (4, 6), 2 system edges (4, 7), (6, 7)
        topo2.add_edge(4, 6);
        topo2.add_edge(4, 7);
        topo2.add_edge(6, 7);

        let pruner2 = TauSpectralPruner::builder()
            .threat_threshold(10.0)
            .system_start_idx(7)
            .build();
        let res2 = pruner2.prune(&topo2, 7).unwrap();
        // island_len = 2, system_len = 7
        // internal = 2, to_system = 2
        // neglect = 2 / 2 = 1.0 (>= 0.1)
        // ratio = (2 * 7) / (2 * 2) = 14 / 4 = 3.5 <= 10.0
        // is_control_vector = false (island_len == 2)
        // Verdict: GarbageCollect
        assert_eq!(res2.action, PolicyAction::GarbageCollect);
        assert_eq!(res2.island_nodes.len(), 2);
    }

    #[test]
    fn test_custom_tau_boundary() {
        let pruner_neg = TauSpectralPruner::builder().tau(-1.0).build();
        let pruner_pos = TauSpectralPruner::builder().tau(1.0).build();

        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);

        let res_neg = pruner_neg.prune(&topo, 0).unwrap();
        let res_pos = pruner_pos.prune(&topo, 0).unwrap();

        assert_eq!(res_neg.action, PolicyAction::Allow);
        assert_eq!(res_pos.action, PolicyAction::Allow);
    }
}
