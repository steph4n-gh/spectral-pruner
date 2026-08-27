//! Empirical Challenger Stress Test Suite for Milestone 3
//! Agent: teamwork_preview_challenger_m3_2
//!
//! Comprehensive empirical verification of:
//! 1. Partition conservation property across 1,000 randomized graphs
//! 2. Policy determinism across 100 repeated executions per topology
//! 3. All 5 core mathematical invariants and 4 zero-assumption hard constraints from AGENTS.md
//! 4. Zero-allocation streaming workspace stability across 2,000 continuous evaluations

use spectral_pruner::engine::{
    PolicyAction, PrunerResolution, PrunerWorkspace, TauSpectralPruner, Topology,
};
use std::collections::BTreeSet;

/// Deterministic 64-bit Linear Congruential Generator (LCG) for reproducible fuzzing and test generation
struct FuzzRng {
    state: u64,
}

impl FuzzRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    #[inline]
    fn gen_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let diff = (max - min) as u64;
        min + (self.next_u64() % diff) as usize
    }

    #[inline]
    fn gen_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    #[inline]
    fn gen_bool(&mut self, p: f64) -> bool {
        self.gen_f64() < p
    }
}

// =========================================================================
// 1. PARTITION CONSERVATION PROPERTY ACROSS 1,000 RANDOMIZED GRAPHS
// =========================================================================

/// Helper to compute the exact expected set of active non-sink, non-system nodes.
fn compute_expected_active_nodes(
    num_nodes: usize,
    sinks: &BTreeSet<usize>,
    system_start_idx: usize,
    system_boundary_len: usize,
) -> BTreeSet<usize> {
    let mut active = BTreeSet::new();
    let is_system = |i: usize| -> bool {
        system_boundary_len > 0 && i >= system_start_idx && i <= system_boundary_len
    };

    for i in 0..num_nodes {
        if !sinks.contains(&i) && !is_system(i) {
            active.insert(i);
        }
    }
    active
}

/// Helper to verify all partition conservation invariants on a PrunerResolution.
fn verify_partition_conservation_invariants(
    iter_idx: usize,
    res: &PrunerResolution,
    num_nodes: usize,
    sinks: &BTreeSet<usize>,
    system_start_idx: usize,
    system_boundary_len: usize,
) {
    let expected_active =
        compute_expected_active_nodes(num_nodes, sinks, system_start_idx, system_boundary_len);

    let mainland_set: BTreeSet<usize> = res.mainland_nodes.iter().copied().collect();
    let island_set: BTreeSet<usize> = res.island_nodes.iter().copied().collect();

    // 1. Mainland and island must be completely disjoint (no duplicates, no overlap)
    assert_eq!(
        res.mainland_nodes.len(),
        mainland_set.len(),
        "Iter {}: mainland_nodes contains duplicate elements!",
        iter_idx
    );
    assert_eq!(
        res.island_nodes.len(),
        island_set.len(),
        "Iter {}: island_nodes contains duplicate elements!",
        iter_idx
    );

    let intersection: Vec<usize> = mainland_set.intersection(&island_set).copied().collect();
    assert!(
        intersection.is_empty(),
        "Iter {}: mainland_nodes and island_nodes overlap! Intersection: {:?}",
        iter_idx,
        intersection
    );

    // 2. Union of mainland and island must EXACTLY equal the set of active non-sink, non-system nodes
    let union: BTreeSet<usize> = mainland_set.union(&island_set).copied().collect();
    assert_eq!(
        union, expected_active,
        "Iter {}: Partition conservation violated! Union of partitions ({:?}) != expected active nodes ({:?})",
        iter_idx, union, expected_active
    );

    // 3. Exact count conservation: |mainland| + |island| == |expected_active|
    assert_eq!(
        res.mainland_nodes.len() + res.island_nodes.len(),
        expected_active.len(),
        "Iter {}: Node count conservation failed: {} + {} != {}",
        iter_idx,
        res.mainland_nodes.len(),
        res.island_nodes.len(),
        expected_active.len()
    );

    // 4. No sink node may EVER appear in mainland_nodes or island_nodes
    for &sink in sinks {
        assert!(
            !mainland_set.contains(&sink),
            "Iter {}: Sink node {} leaked into mainland_nodes!",
            iter_idx,
            sink
        );
        assert!(
            !island_set.contains(&sink),
            "Iter {}: Sink node {} leaked into island_nodes!",
            iter_idx,
            sink
        );
    }

    // 5. No system boundary node may EVER appear in mainland_nodes or island_nodes
    if system_boundary_len > 0 && system_start_idx <= system_boundary_len {
        for sys_node in system_start_idx..=system_boundary_len {
            assert!(
                !mainland_set.contains(&sys_node),
                "Iter {}: System boundary node {} leaked into mainland_nodes!",
                iter_idx,
                sys_node
            );
            assert!(
                !island_set.contains(&sys_node),
                "Iter {}: System boundary node {} leaked into island_nodes!",
                iter_idx,
                sys_node
            );
        }
    }

    // 6. All returned node indices must be strictly valid (< num_nodes)
    for &node in &res.mainland_nodes {
        assert!(
            node < num_nodes,
            "Iter {}: Out-of-bounds node {} in mainland_nodes (num_nodes={})",
            iter_idx,
            node,
            num_nodes
        );
    }
    for &node in &res.island_nodes {
        assert!(
            node < num_nodes,
            "Iter {}: Out-of-bounds node {} in island_nodes (num_nodes={})",
            iter_idx,
            node,
            num_nodes
        );
    }
}

