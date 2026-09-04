//! Empirical Challenger Stress Test Suite for Milestone 2
//! Comprehensive stress-testing and empirical validation of `PrunerWorkspace`,
//! `prune_with_workspace`, spectral gap topologies, and equivalence with `prune`.

use spectral_pruner::engine::{PolicyAction, PrunerWorkspace, TauSpectralPruner, Topology};
use std::collections::BTreeSet;

/// Deterministic 64-bit Linear Congruential Generator (LCG) for reproducible fuzzing
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
}

// =========================================================================
// 1. HIGH-THROUGHPUT STREAMING STRESS TESTS (1,000+ Iterations)
// =========================================================================

#[test]
fn test_streaming_1200_continuous_calls_single_workspace() {
    let mut rng = FuzzRng::new(0xDEADBEEFCAFE0001);
    let mut workspace = PrunerWorkspace::with_capacity(200, 500);
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(2.0)
        .max_iterations(2000)
        .build();

    let mut action_counts = [0usize; 3]; // Allow, GarbageCollect, FatalBlock

    // Run 1,200 continuous streaming calls on varied random topologies
    for iter in 0..1200 {
        let n = rng.gen_range(0, 80);
        let mut topo = Topology::new(n);

        if n > 0 {
            let num_sinks = rng.gen_range(0, n / 4 + 1);
            for _ in 0..num_sinks {
                topo.add_sink(rng.gen_range(0, n));
            }

            let num_edges = rng.gen_range(0, n * 3 + 5);
            for _ in 0..num_edges {
                let u = rng.gen_range(0, n + 5); // allow occasional OOB
                let v = rng.gen_range(0, n + 5);
                topo.edges.push((u, v));
            }
        }

        let sys_len = if n > 5 { rng.gen_range(5, n) } else { 0 };

        // Execute streaming evaluation with workspace
        let res = pruner
            .prune_with_workspace(&topo, sys_len, &mut workspace)
            .expect("Streaming iteration should never fail");

        // Track action distributions
        match res.action {
            PolicyAction::Allow => action_counts[0] += 1,
            PolicyAction::GarbageCollect => action_counts[1] += 1,
            PolicyAction::FatalBlock => action_counts[2] += 1,
        }

        // Verify basic partition invariant: mainland and island are disjoint
        let mainland_set: BTreeSet<usize> = res.mainland_nodes.iter().copied().collect();
        for &node in &res.island_nodes {
            assert!(
                !mainland_set.contains(&node),
                "Iteration {}: Node {} in both mainland and island!",
                iter,
                node
            );
        }

        // Verify no sink nodes are present in output partitions
        for &sink in &topo.sinks {
            assert!(
                !mainland_set.contains(&sink),
                "Iteration {}: Sink node {} found in mainland!",
                iter,
                sink
            );
            assert!(
                !res.island_nodes.contains(&sink),
                "Iteration {}: Sink node {} found in island!",
                iter,
                sink
            );
        }
    }

    assert_eq!(
        action_counts[0] + action_counts[1] + action_counts[2],
        1200,
        "All 1,200 streaming iterations must complete"
    );
    assert!(
        action_counts[0] > 0,
        "Must have Allow actions in streaming run"
    );
}

#[test]
fn test_streaming_workspace_determinism_and_state_isolation() {
    let mut rng = FuzzRng::new(0x1122334455667788);
    let mut ws1 = PrunerWorkspace::new();
    let mut ws2 = PrunerWorkspace::with_capacity(150, 400);
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .max_iterations(1500)
        .build();

    for iter in 0..300 {
        let n = rng.gen_range(3, 60);
        let mut topo = Topology::new(n);

        let num_edges = rng.gen_range(n / 2, n * 3);
        for _ in 0..num_edges {
            let u = rng.gen_range(0, n);
            let v = rng.gen_range(0, n);
            topo.add_edge(u, v);
        }

        let sys_len = if n > 6 { rng.gen_range(5, n - 1) } else { 0 };

        // Run on dynamic workspace ws1
        let res1 = pruner
            .prune_with_workspace(&topo, sys_len, &mut ws1)
            .unwrap();

        // Run on pre-allocated workspace ws2
        let res2 = pruner
            .prune_with_workspace(&topo, sys_len, &mut ws2)
            .unwrap();

        // Re-run on ws1 again to verify idempotence
        let res1_again = pruner
            .prune_with_workspace(&topo, sys_len, &mut ws1)
            .unwrap();

        assert_eq!(
            res1, res2,
            "Iteration {}: Workspace pre-allocation changed output!",
            iter
        );
        assert_eq!(
            res1, res1_again,
            "Iteration {}: Repeated execution with reused workspace non-deterministic!",
            iter
        );
    }
}

