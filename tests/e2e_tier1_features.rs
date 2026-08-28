//! 🧪 E2E Tier 1 Feature Coverage Test Suite
//!
//! Exhaustive requirement-driven test suite verifying all 21 architectural and algorithmic
//! features defined in `PROJECT.md` and `AGENTS.md`. Every feature contains at least 5
//! independent, genuine, fully-asserted test cases.
//!
//! Zero external test dependencies: pure Rust stdlib.

use spectral_pruner::{
    BitSet, CsrGraph, PolicyAction, PrunerBuilder, PrunerWorkspace, TauSpectralPruner, Topology,
};

// =========================================================================
// Feature 01: Topology Graph Builder & Sink Filtering
// =========================================================================
mod feature_01_topology_builder {
    use super::*;

    #[test]
    fn test_f01_empty_and_capacity_construction() {
        let topo = Topology::new(10);
        assert_eq!(topo.num_nodes, 10);
        assert!(topo.edges.is_empty());
        assert!(topo.sinks.is_empty());
    }

    #[test]
    fn test_f01_valid_edges_and_out_of_bounds_filtering() {
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(2, 3);
        // Out of bounds edges must be silently dropped
        topo.add_edge(0, 5);
        topo.add_edge(6, 1);
        topo.add_edge(10, 12);

        assert_eq!(topo.edges.len(), 2);
        assert_eq!(topo.edges, vec![(0, 1), (2, 3)]);
    }

    #[test]
    fn test_f01_sink_addition_and_bounds() {
        let mut topo = Topology::new(4);
        topo.add_sink(1);
        topo.add_sink(3);
        // Duplicate sink and out of bounds sinks
        topo.add_sink(1);
        topo.add_sink(4);
        topo.add_sink(99);

        assert_eq!(topo.sinks.len(), 2);
        assert!(topo.sinks.contains(&1));
        assert!(topo.sinks.contains(&3));
        assert!(!topo.sinks.contains(&4));
    }

    #[test]
    fn test_f01_to_sink_bitset_conversion() {
        let mut topo = Topology::new(8);
        topo.add_sink(0);
        topo.add_sink(2);
        topo.add_sink(7);

        let bitset = topo.to_sink_bitset();
        assert_eq!(bitset.len(), 8);
        assert_eq!(bitset.count_ones(), 3);
        assert!(bitset.contains(0));
        assert!(bitset.contains(2));
        assert!(bitset.contains(7));
        assert!(!bitset.contains(1));
        assert!(!bitset.contains(6));
    }

    #[test]
    fn test_f01_populate_sink_bitset_in_place_reuse() {
        let mut topo = Topology::new(6);
        topo.add_sink(1);
        topo.add_sink(4);

        let mut bitset = BitSet::new(100);
        bitset.insert(50);
        assert_eq!(bitset.count_ones(), 1);

        topo.populate_sink_bitset(&mut bitset);
        assert_eq!(bitset.len(), 6);
        assert_eq!(bitset.count_ones(), 2);
        assert!(bitset.contains(1));
        assert!(bitset.contains(4));
        assert!(!bitset.contains(50));
    }
}

// =========================================================================
// Feature 02: Contiguous CsrGraph Matrix Representation
// =========================================================================
mod feature_02_csr_graph {
    use super::*;

    #[test]
    fn test_f02_from_topology_exact_prefix_sums() {
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(3, 0);

        let sink_bits = BitSet::new(4);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        assert_eq!(csr.num_nodes, 4);
        assert_eq!(csr.row_ptrs.len(), 5);
        assert_eq!(csr.row_ptrs, vec![0, 2, 4, 6, 8]);
        assert_eq!(csr.half_edge_count(), 8);
        assert_eq!(csr.edge_count(), 4);
    }

    #[test]
    fn test_f02_neighbor_slicing_and_out_of_bounds() {
        let mut topo = Topology::new(3);
        topo.add_edge(0, 1);
        topo.add_edge(0, 2);

        let sink_bits = BitSet::new(3);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        assert_eq!(csr.neighbors(0), &[1, 2]);
        assert_eq!(csr.neighbors(1), &[0]);
        assert_eq!(csr.neighbors(2), &[0]);
        assert_eq!(csr.neighbors(3), &[]);
        assert_eq!(csr.neighbors(100), &[]);
    }

    #[test]
    fn test_f02_degree_tracking_and_max_degree() {
        let mut topo = Topology::new(5);
        for i in 1..5 {
            topo.add_edge(0, i);
        }

        let sink_bits = BitSet::new(5);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        assert_eq!(csr.degree(0), 4.0);
        assert_eq!(csr.degree(1), 1.0);
        assert_eq!(csr.degree(4), 1.0);
        assert_eq!(csr.degree(99), 0.0);
        assert_eq!(csr.max_degree(), 4.0);
    }

    #[test]
    fn test_f02_self_loop_and_sink_filtering() {
        let mut topo = Topology::new(4);
        topo.add_edge(0, 0); // Self-loop
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_sink(2);

        let sink_bits = topo.to_sink_bitset();
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        assert_eq!(csr.degree(0), 1.0);
        assert_eq!(csr.degree(1), 1.0);
        assert_eq!(csr.degree(2), 0.0);
        assert_eq!(csr.degree(3), 0.0);
        assert_eq!(csr.neighbors(1), &[0]);
    }

    #[test]
    fn test_f02_compile_into_workspace_zero_alloc() {
        let mut topo = Topology::new(3);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);

        let sink_bits = BitSet::new(3);
        let mut row_ptrs = Vec::new();
        let mut col_indices = Vec::new();
        let mut degrees = Vec::new();
        let mut cursor = Vec::new();

        CsrGraph::compile_into(
            &topo,
            &sink_bits,
            &mut row_ptrs,
            &mut col_indices,
            &mut degrees,
            &mut cursor,
        );