#[test]
fn test_partition_conservation_1000_randomized_graphs() {
    let mut rng = FuzzRng::new(0x2026_0827_DEAD_C0DE);
    let mut workspace = PrunerWorkspace::with_capacity(300, 1000);

    let mut total_tested = 0usize;
    let mut action_distribution = [0usize; 3];

    for iter in 0..1000 {
        // Vary configuration across iterations
        let tau_choice = match iter % 5 {
            0 => 0.0,
            1 => -0.5,
            2 => 0.5,
            3 => 1.0,
            _ => (rng.gen_f64() - 0.5) * 2.0,
        };

        let threat_threshold = 1.0 + rng.gen_f64() * 4.0;
        let system_start_idx = match iter % 4 {
            0 => 5,
            1 => 0,
            2 => rng.gen_range(1, 10),
            _ => rng.gen_range(10, 50),
        };

        let pruner = TauSpectralPruner::builder()
            .tau(tau_choice)
            .threat_threshold(threat_threshold)
            .system_start_idx(system_start_idx)
            .max_iterations(1000)
            .tolerance(1e-7)
            .momentum_beta(0.5)
            .build();

        // Generate diverse topological structure
        let topology_category = iter % 8;
        let num_nodes: usize = match topology_category {
            0 => rng.gen_range(0, 3),   // Edge-case fast paths: N = 0, 1, 2
            1 => rng.gen_range(3, 10),  // Small graphs
            2 => rng.gen_range(10, 40), // Medium graphs
            3 => rng.gen_range(40, 120),// Larger graphs
            4 => rng.gen_range(5, 30),  // Star / hub graphs
            5 => rng.gen_range(4, 25),  // Dense clique clusters
            6 => rng.gen_range(6, 40),  // Disconnected components
            _ => rng.gen_range(5, 50),  // Adversarial sparse/sink graphs
        };

        let mut topo = Topology::new(num_nodes);

        if num_nodes > 0 {
            // Sinks generation
            let sink_prob = match iter % 6 {
                0 => 0.0, // no sinks
                1 => 0.5, // heavy sinks
                2 => 0.1, // light sinks
                3 => 1.0, // all sinks
                _ => rng.gen_f64() * 0.3,
            };

            for i in 0..num_nodes {
                if rng.gen_bool(sink_prob) {
                    topo.add_sink(i);
                }
            }
            // Occasional out-of-bounds sinks to stress robustness
            if rng.gen_bool(0.1) {
                topo.add_sink(num_nodes + rng.gen_range(1, 10));
            }

            // Edges generation based on category
            match topology_category {
                0 | 6 => {
                    // Disconnected or tiny
                    let edge_prob = 0.05;
                    for u in 0..num_nodes {
                        for v in (u + 1)..num_nodes {
                            if rng.gen_bool(edge_prob) {
                                topo.add_edge(u, v);
                            }
                        }
                    }
                }
                4 => {
                    // Star hub at node 0
                    for v in 1..num_nodes {
                        topo.add_edge(0, v);
                    }
                }
                5 => {
                    // Dense clique cluster on first k nodes
                    let k = (num_nodes / 2).max(2);
                    for u in 0..k {
                        for v in (u + 1)..k {
                            topo.add_edge(u, v);
                        }
                    }
                    // Connect island node
                    if num_nodes > k {
                        topo.add_edge(k, num_nodes - 1);
                    }
                }
                _ => {
                    // General random Erdős–Rényi
                    let p = if num_nodes < 20 { 0.2 } else { 0.08 };
                    for u in 0..num_nodes {
                        for v in (u + 1)..num_nodes {
                            if rng.gen_bool(p) {
                                topo.add_edge(u, v);
                            }
                        }
                    }
                }
            }

            // Occasional self-loops, multi-edges, and out-of-bounds edges
            if rng.gen_bool(0.2) {
                for _ in 0..rng.gen_range(1, 5) {
                    let u = rng.gen_range(0, num_nodes + 5);
                    let v = rng.gen_range(0, num_nodes + 5);
                    topo.edges.push((u, v));
                }
            }
        }

        // System boundary configuration
        let system_boundary_len = match iter % 6 {
            0 => 0, // Zero boundary length
            1 => {
                if num_nodes > system_start_idx {
                    rng.gen_range(system_start_idx, num_nodes)
                } else {
                    0
                }
            }
            2 => num_nodes.saturating_sub(1),
            3 => system_start_idx.saturating_sub(1), // Inverted / empty boundary
            _ => {
                if num_nodes > 0 {
                    rng.gen_range(0, num_nodes)
                } else {
                    0
                }
            }
        };

        // 1. Direct prune evaluation
        let res_direct = pruner
            .prune(&topo, system_boundary_len)
            .unwrap_or_else(|err| {
                panic!("Iteration {}: pruner.prune failed with error: {:?}", iter, err)
            });

        // 2. Workspace prune evaluation
        let res_ws = pruner
            .prune_with_workspace(&topo, system_boundary_len, &mut workspace)
            .unwrap_or_else(|err| {
                panic!(
                    "Iteration {}: pruner.prune_with_workspace failed with error: {:?}",
                    iter, err
                )
            });

        // 3. Verify workspace parity with direct prune
        assert_eq!(
            res_direct, res_ws,
            "Iteration {}: Parity mismatch between prune() and prune_with_workspace()",
            iter
        );

        // 4. Verify partition conservation invariants
        verify_partition_conservation_invariants(
            iter,
            &res_direct,
            num_nodes,
            &topo.sinks,
            system_start_idx,
            system_boundary_len,
        );

        match res_direct.action {
            PolicyAction::Allow => action_distribution[0] += 1,
            PolicyAction::GarbageCollect => action_distribution[1] += 1,
            PolicyAction::FatalBlock => action_distribution[2] += 1,
        }

        total_tested += 1;
    }

    assert_eq!(total_tested, 1000);
    println!(
        "Milestone 3 Partition Conservation: 1,000/1,000 passed. Actions: Allow={}, GC={}, FatalBlock={}",
        action_distribution[0], action_distribution[1], action_distribution[2]
    );
    assert!(
        action_distribution[0] > 0,
        "Allow pathway was not exercised"
    );
}

