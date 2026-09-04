//! 🧪 E2E Tier 2: Boundary & Extreme Topological Invariant Test Suite
//!
//! Comprehensive verification of spectral eigensolver and partitioning behaviors
//! under extreme graph topologies, numerical limits, degenerate states, and boundary conditions.
//!
//! Zero external test dependencies: pure Rust stdlib.

use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};

// =========================================================================
// 1. Empty & Minimal Graphs (N in [0, 1, 2])
// =========================================================================
mod minimal_graphs {
    use super::*;

    #[test]
    fn test_boundary_n0_various_system_boundaries() {
        let pruner = TauSpectralPruner::builder().build();
        let topo = Topology::new(0);

        for sys_len in [0, 1, 5, 100] {
            let res = pruner.prune(&topo, sys_len).unwrap();
            assert_eq!(
                res.action,
                if sys_len == 0 {
                    PolicyAction::Allow
                } else {
                    PolicyAction::FatalBlock
                }
            );
            assert!(res.mainland_nodes.is_empty());
            assert!(res.island_nodes.is_empty());
            assert_eq!(res.connectivity_score, 0.0);
        }
    }

    #[test]
    fn test_boundary_n1_isolated_and_sink() {
        let pruner = TauSpectralPruner::builder().build();

        // N=1 active
        let topo_active = Topology::new(1);
        let res_act = pruner.prune(&topo_active, 0).unwrap();
        assert_eq!(res_act.action, PolicyAction::Allow);
        assert_eq!(res_act.mainland_nodes, vec![0]);
        assert!(res_act.island_nodes.is_empty());

        // N=1 as sink
        let mut topo_sink = Topology::new(1);
        topo_sink.add_sink(0);
        let res_sink = pruner.prune(&topo_sink, 0).unwrap();
        assert_eq!(res_sink.action, PolicyAction::Allow);
        assert!(res_sink.mainland_nodes.is_empty());
        assert!(res_sink.island_nodes.is_empty());

        // N=1 as system node (system_start_idx=0, boundary_len=1)
        let pruner_sys0 = TauSpectralPruner::builder().system_start_idx(0).build();
        let res_sys = pruner_sys0.prune(&topo_active, 1).unwrap();
        assert_eq!(res_sys.action, PolicyAction::Allow);
        assert!(res_sys.mainland_nodes.is_empty());
        assert!(res_sys.island_nodes.is_empty());
    }

    #[test]
    fn test_boundary_n2_bridge_sink_and_system() {
        let pruner = TauSpectralPruner::builder().build();

        // N=2 with edge
        let mut topo_edge = Topology::new(2);
        topo_edge.add_edge(0, 1);
        let res1 = pruner.prune(&topo_edge, 0).unwrap();
        assert_eq!(res1.action, PolicyAction::Allow);
        assert_eq!(res1.mainland_nodes, vec![0, 1]);

        // N=2 without edge
        let topo_no_edge = Topology::new(2);
        let res2 = pruner.prune(&topo_no_edge, 0).unwrap();
        assert_eq!(res2.action, PolicyAction::Allow);
        assert_eq!(res2.mainland_nodes, vec![0, 1]);

        // N=2 with one sink
        let mut topo_sink = Topology::new(2);
        topo_sink.add_edge(0, 1);
        topo_sink.add_sink(1);
        let res3 = pruner.prune(&topo_sink, 0).unwrap();
        assert_eq!(res3.action, PolicyAction::Allow);
        assert_eq!(res3.mainland_nodes, vec![0]);
        assert!(!res3.mainland_nodes.contains(&1));
    }
}

// =========================================================================
// 2. Disconnected & Isolated Extremes (Arrington Clamping at Scale)
// =========================================================================
mod disconnected_extremes {
    use super::*;

    #[test]
    fn test_boundary_n100_all_isolated_nodes() {
        let pruner = TauSpectralPruner::builder().build();
        let topo = Topology::new(100); // 100 isolated nodes, degree=0

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len(), 100);
        assert!(res.island_nodes.is_empty());
        assert_eq!(res.connectivity_score, 0.0);
    }

    #[test]
    fn test_boundary_half_isolated_half_clique() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(100);

        // Nodes 0..50 form a dense clique
        for i in 0..50 {
            for j in (i + 1)..50 {
                topo.add_edge(i, j);
            }
        }
        // Nodes 50..100 are completely isolated (degree == 0)

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);

        let total_classified = res.mainland_nodes.len() + res.island_nodes.len();
        assert_eq!(total_classified, 100);

        // All 50 isolated nodes must be safely retained and classified (zero node dropping)
        for iso in 50..100 {
            assert!(
                res.mainland_nodes.contains(&iso) || res.island_nodes.contains(&iso),
                "Isolated node {} must be classified",
                iso
            );
        }
    }
}