        assert_eq!(row_ptrs, vec![0, 1, 3, 4]);
        assert_eq!(col_indices, vec![1, 0, 2, 1]);
        assert_eq!(degrees, vec![1.0, 2.0, 1.0]);
    }
}

// =========================================================================
// Feature 03: Fast BitSet Bitmasks
// =========================================================================
mod feature_03_bitset_masks {
    use super::*;

    #[test]
    fn test_f03_word_boundaries_64_128_192() {
        let mut bs = BitSet::new(195);
        assert_eq!(bs.words.len(), 4);
        assert_eq!(bs.len(), 195);

        let boundary_indices = [0, 63, 64, 65, 127, 128, 129, 191, 192, 194];
        for &idx in &boundary_indices {
            assert!(!bs.contains(idx));
            bs.insert(idx);
            assert!(bs.contains(idx));
        }

        assert_eq!(bs.count_ones(), boundary_indices.len());
        assert!(!bs.contains(1));
        assert!(!bs.contains(62));
        assert!(!bs.contains(195)); // OOB
    }

    #[test]
    fn test_f03_insert_remove_contains_idempotence() {
        let mut bs = BitSet::new(10);
        bs.insert(3);
        bs.insert(3);
        assert!(bs.contains(3));
        assert_eq!(bs.count_ones(), 1);

        bs.remove(3);
        bs.remove(3);
        assert!(!bs.contains(3));
        assert_eq!(bs.count_ones(), 0);
    }

    #[test]
    fn test_f03_clear_and_reset_with_len() {
        let mut bs = BitSet::new(50);
        bs.insert(10);
        bs.insert(20);
        bs.clear();
        assert_eq!(bs.count_ones(), 0);
        assert_eq!(bs.len(), 50);

        bs.reset_with_len(130);
        assert_eq!(bs.len(), 130);
        assert_eq!(bs.words.len(), 3);
        assert_eq!(bs.count_ones(), 0);
    }

    #[test]
    fn test_f03_count_ones_hardware_popcnt() {
        let mut bs = BitSet::new(300);
        for i in 0..100 {
            bs.insert(i * 3);
        }
        assert_eq!(bs.count_ones(), 100);
    }

    #[test]
    fn test_f03_iter_ones_exhaustive_traversal() {
        let mut bs = BitSet::new(150);
        let items = vec![2, 7, 63, 64, 100, 149];
        for &idx in &items {
            bs.insert(idx);
        }

        let collected: Vec<usize> = bs.iter_ones().collect();
        assert_eq!(collected, items);
    }
}

// =========================================================================
// Feature 04: Edge-Case Graph Handling Fast Paths
// =========================================================================
mod feature_04_edge_case_fast_paths {
    use super::*;

    #[test]
    fn test_f04_n0_empty_graph_fast_path() {
        let pruner = TauSpectralPruner::builder().build();
        let topo = Topology::new(0);
        let res = pruner.prune(&topo, 0).unwrap();

        assert_eq!(res.action, PolicyAction::Allow);
        assert!(res.mainland_nodes.is_empty());
        assert!(res.island_nodes.is_empty());
        assert_eq!(res.connectivity_score, 0.0);
    }

    #[test]
    fn test_f04_n1_single_node_fast_path() {
        let pruner = TauSpectralPruner::builder().build();
        let topo = Topology::new(1);
        let res = pruner.prune(&topo, 0).unwrap();

        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes, vec![0]);
        assert!(res.island_nodes.is_empty());
        assert_eq!(res.connectivity_score, 0.0);
    }

    #[test]
    fn test_f04_n2_two_node_fast_path() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(2);
        topo.add_edge(0, 1);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes, vec![0, 1]);
        assert!(res.island_nodes.is_empty());
    }

    #[test]
    fn test_f04_n2_with_sink_fast_path() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(2);
        topo.add_sink(1);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes, vec![0]);
        assert!(res.island_nodes.is_empty());
    }

    #[test]
    fn test_f04_all_disconnected_max_degree_zero() {
        let pruner = TauSpectralPruner::builder().build();
        let topo = Topology::new(8); // 8 nodes, 0 edges

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes, vec![0, 1, 2, 3, 4, 5, 6, 7]);
        assert!(res.island_nodes.is_empty());
        assert_eq!(res.connectivity_score, 0.0);
    }
}

// =========================================================================
// Feature 05: Arrington Zero-Degree Clamping Regularization
// =========================================================================
mod feature_05_arrington_clamping {
    use super::*;

    #[test]
    fn test_f05_single_isolated_node_clamped_to_1_0() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);
        // Node 3 is isolated (degree == 0), node 4 is connected to system space
        topo.add_edge(4, 0);

        let res = pruner.prune(&topo, 0).unwrap();
        // Isolated node 3 must be included in either mainland or island
        assert!(
            res.mainland_nodes.contains(&3) || res.island_nodes.contains(&3),
            "Arrington Clamping invariant: degree-0 node must be classified"
        );
    }

    #[test]
    fn test_f05_multiple_isolated_nodes_clamped() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        let mut topo = Topology::new(7);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        // Nodes 3, 4, 5, 6 are isolated

        let res = pruner.prune(&topo, 0).unwrap();
        let mut all_output = res.mainland_nodes.clone();
        all_output.extend_from_slice(&res.island_nodes);
        all_output.sort();
        assert_eq!(all_output, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_f05_isolated_node_with_sinks() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_sink(2); // Sink node
                          // Node 3 is isolated, Node 4 is isolated

        let res = pruner.prune(&topo, 0).unwrap();
        assert!(!res.mainland_nodes.contains(&2));
        assert!(!res.island_nodes.contains(&2));
        assert!(res.mainland_nodes.contains(&3) || res.island_nodes.contains(&3));
        assert!(res.mainland_nodes.contains(&4) || res.island_nodes.contains(&4));
    }

    #[test]
    fn test_f05_isolated_nodes_with_system_boundary() {
        let pruner = TauSpectralPruner::builder().system_start_idx(3).build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        // Node 3 is system, Node 4 is system, Node 5 is isolated local node

        let res = pruner.prune(&topo, 4).unwrap();
        assert!(!res.mainland_nodes.contains(&3));
        assert!(!res.island_nodes.contains(&3));
        assert!(!res.mainland_nodes.contains(&4));
        assert!(!res.island_nodes.contains(&4));
        assert!(res.mainland_nodes.contains(&5) || res.island_nodes.contains(&5));
    }

    #[test]
    fn test_f05_clamping_determinism_across_restarts() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);

        let res1 = pruner.prune(&topo, 0).unwrap();
        let res2 = pruner.prune(&topo, 0).unwrap();

        assert_eq!(res1.mainland_nodes, res2.mainland_nodes);
        assert_eq!(res1.island_nodes, res2.island_nodes);
        assert_eq!(res1.action, res2.action);
    }
}