#[test]
fn test_streaming_workspace_capacity_preservation_zero_reallocations() {
    // Pre-allocate workspace for max expected size
    let max_nodes = 150;
    let max_edges = 600;
    let mut ws = PrunerWorkspace::with_capacity(max_nodes, max_edges);

    let initial_cap_v = ws.v_vec.capacity();
    let initial_cap_rows = ws.csr_row_ptrs.capacity();
    let initial_cap_cols = ws.csr_col_indices.capacity();

    let pruner = TauSpectralPruner::builder().build();
    let mut rng = FuzzRng::new(0x9988776655443322);

    // Stream 500 topologies strictly within capacity
    for _ in 0..500 {
        let n = rng.gen_range(1, max_nodes);
        let mut topo = Topology::new(n);
        let edge_count = rng.gen_range(0, (n * 3).min(max_edges));
        for _ in 0..edge_count {
            let u = rng.gen_range(0, n);
            let v = rng.gen_range(0, n);
            topo.add_edge(u, v);
        }

        let _ = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();

        // Verify buffer capacities never reallocated / grew beyond initial
        assert_eq!(
            ws.v_vec.capacity(),
            initial_cap_v,
            "v_vec reallocated unexpectedly!"
        );
        assert_eq!(
            ws.csr_row_ptrs.capacity(),
            initial_cap_rows,
            "csr_row_ptrs reallocated unexpectedly!"
        );
        assert!(
            ws.csr_col_indices.capacity() <= initial_cap_cols,
            "csr_col_indices grew unexpectedly!"
        );
    }
}

#[test]
fn test_streaming_workspace_buffer_growth_and_shrinkage_resilience() {
    let mut ws = PrunerWorkspace::new();
    let pruner = TauSpectralPruner::builder().build();

    // Sequence of drastically fluctuating sizes: large -> small -> zero -> large -> single node
    let sizes = [300, 1, 150, 0, 2, 250, 5, 400, 4, 3, 0, 100, 1, 200, 10, 50];

    for (step, &size) in sizes.iter().enumerate() {
        let mut topo = Topology::new(size);
        for i in 0..size.saturating_sub(1) {
            topo.add_edge(i, i + 1);
        }

        let res = pruner
            .prune_with_workspace(&topo, 0, &mut ws)
            .unwrap_or_else(|e| panic!("Step {} size {} failed: {:?}", step, size, e));

        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), size);
    }
}

// =========================================================================
// 2. SPECTRAL GAP STRESS TESTS (Dense Cliques, Stars, Barbells, Disconnected, Cycles)
// =========================================================================

#[test]
fn test_spectral_gap_dense_cliques() {
    let pruner = TauSpectralPruner::builder().tau(0.0).build();
    let mut ws = PrunerWorkspace::new();

    // Test dense cliques K_4, K_8, K_16, K_32, K_64
    for &k in &[4, 8, 16, 32, 64] {
        let mut topo = Topology::new(k);
        for i in 0..k {
            for j in (i + 1)..k {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();

        assert_eq!(res.action, PolicyAction::Allow);
        // A complete clique K_k has theoretical algebraic connectivity lambda_2 = k.
        // Rayleigh quotient should approximate k closely.
        assert!(
            res.connectivity_score > 0.0,
            "Clique K_{} connectivity score should be positive, got {}",
            k,
            res.connectivity_score
        );

        // Complete clique partitions all active nodes into mainland and island
        assert_eq!(
            res.mainland_nodes.len() + res.island_nodes.len(),
            k,
            "Clique K_{} node count conservation mismatch",
            k
        );
        assert!(!res.mainland_nodes.is_empty());
        assert!(!res.island_nodes.is_empty());
    }
}

#[test]
fn test_spectral_gap_all_disconnected_and_isolated_nodes() {
    let pruner = TauSpectralPruner::builder().tau(0.0).build();
    let mut ws = PrunerWorkspace::new();

    // All isolated nodes: max_degree == 0
    for &n in &[3, 10, 50, 200] {
        let topo = Topology::new(n);
        let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();

        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len(), n);
        assert!(res.island_nodes.is_empty());
        assert_eq!(res.connectivity_score, 0.0);
    }

    // Partially disconnected: 1 clique of size 5 + 10 isolated nodes
    let mut topo = Topology::new(15);
    for i in 0..5 {
        for j in (i + 1)..5 {
            topo.add_edge(i, j);
        }
    }
    // Nodes 5..15 are isolated (degree 0)
    let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
    assert_eq!(res.action, PolicyAction::Allow);

    // Arrington Clamping: isolated nodes (d=0) get initialized to +1.0 and
    // must be classified into partitions, never dropped!
    let total_classified = res.mainland_nodes.len() + res.island_nodes.len();
    assert_eq!(
        total_classified, 15,
        "All 15 nodes including isolated nodes must be classified"
    );
}

#[test]
fn test_spectral_gap_star_graphs() {
    let pruner = TauSpectralPruner::builder().tau(0.0).build();
    let mut ws = PrunerWorkspace::new();

    // Star graph S_n with center hub at 0 and leaves at 1..n
    for &n in &[4, 10, 25, 100] {
        let mut topo = Topology::new(n);
        for leaf in 1..n {
            topo.add_edge(0, leaf);
        }

        let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), n);

        // Theoretical algebraic connectivity of a star graph is lambda_2 = 1.0
        assert!(
            res.connectivity_score > 0.0,
            "Star graph S_{} connectivity score should be positive, got {}",
            n,
            res.connectivity_score
        );
    }
}