// =========================================================================
// 3. Extreme Degree Topologies (Massive Stars & Double Stars)
// =========================================================================
mod extreme_degrees {
    use super::*;

    #[test]
    fn test_boundary_massive_star_graph_n1000() {
        let pruner = TauSpectralPruner::builder().max_iterations(200).build();
        let mut topo = Topology::new(1000);

        // Hub at 0, 999 leaves at 1..1000
        for leaf in 1..1000 {
            topo.add_edge(0, leaf);
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 1000);
        assert!(!res.mainland_nodes.is_empty());
        assert!(!res.island_nodes.is_empty());
        assert!(res.connectivity_score > 0.0);
    }

    #[test]
    fn test_boundary_double_star_graph() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(100);

        // Hub A: 0, leaves 1..50
        for i in 1..50 {
            topo.add_edge(0, i);
        }
        // Hub B: 50, leaves 51..100
        for i in 51..100 {
            topo.add_edge(50, i);
        }
        // Bridge between hubs
        topo.add_edge(0, 50);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 100);
    }
}

// =========================================================================
// 4. Dense Cliques (K_100, K_200, K_300)
// =========================================================================
mod dense_cliques {
    use super::*;

    #[test]
    fn test_boundary_dense_clique_k100() {
        let pruner = TauSpectralPruner::builder().max_iterations(100).build();
        let n = 100;
        let mut topo = Topology::new(n);
        for i in 0..n {
            for j in (i + 1)..n {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), n);
        assert!(res.connectivity_score > 50.0);
    }

    #[test]
    fn test_boundary_dense_clique_k200() {
        let pruner = TauSpectralPruner::builder().max_iterations(50).build();
        let n = 200;
        let mut topo = Topology::new(n);
        for i in 0..n {
            for j in (i + 1)..n {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), n);
    }

    #[test]
    fn test_boundary_dense_clique_k300() {
        let pruner = TauSpectralPruner::builder().max_iterations(30).build();
        let n = 300;
        let mut topo = Topology::new(n);
        for i in 0..n {
            for j in (i + 1)..n {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), n);
    }
}

// =========================================================================
// 5. Linear Paths & Cycles
// =========================================================================
mod paths_and_cycles {
    use super::*;

    #[test]
    fn test_boundary_long_path_graph_n200() {
        let pruner = TauSpectralPruner::builder().max_iterations(200).build();
        let n = 200;
        let mut topo = Topology::new(n);
        for i in 0..n - 1 {
            topo.add_edge(i, i + 1);
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), n);
        assert!(res.connectivity_score > 0.0);
    }

    #[test]
    fn test_boundary_large_cycle_graph_n200() {
        let pruner = TauSpectralPruner::builder().max_iterations(200).build();
        let n = 200;
        let mut topo = Topology::new(n);
        for i in 0..n {
            topo.add_edge(i, (i + 1) % n);
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), n);
    }
}

// =========================================================================
// 6. Barbell & Bottleneck Graphs
// =========================================================================
mod barbells_and_bottlenecks {
    use super::*;

    #[test]
    fn test_boundary_barbell_k50_bridge_k50() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(100);

        // Bell 1: 0..50 clique
        for i in 0..50 {
            for j in (i + 1)..50 {
                topo.add_edge(i, j);
            }
        }
        // Bell 2: 50..100 clique
        for i in 50..100 {
            for j in (i + 1)..100 {
                topo.add_edge(i, j);
            }
        }
        // Bridge between node 49 and node 50
        topo.add_edge(49, 50);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        // Fiedler cut cleanly splits the two bells
        assert_eq!(res.mainland_nodes.len(), 50);
        assert_eq!(res.island_nodes.len(), 50);
    }

    #[test]
    fn test_boundary_barbell_long_path_bridge() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(90);

        // Bell 1: 0..20 clique
        for i in 0..20 {
            for j in (i + 1)..20 {
                topo.add_edge(i, j);
            }
        }
        // Path bridge: 19 - 20 - 21 - ... - 69 - 70
        for i in 19..70 {
            topo.add_edge(i, i + 1);
        }
        // Bell 2: 70..90 clique
        for i in 70..90 {
            for j in (i + 1)..90 {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 90);
    }
}

// =========================================================================
// 7. System Boundary Extremes
// =========================================================================
mod system_boundary_extremes {
    use super::*;

    #[test]
    fn test_boundary_system_start_idx_zero() {
        let pruner = TauSpectralPruner::builder().system_start_idx(0).build();
        let mut topo = Topology::new(10);
        for i in 0..9 {
            topo.add_edge(i, i + 1);
        }

        let res = pruner.prune(&topo, 5).unwrap();
        // System nodes are [0..=5]. Only local nodes [6, 7, 8, 9] should be in output
        for &node in &res.mainland_nodes {
            assert!(node > 5);
        }
        for &node in &res.island_nodes {
            assert!(node > 5);
        }
    }