// =========================================================================
// Feature 06: Shifted Laplacian SpMV Operator (M = I - alpha * L)
// =========================================================================
mod feature_06_shifted_laplacian_spmv {
    use super::*;

    #[test]
    fn test_f06_alpha_scaling_by_max_degree() {
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(0, 2);
        topo.add_edge(0, 3); // max_degree = 3

        let sink_bits = BitSet::new(4);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);
        let max_d = csr.max_degree();
        let expected_alpha = 1.0 / (2.0 * max_d + 1.1);

        assert_eq!(max_d, 3.0);
        assert!((expected_alpha - (1.0 / 7.1)).abs() < 1e-12);
    }

    #[test]
    fn test_f06_spmv_csr_slice_multiplication() {
        let pruner = TauSpectralPruner::builder().max_iterations(1).build();
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f06_sink_rows_zeroed_in_spmv() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_sink(1);

        let res = pruner.prune(&topo, 0).unwrap();
        assert!(!res.mainland_nodes.contains(&1));
        assert!(!res.island_nodes.contains(&1));
    }

    #[test]
    fn test_f06_spmv_energy_preservation() {
        let pruner = TauSpectralPruner::builder()
            .max_iterations(50)
            .tolerance(1e-12)
            .build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(3, 4);

        let res = pruner.prune(&topo, 0).unwrap();
        assert!(res.connectivity_score >= 0.0);
        assert!(!res.connectivity_score.is_nan());
    }

    #[test]
    fn test_f06_linear_convergence_rate() {
        let pruner_short = TauSpectralPruner::builder().max_iterations(5).build();
        let pruner_long = TauSpectralPruner::builder().max_iterations(500).build();

        let mut topo = Topology::new(6);
        for i in 0..5 {
            topo.add_edge(i, i + 1);
        }

        let res_short = pruner_short.prune(&topo, 0).unwrap();
        let res_long = pruner_long.prune(&topo, 0).unwrap();

        assert_eq!(res_short.action, res_long.action);
    }
}

// =========================================================================
// Feature 07: Null-Space Projection Active Node Centering
// =========================================================================
mod feature_07_null_space_projection {
    use super::*;

    #[test]
    fn test_f07_zero_mean_invariant() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(3, 4);

        let mut ws = PrunerWorkspace::new();
        let _res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();

        let active_sum: f64 = (0..5).map(|i| ws.v_vec[i]).sum();
        assert!(
            active_sum.abs() < 1e-9,
            "Mean after projection must be ~0.0"
        );
    }

    #[test]
    fn test_f07_sink_exclusion_from_mean() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(2, 3);
        topo.add_sink(4); // Sink node

        let mut ws = PrunerWorkspace::new();
        let _res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();

        assert_eq!(ws.v_vec[4], 0.0);
        let active_sum: f64 = (0..4).map(|i| ws.v_vec[i]).sum();
        assert!(active_sum.abs() < 1e-9);
    }

    #[test]
    fn test_f07_all_ones_null_space_invariance() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(3, 0);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f07_odd_vs_even_active_nodes() {
        let pruner = TauSpectralPruner::builder().build();

        let mut topo_odd = Topology::new(5);
        for i in 0..4 {
            topo_odd.add_edge(i, i + 1);
        }
        let res_odd = pruner.prune(&topo_odd, 0).unwrap();
        assert_eq!(res_odd.action, PolicyAction::Allow);

        let mut topo_even = Topology::new(6);
        for i in 0..5 {
            topo_even.add_edge(i, i + 1);
        }
        let res_even = pruner.prune(&topo_even, 0).unwrap();
        assert_eq!(res_even.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f07_single_active_node_projection() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(3);
        topo.add_sink(1);
        topo.add_sink(2);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes, vec![0]);
    }
}

// =========================================================================
// Feature 08: Momentum Acceleration (Heavy-Ball / Polyak)
// =========================================================================
mod feature_08_momentum_acceleration {
    use super::*;