#[test]
fn test_spectral_gap_barbell_graphs() {
    let pruner = TauSpectralPruner::builder().tau(0.0).build();
    let mut ws = PrunerWorkspace::new();

    // Barbell graph: Clique K1 (size 10: nodes 0..10), Bridge Path (nodes 10..15), Clique K2 (size 10: nodes 15..25)
    let mut topo = Topology::new(25);

    // Clique 1: 0..10
    for i in 0..10 {
        for j in (i + 1)..10 {
            topo.add_edge(i, j);
        }
    }

    // Bridge: 9 -> 10 -> 11 -> 12 -> 13 -> 14 -> 15
    for i in 9..15 {
        topo.add_edge(i, i + 1);
    }

    // Clique 2: 15..25
    for i in 15..25 {
        for j in (i + 1)..25 {
            topo.add_edge(i, j);
        }
    }

    let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();

    assert_eq!(res.action, PolicyAction::Allow);
    assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 25);

    // Barbell graph has a pronounced spectral bottleneck (small algebraic connectivity)
    // Fiedler vector cleanly separates the two large cliques across the bridge
    let mainland_has_k1 = res.mainland_nodes.iter().filter(|&&x| x < 9).count();
    let mainland_has_k2 = res.mainland_nodes.iter().filter(|&&x| x >= 16).count();

    // One clique must be predominantly in mainland, the other in island
    assert!(
        (mainland_has_k1 >= 8 && mainland_has_k2 <= 2)
            || (mainland_has_k2 >= 8 && mainland_has_k1 <= 2),
        "Barbell graph spectral bisection failed to separate the two cliques: K1 in mainland={}, K2 in mainland={}",
        mainland_has_k1,
        mainland_has_k2
    );
}

#[test]
fn test_spectral_gap_cycle_and_path_graphs() {
    let pruner = TauSpectralPruner::builder().tau(0.0).build();
    let mut ws = PrunerWorkspace::new();

    // Cycle graph C_n (ring)
    for &n in &[6, 12, 30, 60] {
        let mut topo = Topology::new(n);
        for i in 0..n {
            topo.add_edge(i, (i + 1) % n);
        }

        let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), n);

        // Cycle graph bisection produces two balanced halves
        let diff = (res.mainland_nodes.len() as isize - res.island_nodes.len() as isize).abs();
        assert!(
            diff <= 2,
            "Cycle C_{} partition imbalance too high: mainland={}, island={}",
            n,
            res.mainland_nodes.len(),
            res.island_nodes.len()
        );
    }

    // Path graph P_n (linear chain)
    for &n in &[5, 15, 50] {
        let mut topo = Topology::new(n);
        for i in 0..n - 1 {
            topo.add_edge(i, i + 1);
        }

        let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), n);
    }
}

// =========================================================================
// 3. EXACT PARITY TESTS: prune vs prune_with_workspace (500+ Topologies)
// =========================================================================