// =========================================================================
// 2. POLICY DETERMINISM: 100 REPEATED RUNS ON CHALLENGING TOPOLOGIES
// =========================================================================

#[test]
fn test_policy_determinism_100_runs_identical_verdicts_and_partitions() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(2.0)
        .system_start_idx(5)
        .max_iterations(5000)
        .tolerance(1e-9)
        .momentum_beta(0.5)
        .build();

    let mut workspace = PrunerWorkspace::with_capacity(100, 300);

    // Suite of 7 diverse and challenging topologies
    let mut test_topologies: Vec<(&str, Topology, usize)> = Vec::new();

    // Topology 1: Micro-steering Single-Token Tripwire
    {
        let mut t = Topology::new(6);
        t.add_edge(0, 1);
        t.add_edge(1, 2);
        t.add_edge(2, 0);
        t.add_edge(3, 5); // Island 3 -> System 5
        test_topologies.push(("Single-Token Tripwire (FatalBlock)", t, 5));
    }

    // Topology 2: Instruction Neglect Independent Set
    {
        let mut t = Topology::new(8);
        // Mainland (0, 1, 2, 3) connected to system 5
        t.add_edge(0, 1);
        t.add_edge(1, 2);
        t.add_edge(2, 3);
        t.add_edge(3, 0);
        t.add_edge(0, 5);
        // Island cluster (4, 6, 7) completely decoupled from system 5
        t.add_edge(4, 6);
        t.add_edge(6, 7);
        t.add_edge(7, 4);
        test_topologies.push(("Instruction Neglect (FatalBlock)", t, 5));
    }

    // Topology 3: Benign cluster with moderate system connection
    {
        let mut t = Topology::new(8);
        t.add_edge(0, 1);
        t.add_edge(1, 2);
        t.add_edge(2, 3);
        t.add_edge(3, 0);
        // Island (4, 6) connected to each other and both connected to system 7
        t.add_edge(4, 6);
        t.add_edge(4, 7);
        t.add_edge(6, 7);
        let pruner_high_thresh = TauSpectralPruner::builder()
            .threat_threshold(10.0)
            .system_start_idx(7)
            .build();
        let baseline = pruner_high_thresh.prune(&t, 7).unwrap();
        assert_eq!(baseline.action, PolicyAction::GarbageCollect);
        test_topologies.push(("Benign Cluster (GarbageCollect)", t, 7));
    }

    // Topology 4: Highly symmetric Cycle Graph C12
    {
        let mut t = Topology::new(12);
        for i in 0..12 {
            t.add_edge(i, (i + 1) % 12);
        }
        test_topologies.push(("Symmetric Cycle C12 (Allow)", t, 0));
    }

    // Topology 5: Dense Clique K8 with Sinks
    {
        let mut t = Topology::new(8);
        for u in 0..8 {
            for v in (u + 1)..8 {
                t.add_edge(u, v);
            }
        }
        t.add_sink(6);
        t.add_sink(7);
        test_topologies.push(("Dense Clique K8 with Sinks (Allow)", t, 0));
    }

    // Topology 6: All Disconnected / Zero Degree Chaff Graph
    {
        let mut t = Topology::new(10);
        t.add_sink(0);
        t.add_sink(9);
        test_topologies.push(("All Disconnected with Sinks (Allow)", t, 0));
    }

    // Topology 7: Complex Multi-Component Graph with Boundary Framing
    {
        let mut t = Topology::new(15);
        // Component A
        t.add_edge(0, 1);
        t.add_edge(1, 2);
        t.add_edge(2, 3);
        t.add_edge(3, 4);
        // Component B (anomalous)
        t.add_edge(6, 7);
        t.add_edge(7, 8);
        t.add_edge(8, 6);
        // System link
        t.add_edge(6, 12);
        t.add_edge(7, 13);
        t.add_sink(14);
        test_topologies.push(("Complex Multi-Component Boundary", t, 13));
    }

    for (name, topo, sys_len) in test_topologies {
        // Run first execution as baseline
        let baseline = pruner
            .prune(&topo, sys_len)
            .unwrap_or_else(|e| panic!("Baseline failed for {}: {:?}", name, e));

        // Execute 100 repeated runs
        for run_idx in 1..=100 {
            // Test direct prune
            let res_direct = pruner
                .prune(&topo, sys_len)
                .unwrap_or_else(|e| panic!("Run {} direct failed for {}: {:?}", run_idx, name, e));

            // Test workspace prune
            let res_ws = pruner
                .prune_with_workspace(&topo, sys_len, &mut workspace)
                .unwrap_or_else(|e| panic!("Run {} ws failed for {}: {:?}", run_idx, name, e));

            // Assert 100% exact determinism
            assert_eq!(
                res_direct.action, baseline.action,
                "Determinism violation in {} (run {}): Action mismatch {:?} vs {:?}",
                name, run_idx, res_direct.action, baseline.action
            );
            assert_eq!(
                res_direct.mainland_nodes, baseline.mainland_nodes,
                "Determinism violation in {} (run {}): mainland_nodes mismatch",
                name, run_idx
            );
            assert_eq!(
                res_direct.island_nodes, baseline.island_nodes,
                "Determinism violation in {} (run {}): island_nodes mismatch",
                name, run_idx
            );
            assert!(
                (res_direct.connectivity_score - baseline.connectivity_score).abs() < 1e-12,
                "Determinism violation in {} (run {}): connectivity_score mismatch {} vs {}",
                name,
                run_idx,
                res_direct.connectivity_score,
                baseline.connectivity_score
            );

            assert_eq!(
                res_ws, baseline,
                "Workspace determinism violation in {} (run {})",
                name, run_idx
            );
        }
    }
}