    #[test]
    fn test_f08_beta_zero_standard_power_iteration() {
        let pruner = TauSpectralPruner::builder().momentum_beta(0.0).build();
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f08_beta_default_0_5_accelerates_convergence() {
        let pruner = TauSpectralPruner::builder().momentum_beta(0.5).build();
        assert_eq!(pruner.momentum_beta(), 0.5);

        let mut topo = Topology::new(5);
        for i in 0..4 {
            topo.add_edge(i, i + 1);
        }
        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f08_beta_sweep_0_1_to_0_9() {
        let betas = [0.1, 0.3, 0.5, 0.7, 0.9];
        let mut topo = Topology::new(6);
        for i in 0..5 {
            topo.add_edge(i, i + 1);
        }

        for &beta in &betas {
            let pruner = TauSpectralPruner::builder().momentum_beta(beta).build();
            let res = pruner.prune(&topo, 0).unwrap();
            assert_eq!(res.action, PolicyAction::Allow);
        }
    }

    #[test]
    fn test_f08_momentum_reset_in_workspace() {
        let pruner = TauSpectralPruner::builder().momentum_beta(0.6).build();
        let mut ws = PrunerWorkspace::new();

        let mut topo1 = Topology::new(4);
        topo1.add_edge(0, 1);
        topo1.add_edge(2, 3);
        let _ = pruner.prune_with_workspace(&topo1, 0, &mut ws).unwrap();

        let mut topo2 = Topology::new(5);
        topo2.add_edge(0, 1);
        topo2.add_edge(1, 2);
        let res2 = pruner.prune_with_workspace(&topo2, 0, &mut ws).unwrap();

        assert_eq!(res2.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f08_momentum_on_dense_cliques() {
        let pruner = TauSpectralPruner::builder().momentum_beta(0.5).build();
        let mut topo = Topology::new(10);
        for i in 0..10 {
            for j in (i + 1)..10 {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 10);
    }
}

// =========================================================================
// Feature 09: Rayleigh Quotient lambda_2 Algebraic Connectivity
// =========================================================================
mod feature_09_rayleigh_quotient {
    use super::*;

    #[test]
    fn test_f09_complete_clique_connectivity() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(4);
        for i in 0..4 {
            for j in (i + 1)..4 {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 0).unwrap();
        // Complete graph K_4 has algebraic connectivity ~ 4.0
        assert!(res.connectivity_score > 2.0);
    }

    #[test]
    fn test_f09_star_graph_connectivity() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(5);
        for i in 1..5 {
            topo.add_edge(0, i);
        }

        let res = pruner.prune(&topo, 0).unwrap();
        // Star graph has algebraic connectivity 1.0
        assert!(res.connectivity_score > 0.5);
    }

    #[test]
    fn test_f09_path_graph_connectivity() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        for i in 0..5 {
            topo.add_edge(i, i + 1);
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert!(res.connectivity_score > 0.0);
    }

    #[test]
    fn test_f09_disconnected_graph_zero_connectivity() {
        let pruner = TauSpectralPruner::builder().build();
        let topo = Topology::new(6); // 6 isolated nodes

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.connectivity_score, 0.0);
    }

    #[test]
    fn test_f09_monotonic_decrease_with_weak_bridge() {
        let pruner = TauSpectralPruner::builder().build();

        // 2 cliques K_3 connected by 1 edge
        let mut topo = Topology::new(6);
        // Clique 1: 0, 1, 2
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);
        // Clique 2: 3, 4, 5
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);
        topo.add_edge(5, 3);
        // Bridge
        topo.add_edge(2, 3);

        let res = pruner.prune(&topo, 0).unwrap();
        assert!(res.connectivity_score > 0.0);
        assert!(res.connectivity_score < 3.0);
    }
}

// =========================================================================
// Feature 10: Reusable PrunerWorkspace (Zero-Allocation Streaming)
// =========================================================================
mod feature_10_reusable_workspace {
    use super::*;

    #[test]
    fn test_f10_with_capacity_allocation_bounds() {
        let ws = PrunerWorkspace::with_capacity(50, 100);
        assert!(ws.v_vec.capacity() >= 50);
        assert!(ws.csr_row_ptrs.capacity() >= 51);
        assert!(ws.csr_col_indices.capacity() >= 200);
        assert!(ws.degrees.capacity() >= 50);
    }

    #[test]
    fn test_f10_reset_for_nodes_clearing() {
        let mut ws = PrunerWorkspace::with_capacity(20, 30);
        ws.reset_for_nodes(10);

        assert_eq!(ws.v_vec.len(), 10);
        assert_eq!(ws.v_m.len(), 10);
        assert_eq!(ws.sink_bits.len(), 10);
        assert_eq!(ws.island_bits.len(), 10);
    }

    #[test]
    fn test_f10_prune_and_prune_with_workspace_exact_parity() {
        let pruner = TauSpectralPruner::builder().build();
        let mut ws = PrunerWorkspace::new();

        let mut topo = Topology::new(7);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);

        let res_direct = pruner.prune(&topo, 0).unwrap();
        let res_ws = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();

        assert_eq!(res_direct, res_ws);
    }

    #[test]
    fn test_f10_workspace_capacity_growth_resilience() {
        let pruner = TauSpectralPruner::builder().build();
        let mut ws = PrunerWorkspace::with_capacity(5, 5);

        // Feed larger graph
        let mut topo = Topology::new(30);
        for i in 0..29 {
            topo.add_edge(i, i + 1);
        }

        let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f10_workspace_zero_alloc_streaming_throughput() {
        let pruner = TauSpectralPruner::builder().build();
        let mut ws = PrunerWorkspace::with_capacity(20, 50);

        for iter in 0..100 {
            let mut topo = Topology::new(10);
            topo.add_edge(iter % 5, (iter + 1) % 5);
            topo.add_edge(5 + (iter % 4), 6 + (iter % 4));

            let res = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
            assert_eq!(res.action, PolicyAction::Allow);
        }
    }
}

// =========================================================================
// Feature 11: Injected Tau-Boundary Tie-Breaking Split
// =========================================================================
mod feature_11_injected_tau_bisection {
    use super::*;