    #[test]
    fn test_boundary_system_boundary_len_equals_n() {
        let pruner = TauSpectralPruner::builder().system_start_idx(0).build();
        let mut topo = Topology::new(8);
        for i in 0..7 {
            topo.add_edge(i, i + 1);
        }

        let res = pruner.prune(&topo, 8).unwrap();
        // Entire graph is system domain -> output partitions are empty
        assert_eq!(res.action, PolicyAction::Allow);
        assert!(res.mainland_nodes.is_empty());
        assert!(res.island_nodes.is_empty());
    }

    #[test]
    fn test_boundary_system_boundary_len_greater_than_n() {
        let pruner = TauSpectralPruner::builder().system_start_idx(2).build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);

        let res = pruner.prune(&topo, 100).unwrap();
        // Nodes >= 2 are system nodes. Nodes 0, 1 are local
        assert!(res.mainland_nodes.contains(&0) || res.island_nodes.contains(&0));
        assert!(res.mainland_nodes.contains(&1) || res.island_nodes.contains(&1));
        assert!(!res.mainland_nodes.contains(&2));
        assert!(!res.island_nodes.contains(&2));
    }

    #[test]
    fn test_boundary_inverted_system_start_greater_than_len() {
        let pruner = TauSpectralPruner::builder().system_start_idx(10).build();
        let mut topo = Topology::new(8);
        for i in 0..7 {
            topo.add_edge(i, i + 1);
        }

        let res = pruner.prune(&topo, 5).unwrap();
        // system_start_idx (10) > boundary_len (5) -> No nodes match is_system_node
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 8);
    }
}

// =========================================================================
// 8. Sink Distribution Extremes
// =========================================================================
mod sink_extremes {
    use super::*;

    #[test]
    fn test_boundary_all_nodes_are_sinks() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(10);
        for i in 0..10 {
            topo.add_sink(i);
        }
        topo.add_edge(0, 1);
        topo.add_edge(2, 3);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert!(res.mainland_nodes.is_empty());
        assert!(res.island_nodes.is_empty());
    }

    #[test]
    fn test_boundary_alternating_sinks() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(20);
        for i in 0..19 {
            topo.add_edge(i, i + 1);
        }
        // Mark every odd node as sink
        for i in (1..20).step_by(2) {
            topo.add_sink(i);
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);

        // Only even nodes (0, 2, 4, ..., 18) remain
        let mut all_output = res.mainland_nodes.clone();
        all_output.extend_from_slice(&res.island_nodes);
        all_output.sort();

        let expected: Vec<usize> = (0..20).filter(|i| i % 2 == 0).collect();
        assert_eq!(all_output, expected);
    }
}

// =========================================================================
// 9. Numerical & Floating Point Limits
// =========================================================================
mod numerical_limits {
    use super::*;

    #[test]
    fn test_boundary_extreme_tolerances() {
        let tols = [1e-15, 1e-12, 1e-6, 1e-2];
        let mut topo = Topology::new(6);
        for i in 0..5 {
            topo.add_edge(i, i + 1);
        }

        for &tol in &tols {
            let pruner = TauSpectralPruner::builder().tolerance(tol).build();
            let res = pruner.prune(&topo, 0).unwrap();
            assert_eq!(res.action, PolicyAction::Allow);
            assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 6);
        }
    }

    #[test]
    fn test_boundary_extreme_max_iterations() {
        let iters = [1, 2, 5, 50, 10_000];
        let mut topo = Topology::new(6);
        for i in 0..5 {
            topo.add_edge(i, i + 1);
        }

        for &iter in &iters {
            let pruner = TauSpectralPruner::builder().max_iterations(iter).build();
            let res = pruner.prune(&topo, 0).unwrap();
            assert_eq!(res.action, PolicyAction::Allow);
        }
    }

    #[test]
    fn test_boundary_extreme_momentum_beta() {
        let betas = [0.0, 0.0001, 0.5, 0.999];
        let mut topo = Topology::new(8);
        for i in 0..7 {
            topo.add_edge(i, i + 1);
        }

        for &beta in &betas {
            let pruner = TauSpectralPruner::builder().momentum_beta(beta).build();
            let res = pruner.prune(&topo, 0).unwrap();
            assert_eq!(res.action, PolicyAction::Allow);
        }
    }

    #[test]
    fn test_boundary_extreme_tau_values() {
        let taus = [-1e6, -100.0, -1.0, 0.0, 1.0, 100.0, 1e6];
        let mut topo = Topology::new(8);
        for i in 0..7 {
            topo.add_edge(i, i + 1);
        }

        for &tau in &taus {
            let pruner = TauSpectralPruner::builder().tau(tau).build();
            let res = pruner.prune(&topo, 0).unwrap();
            assert_eq!(res.action, PolicyAction::Allow);
            assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 8);
        }
    }
}