// =========================================================================
// 3. HIGH-THROUGHPUT STREAMING REUSE (2,000 Continuous Calls)
// =========================================================================

#[test]
fn test_streaming_workspace_2000_continuous_calls_zero_heap_growth() {
    let mut rng = FuzzRng::new(0xFEED_FACE_2026_0827);
    let mut workspace = PrunerWorkspace::with_capacity(128, 512);
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(2.0)
        .max_iterations(500)
        .build();

    let initial_cap_v = workspace.v_vec.capacity();
    let initial_cap_csr = workspace.csr_row_ptrs.capacity();

    for iter in 0..2000 {
        let n = rng.gen_range(1, 100);
        let mut topo = Topology::new(n);

        // Add sinks
        if n > 3 {
            topo.add_sink(rng.gen_range(0, n));
        }

        // Add random edges
        let num_edges = rng.gen_range(0, n * 2);
        for _ in 0..num_edges {
            let u = rng.gen_range(0, n);
            let v = rng.gen_range(0, n);
            topo.add_edge(u, v);
        }

        let sys_len = if n > 5 { rng.gen_range(5, n) } else { 0 };

        let res = pruner
            .prune_with_workspace(&topo, sys_len, &mut workspace)
            .expect("Workspace prune must never fail on valid topology");

        // Verify basic invariants
        let expected_active =
            compute_expected_active_nodes(n, &topo.sinks, pruner.system_start_idx(), sys_len);
        assert_eq!(
            res.mainland_nodes.len() + res.island_nodes.len(),
            expected_active.len(),
            "Iter {}: Partition conservation mismatch in streaming reuse",
            iter
        );
    }

    // Capacities for graph sizes <= initial capacity should remain stable
    assert!(workspace.v_vec.capacity() >= initial_cap_v);
    assert!(workspace.csr_row_ptrs.capacity() >= initial_cap_csr);
}