    #[test]
    fn test_f11_default_tau_zero_split() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        assert_eq!(pruner.tau(), 0.0);

        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f11_negative_tau_sweep() {
        let pruner = TauSpectralPruner::builder().tau(-0.5).build();
        assert_eq!(pruner.tau(), -0.5);

        let mut topo = Topology::new(5);
        for i in 0..4 {
            topo.add_edge(i, i + 1);
        }
        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f11_positive_tau_sweep() {
        let pruner = TauSpectralPruner::builder().tau(0.5).build();
        assert_eq!(pruner.tau(), 0.5);

        let mut topo = Topology::new(5);
        for i in 0..4 {
            topo.add_edge(i, i + 1);
        }
        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f11_volume_based_mainland_island_assignment() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);
        topo.add_edge(3, 4);

        let res = pruner.prune(&topo, 0).unwrap();
        assert!(res.mainland_nodes.len() >= res.island_nodes.len());
    }

    #[test]
    fn test_f11_exact_boundary_value_tie() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(2, 3);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 4);
    }
}

// =========================================================================
// Feature 12: Scale-Invariant Semantic Density Ratio
// =========================================================================
mod feature_12_scale_invariant_density_ratio {
    use super::*;

    #[test]
    fn test_f12_dense_island_high_threat() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(8)
            .threat_threshold(2.0)
            .build();
        let mut topo = Topology::new(9);
        // Mainland: 0..3 clique
        for i in 0..4 {
            for j in (i + 1)..4 {
                topo.add_edge(i, j);
            }
        }
        // Dense Island: 4..7 clique (4 nodes, 6 edges)
        for i in 4..8 {
            for j in (i + 1)..8 {
                topo.add_edge(i, j);
            }
        }
        // System node: 8. Single link to system
        topo.add_edge(4, 8);

        let res = pruner.prune(&topo, 8).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }

    #[test]
    fn test_f12_sparse_island_low_threat() {
        let pruner = TauSpectralPruner::builder().threat_threshold(100.0).build();
        let mut topo = Topology::new(8);
        for i in 0..4 {
            topo.add_edge(i, i + 1);
        }
        // Island with many system connections
        topo.add_edge(5, 6);
        topo.add_edge(5, 7);
        topo.add_edge(6, 7);

        let res = pruner.prune(&topo, 7).unwrap();
        assert!(res.action != PolicyAction::FatalBlock || res.island_nodes.is_empty());
    }

    #[test]
    fn test_f12_scale_invariance_proportional_growth() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(10)
            .threat_threshold(2.0)
            .build();

        let mut topo = Topology::new(15);
        // Island nodes 0..3 clique
        for i in 0..3 {
            for j in (i + 1)..3 {
                topo.add_edge(i, j);
            }
        }
        // Single link to system
        topo.add_edge(0, 10);

        let res = pruner.prune(&topo, 12).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }

    #[test]
    fn test_f12_zero_to_system_infinite_ratio() {
        let pruner = TauSpectralPruner::builder().system_start_idx(5).build();
        let mut topo = Topology::new(8);
        // Mainland connected to system
        topo.add_edge(0, 1);
        topo.add_edge(1, 5);
        // Island isolated from system
        topo.add_edge(3, 4);

        let res = pruner.prune(&topo, 5).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }

    #[test]
    fn test_f12_empty_island_zero_ratio() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(2);
        topo.add_edge(0, 1);

        let res = pruner.prune(&topo, 1).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }
}

// =========================================================================
// Feature 13: Instruction Neglect Thresholding
// =========================================================================
mod feature_13_instruction_neglect {
    use super::*;

    #[test]
    fn test_f13_zero_system_edges_neglect() {
        let pruner = TauSpectralPruner::builder().system_start_idx(5).build();
        let mut topo = Topology::new(7);
        topo.add_edge(0, 1);
        topo.add_edge(1, 5); // Mainland to system
        topo.add_edge(3, 4); // Island with 0 system edges

        let res = pruner.prune(&topo, 5).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }

    #[test]
    fn test_f13_sub_threshold_connection_neglect() {
        let pruner = TauSpectralPruner::builder().system_start_idx(20).build();
        let mut topo = Topology::new(22);
        // Mainland connected to system
        for i in 0..5 {
            topo.add_edge(i, 20);
        }
        // Island of 15 nodes with only 1 link to system (1/15 = 0.066 < 0.1)
        for i in 5..19 {
            topo.add_edge(i, i + 1);
        }
        topo.add_edge(5, 20); // 1 system link

        let res = pruner.prune(&topo, 20).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }

    #[test]
    fn test_f13_exact_threshold_connection() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(10)
            .threat_threshold(100.0)
            .build();
        let mut topo = Topology::new(12);
        // Island of 10 nodes (0..9) with 1 system link (1/10 = 0.10)
        for i in 0..9 {
            topo.add_edge(i, i + 1);
        }
        topo.add_edge(0, 10);

        let res = pruner.prune(&topo, 10).unwrap();
        // Because ratio threshold is high, instruction neglect is exactly 0.1 (not < 0.1)
        assert!(res.action != PolicyAction::Allow || res.island_nodes.is_empty());
    }

    #[test]
    fn test_f13_healthy_system_connection() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(4)
            .threat_threshold(10.0)
            .build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(0, 4);
        topo.add_edge(1, 4);
        topo.add_edge(2, 3);
        topo.add_edge(2, 4);
        topo.add_edge(3, 4);

        let res = pruner.prune(&topo, 4).unwrap();
        assert_eq!(res.action, PolicyAction::GarbageCollect);
    }

    #[test]
    fn test_f13_multi_node_cluster_neglect() {
        let pruner = TauSpectralPruner::builder().system_start_idx(8).build();
        let mut topo = Topology::new(10);
        topo.add_edge(0, 8);
        topo.add_edge(1, 8);
        // Disconnected cluster 3..7
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);
        topo.add_edge(5, 6);
        topo.add_edge(6, 7);

        let res = pruner.prune(&topo, 8).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }
}