#[test]
fn test_exact_parity_500_randomized_topologies() {
    let mut rng = FuzzRng::new(0xCAFEF00D12345678);
    let mut workspace = PrunerWorkspace::new();

    let pruners = [
        TauSpectralPruner::builder()
            .tau(0.0)
            .threat_threshold(2.0)
            .max_iterations(1000)
            .system_start_idx(5)
            .build(),
        TauSpectralPruner::builder()
            .tau(0.1)
            .threat_threshold(1.5)
            .max_iterations(1000)
            .system_start_idx(3)
            .build(),
        TauSpectralPruner::builder()
            .tau(-0.1)
            .threat_threshold(3.0)
            .momentum_beta(0.3)
            .max_iterations(1000)
            .system_start_idx(4)
            .build(),
    ];

    for (cfg_idx, pruner) in pruners.iter().enumerate() {
        for iter in 0..500 {
            let n = rng.gen_range(0, 60);
            let mut topo = Topology::new(n);

            if n > 0 {
                // Add sinks
                let sink_count = rng.gen_range(0, n / 3 + 1);
                for _ in 0..sink_count {
                    topo.add_sink(rng.gen_range(0, n));
                }

                // Add edges (mix of sparse, dense, multigraph, self-loops, OOB)
                let edge_count = rng.gen_range(0, n * 2 + 4);
                for _ in 0..edge_count {
                    let u = rng.gen_range(0, n + 5);
                    let v = rng.gen_range(0, n + 5);
                    topo.edges.push((u, v));
                }
            }

            let sys_len = if n > 5 { rng.gen_range(0, n) } else { 0 };

            // 1. Evaluate with direct internal allocation
            let res_direct = pruner
                .prune(&topo, sys_len)
                .expect("Direct prune should not fail");

            // 2. Evaluate with reused workspace
            let res_ws = pruner
                .prune_with_workspace(&topo, sys_len, &mut workspace)
                .expect("Workspace prune should not fail");

            // Assert exact structural parity
            assert_eq!(
                res_direct.action, res_ws.action,
                "Config {}, Iteration {}: Action mismatch! direct={:?}, ws={:?}",
                cfg_idx, iter, res_direct.action, res_ws.action
            );

            assert_eq!(
                res_direct.mainland_nodes, res_ws.mainland_nodes,
                "Config {}, Iteration {}: Mainland nodes mismatch!",
                cfg_idx, iter
            );

            assert_eq!(
                res_direct.island_nodes, res_ws.island_nodes,
                "Config {}, Iteration {}: Island nodes mismatch!",
                cfg_idx, iter
            );

            let score_diff = (res_direct.connectivity_score - res_ws.connectivity_score).abs();
            assert!(
                score_diff < 1e-10
                    || (res_direct.connectivity_score.is_nan()
                        && res_ws.connectivity_score.is_nan()),
                "Config {}, Iteration {}: Connectivity score mismatch! direct={}, ws={}, diff={}",
                cfg_idx,
                iter,
                res_direct.connectivity_score,
                res_ws.connectivity_score,
                score_diff
            );
        }
    }
}

// =========================================================================
// 4. SECURITY INVARIANT & POLICY THREAT TESTS (AGENTS.md Directives)
// =========================================================================

#[test]
fn test_security_invariant_single_token_tripwire_exact_trigger() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(5)
        .build();
    let mut ws = PrunerWorkspace::new();

    // Island has exactly N=1 (node 3), internal edges = 0, to_system = 1 (edge 3 -> 5)
    let mut topo = Topology::new(6);
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);
    topo.add_edge(3, 5); // Micro-steering connection to system space (node 5)

    let res = pruner.prune_with_workspace(&topo, 5, &mut ws).unwrap();

    assert_eq!(
        res.action,
        PolicyAction::FatalBlock,
        "Single-token tripwire should trigger FATAL_BLOCK"
    );
    assert_eq!(res.island_nodes, vec![3]);
    // System node 5 is excluded; nodes 0, 1, 2, 4 are classified into mainland
    assert_eq!(res.mainland_nodes, vec![0, 1, 2, 4]);
}

#[test]
fn test_security_invariant_instruction_neglect_threshold() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(10)
        .build();
    let mut ws = PrunerWorkspace::new();

    // Create a cluster of nodes with 0 connections to system space
    // to_system / N_island = 0.0 < 0.1 => FATAL_BLOCK
    let mut topo = Topology::new(15);
    // Mainland
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Island cluster 3..10
    for i in 3..10 {
        for j in (i + 1)..10 {
            topo.add_edge(i, j);
        }
    }

    let res = pruner.prune_with_workspace(&topo, 12, &mut ws).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::FatalBlock,
        "Instruction neglect should trigger FATAL_BLOCK"
    );
}

#[test]
fn test_security_invariant_telemetry_separation() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(3)
        .build();
    let mut ws = PrunerWorkspace::new();

    let mut topo = Topology::new(6);
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);
    topo.add_edge(0, 3); // connection to system node 3
    topo.add_edge(1, 4); // connection to system node 4

    let res = pruner.prune_with_workspace(&topo, 4, &mut ws).unwrap();

    // Nodes 3 and 4 are in system range [3, 4] and must be filtered out of final outputs
    for &node in &res.mainland_nodes {
        assert!(
            !(3..=4).contains(&node),
            "System node {} found in mainland output!",
            node
        );
    }
    for &node in &res.island_nodes {
        assert!(
            !(3..=4).contains(&node),
            "System node {} found in island output!",
            node
        );
    }
}