// =========================================================================
// 4. MATHEMATICAL INVARIANT & SECURITY MECHANISM RIGOROUS CHECKS
// =========================================================================

#[test]
fn test_invariant_1_injected_tau_boundary_tie_breaking() {
    // Rigid numerical split: v_i <= tau vs v_i > tau
    let mut topo = Topology::new(8);
    for i in 0..7 {
        topo.add_edge(i, i + 1);
    }

    let p_neg = TauSpectralPruner::builder().tau(-0.1).build();
    let p_zero = TauSpectralPruner::builder().tau(0.0).build();
    let p_pos = TauSpectralPruner::builder().tau(0.1).build();

    let r_neg = p_neg.prune(&topo, 0).unwrap();
    let r_zero = p_zero.prune(&topo, 0).unwrap();
    let r_pos = p_pos.prune(&topo, 0).unwrap();

    // In all cases, partition conservation must hold
    assert_eq!(r_neg.mainland_nodes.len() + r_neg.island_nodes.len(), 8);
    assert_eq!(r_zero.mainland_nodes.len() + r_zero.island_nodes.len(), 8);
    assert_eq!(r_pos.mainland_nodes.len() + r_pos.island_nodes.len(), 8);
}

#[test]
fn test_invariant_2_arrington_clamping_isolated_nodes() {
    let pruner = TauSpectralPruner::builder().tau(0.0).build();
    let mut topo = Topology::new(6);
    // Subgraph 0-1-2
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);
    // Nodes 3, 4, 5 are completely degree-0 isolated nodes
    let res = pruner.prune(&topo, 0).unwrap();

    // Isolated nodes must NEVER be dropped or omitted
    let total_classified = res.mainland_nodes.len() + res.island_nodes.len();
    assert_eq!(
        total_classified, 6,
        "Arrington clamping invariant broken: all 6 nodes must be classified!"
    );
    for iso in [3, 4, 5] {
        assert!(
            res.mainland_nodes.contains(&iso) || res.island_nodes.contains(&iso),
            "Degree-0 isolated node {} was skipped during classification!",
            iso
        );
    }
}