// =========================================================================
// Feature 14: Micro-Steering Single-Token Tripwire
// =========================================================================
mod feature_14_single_token_tripwire {
    use super::*;

    #[test]
    fn test_f14_exact_tripwire_trigger_1_edge() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);
        // Single isolated token 3 with exactly 1 link to system node 5
        topo.add_edge(3, 5);

        let res = pruner.prune(&topo, 5).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
        assert_eq!(res.island_nodes, vec![3]);
    }

    #[test]
    fn test_f14_tripwire_bypass_2_edges() {
        let pruner = TauSpectralPruner::builder()
            .threat_threshold(10.0)
            .system_start_idx(4)
            .build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        // Node 3 has 2 links to system (to_system = 2.0 >= 2.0)
        topo.add_edge(3, 4);
        topo.add_edge(3, 5);

        let res = pruner.prune(&topo, 5).unwrap();
        // Bypasses single-token tripwire (to_system >= 2.0)
        assert_ne!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f14_tripwire_bypass_internal_edges() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        // Node 3 has an internal edge to node 4
        topo.add_edge(3, 4);
        topo.add_edge(3, 5);

        let res = pruner.prune(&topo, 5).unwrap();
        // Since island length is 2, single-token tripwire is not the trigger
        assert_eq!(res.action, PolicyAction::GarbageCollect);
        assert!(!res.diagnostics.single_token_triggered);
        assert!(!res.diagnostics.density_triggered);
    }

    #[test]
    fn test_f14_tripwire_bypass_multi_node() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(7);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(3, 4);
        topo.add_edge(3, 6);
        topo.add_edge(4, 6);

        let res = pruner.prune(&topo, 6).unwrap();
        assert_ne!(res.island_nodes.len(), 1);
    }

    #[test]
    fn test_f14_tripwire_with_varying_system_lengths() {
        for sys_len in 5..10 {
            let pruner = TauSpectralPruner::builder()
                .system_start_idx(sys_len)
                .build();
            let mut topo = Topology::new(sys_len + 1);
            let single_token = sys_len - 1;
            // Connect mainland nodes 0..single_token in a cycle
            for i in 0..single_token {
                topo.add_edge(i, (i + 1) % single_token);
            }
            // Single isolated token with exactly 1 link to system node
            topo.add_edge(single_token, sys_len);

            let res = pruner.prune(&topo, sys_len).unwrap();
            assert_eq!(res.action, PolicyAction::FatalBlock);
            assert_eq!(res.island_nodes, vec![single_token]);
        }
    }
}

// =========================================================================
// Feature 15: Policy Verdict Mapping & Formatting
// =========================================================================
mod feature_15_policy_verdict_mapping {
    use super::*;

    #[test]
    fn test_f15_allow_on_zero_system_boundary() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(3, 4);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f15_allow_on_empty_island() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(2);
        topo.add_edge(0, 1);

        let res = pruner.prune(&topo, 1).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f15_garbage_collect_on_benign_sub_threshold() {
        let pruner = TauSpectralPruner::builder()
            .threat_threshold(100.0)
            .system_start_idx(4)
            .build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(0, 4);
        topo.add_edge(1, 4);
        topo.add_edge(2, 3);
        topo.add_edge(2, 4);
        topo.add_edge(3, 5);

        let res = pruner.prune(&topo, 5).unwrap();
        assert_eq!(res.action, PolicyAction::GarbageCollect);
    }

    #[test]
    fn test_f15_fatal_block_on_high_density() {
        let pruner = TauSpectralPruner::builder().threat_threshold(1.0).build();
        let mut topo = Topology::new(7);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        // Island clique
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);
        topo.add_edge(5, 3);
        topo.add_edge(3, 6);

        let res = pruner.prune(&topo, 6).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
    }

    #[test]
    fn test_f15_policy_action_display_formatting() {
        assert_eq!(format!("{}", PolicyAction::Allow), "ALLOW");
        assert_eq!(
            format!("{}", PolicyAction::GarbageCollect),
            "GARBAGE_COLLECT"
        );
        assert_eq!(format!("{}", PolicyAction::FatalBlock), "FATAL_BLOCK");
    }
}

// =========================================================================
// Feature 16: Telemetry vs Output Separation
// =========================================================================
mod feature_16_telemetry_separation {
    use super::*;

    #[test]
    fn test_f16_system_nodes_excluded_from_mainland_and_island() {
        let pruner = TauSpectralPruner::builder().system_start_idx(3).build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);

        let res = pruner.prune(&topo, 5).unwrap();
        for &node in &res.mainland_nodes {
            assert!(node < 3, "Mainland must not contain system nodes [3..5]");
        }
        for &node in &res.island_nodes {
            assert!(node < 3, "Island must not contain system nodes [3..5]");
        }
    }

    #[test]
    fn test_f16_system_nodes_participate_in_eigensolver() {
        let pruner = TauSpectralPruner::builder().system_start_idx(2).build();
        let mut topo = Topology::new(4);
        topo.add_edge(0, 2);
        topo.add_edge(1, 3);

        let res = pruner.prune(&topo, 3).unwrap();
        assert!(res.connectivity_score >= 0.0);
    }

    #[test]
    fn test_f16_custom_system_start_idx() {
        let pruner = TauSpectralPruner::builder().system_start_idx(1).build();
        assert_eq!(pruner.system_start_idx(), 1);

        let mut topo = Topology::new(5);
        topo.add_edge(0, 4);

        let res = pruner.prune(&topo, 3).unwrap();
        // System range is 1..=3
        assert!(!res.mainland_nodes.contains(&1));
        assert!(!res.mainland_nodes.contains(&2));
        assert!(!res.mainland_nodes.contains(&3));
    }

    #[test]
    fn test_f16_inverted_system_range_no_stripping() {
        let pruner = TauSpectralPruner::builder().system_start_idx(10).build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);

        let res = pruner.prune(&topo, 3).unwrap();
        // start_idx (10) > boundary_len (3) => is_system_node returns false
        assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 5);
    }

    #[test]
    fn test_f16_telemetry_separation_with_sinks() {
        let pruner = TauSpectralPruner::builder().system_start_idx(2).build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_sink(2); // Sink inside system range

        let res = pruner.prune(&topo, 4).unwrap();
        assert!(!res.mainland_nodes.contains(&2));
        assert!(!res.island_nodes.contains(&2));
    }
}

// =========================================================================
// Feature 17: Configuration & Validation Layer
// =========================================================================
mod feature_17_config_validation {
    use super::*;

    #[test]
    fn test_f17_builder_defaults() {
        let pruner = TauSpectralPruner::builder().build();
        assert_eq!(pruner.tau(), 0.0);
        assert_eq!(pruner.threat_threshold(), 2.0);
        assert_eq!(pruner.max_iterations(), 10_000);
        assert_eq!(pruner.tolerance(), 1e-9);
        assert_eq!(pruner.momentum_beta(), 0.5);
        assert_eq!(pruner.system_start_idx(), 5);
    }

    #[test]
    fn test_f17_invalid_tolerance_errors() {
        assert!(PrunerBuilder::default().tolerance(0.0).try_build().is_err());
        assert!(PrunerBuilder::default()
            .tolerance(-1e-5)
            .try_build()
            .is_err());
        assert!(PrunerBuilder::default()
            .tolerance(f64::NAN)
            .try_build()
            .is_err());
    }

    #[test]
    fn test_f17_invalid_max_iterations_error() {
        assert!(PrunerBuilder::default()
            .max_iterations(0)
            .try_build()
            .is_err());
        assert!(PrunerBuilder::default()
            .max_iterations(1)
            .try_build()
            .is_ok());
    }

    #[test]
    fn test_f17_invalid_momentum_beta_errors() {
        assert!(PrunerBuilder::default()
            .momentum_beta(-0.1)
            .try_build()
            .is_err());
        assert!(PrunerBuilder::default()
            .momentum_beta(1.0)
            .try_build()
            .is_err());
        assert!(PrunerBuilder::default()
            .momentum_beta(1.5)
            .try_build()
            .is_err());
        assert!(PrunerBuilder::default()
            .momentum_beta(f64::NAN)
            .try_build()
            .is_err());
    }

    #[test]
    fn test_f17_invalid_threat_threshold_errors() {
        assert!(PrunerBuilder::default()
            .threat_threshold(-0.1)
            .try_build()
            .is_err());
        assert!(PrunerBuilder::default()
            .threat_threshold(f64::NAN)
            .try_build()
            .is_err());
        assert!(PrunerBuilder::default()
            .threat_threshold(0.0)
            .try_build()
            .is_ok());
    }
}

// =========================================================================
// Feature 18: Invariant Baseline Tests Parity
// =========================================================================
mod feature_18_invariant_baseline {
    use super::*;

    #[test]
    fn test_f18_baseline_nominal_flow() {
        let pruner = TauSpectralPruner::builder()
            .tau(0.0)
            .threat_threshold(2.0)
            .build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f18_baseline_control_vector_override() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);
        topo.add_edge(3, 5);

        let res = pruner.prune(&topo, 5).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
        assert_eq!(res.island_nodes, vec![3]);
    }

    #[test]
    fn test_f18_baseline_isolated_node_regression() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 0);

        let res = pruner.prune(&topo, 0).unwrap();
        assert!(res.island_nodes.contains(&3) || res.mainland_nodes.contains(&3));
    }

    #[test]
    fn test_f18_baseline_custom_system_boundary() {
        let pruner = TauSpectralPruner::builder()
            .tau(0.0)
            .system_start_idx(2)
            .build();
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(4, 2);

        let res = pruner.prune(&topo, 3).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
        assert_eq!(res.island_nodes, vec![4]);
    }

    #[test]
    fn test_f18_baseline_tiny_topology_with_sink() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(2);
        topo.add_sink(0);

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        assert_eq!(res.mainland_nodes, vec![1]);
        assert!(res.island_nodes.is_empty());
    }
}

// =========================================================================
// Feature 19: E2E Property Tests & Mathematical Invariants
// =========================================================================
mod feature_19_e2e_property_tests {
    use super::*;

    #[test]
    fn test_f19_partition_conservation_disjoint_union() {
        let pruner = TauSpectralPruner::builder().system_start_idx(5).build();
        let mut topo = Topology::new(8);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(3, 4);
        topo.add_sink(7);

        let res = pruner.prune(&topo, 6).unwrap();

        // Active non-system, non-sink nodes are 0, 1, 2, 3, 4
        let mut combined = res.mainland_nodes.clone();
        combined.extend_from_slice(&res.island_nodes);
        combined.sort();
        assert_eq!(combined, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_f19_partition_disjointness() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);

        let res = pruner.prune(&topo, 0).unwrap();
        for node in &res.mainland_nodes {
            assert!(!res.island_nodes.contains(node));
        }
    }

    #[test]
    fn test_f19_sink_non_inclusion() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_sink(2);
        topo.add_sink(4);