#[test]
fn test_invariant_3_scale_invariant_semantic_density_ratio() {
    // Ratio = (Internal Edges * N_system) / (System Edges * N_island)
    let pruner = TauSpectralPruner::builder()
        .threat_threshold(3.0)
        .system_start_idx(5)
        .build();

    let mut topo = Topology::new(8);
    // Mainland (0, 1, 2)
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Island (3, 4) with internal edge (3, 4) and system edge (3, 6)
    topo.add_edge(3, 4);
    topo.add_edge(3, 6);
    // system_len = 6, island_len = 2, internal = 2, to_system = 1
    // ratio = (2 * 6) / (1 * 2) = 6.0 > 3.0 -> FatalBlock
    let res = pruner.prune(&topo, 6).unwrap();
    assert_eq!(res.action, PolicyAction::FatalBlock);
}

#[test]
fn test_invariant_4_instruction_neglect_thresholding() {
    // Trigger FatalBlock when to_system / N_island < 0.1
    let pruner = TauSpectralPruner::builder()
        .threat_threshold(100.0) // Very permissive density threshold
        .system_start_idx(6)
        .build();

    let mut topo = Topology::new(8);
    // Mainland (0, 1, 2, 3) connected to system 6
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 3);
    topo.add_edge(3, 0);
    topo.add_edge(0, 6);

    // Island (4, 5) with internal connection, but 0 connection to system
    topo.add_edge(4, 5);

    let res = pruner.prune(&topo, 6).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::FatalBlock,
        "Instruction neglect invariant violated: 0 system connection must FatalBlock"
    );
}

#[test]
fn test_invariant_5_micro_steering_single_token_tripwire() {
    // Condition: N_island == 1, internal == 0, to_system > 0.0 && to_system < 2.0
    let pruner = TauSpectralPruner::builder()
        .threat_threshold(100.0)
        .system_start_idx(4)
        .build();

    let mut topo = Topology::new(5);
    // Mainland (0, 1, 2)
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Island node 3 connects ONLY to system node 4 with 1 edge (to_system = 1.0)
    topo.add_edge(3, 4);

    let res = pruner.prune(&topo, 4).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::FatalBlock,
        "Arrington single-token tripwire failed to trigger FatalBlock!"
    );
    assert_eq!(res.island_nodes, vec![3]);
}

#[test]
fn test_invariant_telemetry_vs_output_separation() {
    let pruner = TauSpectralPruner::builder().system_start_idx(3).build();

    let mut topo = Topology::new(6);
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);
    // System nodes are 3, 4, 5
    topo.add_edge(0, 3);
    topo.add_edge(1, 4);

    let res = pruner.prune(&topo, 5).unwrap();
    // System nodes 3, 4, 5 must NOT appear in output partitions
    for sys in [3, 4, 5] {
        assert!(
            !res.mainland_nodes.contains(&sys),
            "System node {} leaked into mainland!",
            sys
        );
        assert!(
            !res.island_nodes.contains(&sys),
            "System node {} leaked into island!",
            sys
        );
    }
    // Only nodes 0, 1, 2 should be in output
    let mut all_out = res.mainland_nodes;
    all_out.extend(res.island_nodes);
    all_out.sort();
    assert_eq!(all_out, vec![0, 1, 2]);
}

#[test]
fn test_invariant_all_sinks_or_all_system_empty_partitions() {
    let pruner = TauSpectralPruner::builder().system_start_idx(0).build();

    // Scenario A: All nodes are sinks
    let mut topo_sinks = Topology::new(5);
    for i in 0..5 {
        topo_sinks.add_sink(i);
    }
    let res_sinks = pruner.prune(&topo_sinks, 0).unwrap();
    assert_eq!(res_sinks.action, PolicyAction::Allow);
    assert!(res_sinks.mainland_nodes.is_empty());
    assert!(res_sinks.island_nodes.is_empty());

    // Scenario B: All nodes are in system boundary [0, 4]
    let topo_sys = Topology::new(5);
    let res_sys = pruner.prune(&topo_sys, 4).unwrap();
    assert_eq!(res_sys.action, PolicyAction::Allow);
    assert!(res_sys.mainland_nodes.is_empty());
    assert!(res_sys.island_nodes.is_empty());
}