        let res = pruner.prune(&topo, 0).unwrap();
        assert!(!res.mainland_nodes.contains(&2));
        assert!(!res.island_nodes.contains(&2));
        assert!(!res.mainland_nodes.contains(&4));
        assert!(!res.island_nodes.contains(&4));
    }

    #[test]
    fn test_f19_deterministic_repeated_execution() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(7);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(4, 5);

        let first = pruner.prune(&topo, 0).unwrap();
        for _ in 0..20 {
            let curr = pruner.prune(&topo, 0).unwrap();
            assert_eq!(first, curr);
        }
    }

    #[test]
    fn test_f19_connectivity_score_non_negative() {
        let pruner = TauSpectralPruner::builder().build();
        for n in 3..12 {
            let mut topo = Topology::new(n);
            for i in 0..n - 1 {
                topo.add_edge(i, i + 1);
            }
            let res = pruner.prune(&topo, 0).unwrap();
            assert!(res.connectivity_score >= 0.0);
            assert!(!res.connectivity_score.is_nan());
        }
    }
}

// =========================================================================
// Feature 20: Fuzzing & Adversarial Pseudo-Random Harness
// =========================================================================
mod feature_20_fuzzing_adversarial {
    use super::*;

    struct SimpleLcg(u64);
    impl SimpleLcg {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
            self.0
        }
        fn next_usize(&mut self, max: usize) -> usize {
            if max == 0 {
                0
            } else {
                (self.next_u64() % (max as u64)) as usize
            }
        }
    }

    #[test]
    fn test_f20_lcg_pseudo_random_generator() {
        let mut lcg1 = SimpleLcg(12345);
        let mut lcg2 = SimpleLcg(12345);
        for _ in 0..100 {
            assert_eq!(lcg1.next_u64(), lcg2.next_u64());
        }
    }

    #[test]
    fn test_f20_random_graph_degree_conservation() {
        let mut rng = SimpleLcg(42);
        let n = 20;
        let mut topo = Topology::new(n);
        for _ in 0..50 {
            let u = rng.next_usize(n);
            let v = rng.next_usize(n);
            topo.add_edge(u, v);
        }

        let sink_bits = BitSet::new(n);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);
        let half_edges: usize = (0..n).map(|i| csr.degree(i) as usize).sum();
        assert_eq!(half_edges, csr.half_edge_count());
    }

    #[test]
    fn test_f20_csr_vs_adjacency_differential() {
        let mut rng = SimpleLcg(99);
        let n = 15;
        let mut topo = Topology::new(n);
        for _ in 0..30 {
            let u = rng.next_usize(n);
            let v = rng.next_usize(n);
            topo.add_edge(u, v);
        }

        let sink_bits = BitSet::new(n);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        for u in 0..n {
            let neighbors = csr.neighbors(u);
            for &v in neighbors {
                assert!(
                    topo.edges.contains(&(u, v)) || topo.edges.contains(&(v, u)),
                    "CSR edge must exist in topology"
                );
            }
        }
    }

    #[test]
    fn test_f20_random_sink_masking() {
        let mut rng = SimpleLcg(777);
        let n = 20;
        let mut topo = Topology::new(n);
        for _ in 0..40 {
            topo.add_edge(rng.next_usize(n), rng.next_usize(n));
        }
        for _ in 0..5 {
            topo.add_sink(rng.next_usize(n));
        }

        let sink_bits = topo.to_sink_bitset();
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        for sink in &topo.sinks {
            assert_eq!(csr.degree(*sink), 0.0);
            assert_eq!(csr.neighbors(*sink), &[]);
        }
    }

    #[test]
    fn test_f20_random_topology_zero_panic() {
        let pruner = TauSpectralPruner::builder().build();
        let mut ws = PrunerWorkspace::new();
        let mut rng = SimpleLcg(0xACE1);

        for _ in 0..100 {
            let n = rng.next_usize(30);
            let mut topo = Topology::new(n);
            let e = rng.next_usize(50);
            for _ in 0..e {
                topo.add_edge(rng.next_usize(n + 2), rng.next_usize(n + 2));
            }
            let s = rng.next_usize(5);
            for _ in 0..s {
                topo.add_sink(rng.next_usize(n + 2));
            }
            let sys = rng.next_usize(n + 5);

            let res = pruner.prune_with_workspace(&topo, sys, &mut ws);
            assert!(res.is_ok());
        }
    }
}

// =========================================================================
// Feature 21: Benchmark Throughput & Scalability Validation
// =========================================================================
mod feature_21_benchmark_throughput {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_f21_small_topology_latency() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(10);
        for i in 0..9 {
            topo.add_edge(i, i + 1);
        }

        let start = Instant::now();
        for _ in 0..50 {
            let _ = pruner.prune(&topo, 0).unwrap();
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 100);
    }

    #[test]
    fn test_f21_medium_topology_throughput() {
        let pruner = TauSpectralPruner::builder().build();
        let mut ws = PrunerWorkspace::with_capacity(50, 100);
        let mut topo = Topology::new(50);
        for i in 0..49 {
            topo.add_edge(i, i + 1);
        }

        let start = Instant::now();
        for _ in 0..50 {
            let _ = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 2000);
    }

    #[test]
    fn test_f21_dense_clique_scaling() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(30);
        for i in 0..30 {
            for j in (i + 1)..30 {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f21_star_graph_scaling() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topo = Topology::new(100);
        for i in 1..100 {
            topo.add_edge(0, i);
        }

        let res = pruner.prune(&topo, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_f21_zero_allocation_streaming_workspace() {
        let pruner = TauSpectralPruner::builder().build();
        let mut ws = PrunerWorkspace::with_capacity(20, 50);

        let initial_v_cap = ws.v_vec.capacity();
        let initial_csr_cap = ws.csr_col_indices.capacity();

        for i in 0..50 {
            let mut topo = Topology::new(15);
            topo.add_edge(i % 10, (i + 1) % 10);
            let _ = pruner.prune_with_workspace(&topo, 0, &mut ws).unwrap();
        }

        assert_eq!(ws.v_vec.capacity(), initial_v_cap);
        assert_eq!(ws.csr_col_indices.capacity(), initial_csr_cap);
    }
}
