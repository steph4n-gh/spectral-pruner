use spectral_pruner::{PolicyAction, PrunerWorkspace, TauSpectralPruner, Topology};

fn path(nodes: usize, weight: f64) -> Topology {
    let mut graph = Topology::new(nodes);
    for node in 1..nodes {
        graph.add_weighted_edge(node - 1, node, weight);
    }
    graph
}

#[test]
fn nonfinite_partition_parameters_are_rejected() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(TauSpectralPruner::builder().tau(value).try_build().is_err());
        assert!(TauSpectralPruner::builder()
            .tolerance(value)
            .try_build()
            .is_err());
    }
    for tau in [-f64::MAX, 0.0, f64::MAX] {
        assert!(TauSpectralPruner::builder().tau(tau).try_build().is_ok());
    }
}

#[test]
fn invalid_boundaries_block_every_resolution_path_and_preserve_nodes() {
    let mut workspace = PrunerWorkspace::new();
    for nodes in 0..8 {
        for graph in [Topology::new(nodes), path(nodes, 1.0)] {
            for (start, end) in [(nodes + 1, nodes + 2), (2, 1)] {
                for tau in [-100.0, 0.0, 100.0] {
                    let pruner = TauSpectralPruner::builder()
                        .system_start_idx(start)
                        .tau(tau)
                        .build();
                    let result = pruner
                        .prune_with_workspace(&graph, end, &mut workspace)
                        .unwrap();
                    assert_eq!(
                        result.action,
                        PolicyAction::FatalBlock,
                        "n={nodes}, start={start}, end={end}, tau={tau}"
                    );
                    assert!(!result.diagnostics.boundary_configuration_valid);
                    let mut classified = result.mainland_nodes;
                    classified.extend(result.island_nodes);
                    classified.sort_unstable();
                    assert_eq!(classified, (0..nodes).collect::<Vec<_>>());
                    assert_eq!(pruner.prune(&graph, 0).unwrap().action, PolicyAction::Allow);
                }
            }
        }
    }
}

#[test]
fn accumulated_weight_overflow_is_an_error_in_both_entry_points() {
    let mut parallel = Topology::new(2);
    parallel.weighted_edges = vec![(0, 1, 1e308), (0, 1, 1e308)];
    let mut disjoint = Topology::new(4);
    disjoint.weighted_edges = vec![(0, 1, 1e308), (2, 3, 1e308)];
    let pruner = TauSpectralPruner::builder().build();
    let mut workspace = PrunerWorkspace::new();
    for graph in [parallel, path(3, 1e308), disjoint] {
        assert!(pruner.prune(&graph, 0).is_err());
        assert!(pruner
            .prune_with_workspace(&graph, 0, &mut workspace)
            .is_err());
    }
    let graph = path(4, 1.0);
    assert_eq!(
        pruner.prune(&graph, 0).unwrap(),
        pruner
            .prune_with_workspace(&graph, 0, &mut workspace)
            .unwrap()
    );
}

#[test]
fn uniform_weight_scaling_preserves_path_connectivity() {
    let expected = 2.0 - 2.0 * (std::f64::consts::PI / 4.0).cos();
    for weight in [1e-200, 1e-12, 1e-9, 1.0, 1e150] {
        let result = TauSpectralPruner::builder()
            .build()
            .prune(&path(4, weight), 0)
            .unwrap();
        assert!(result.diagnostics.solver_converged);
        assert!(
            (result.connectivity_score / weight - expected).abs() < 1e-7,
            "weight={weight}, score={}",
            result.connectivity_score
        );
    }
}

#[test]
fn iteration_exhaustion_is_reported_and_blocks_calibrated_policy() {
    let graph = path(20, 1.0);
    for tau in [0.0, 100.0] {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(19)
            .tau(tau)
            .max_iterations(1)
            .spectral_only()
            .connectivity_threshold(0.0)
            .build();
        let result = pruner.prune(&graph, 19).unwrap();
        assert_eq!(result.action, PolicyAction::FatalBlock);
        assert!(!result.diagnostics.solver_converged);
        assert_eq!(result.diagnostics.solver_iterations, 1);
        assert!(result.diagnostics.relative_residual.unwrap() > pruner.tolerance());
        assert!(result.diagnostics.numerical_failure_triggered);
        assert!(!result.diagnostics.connectivity_triggered);
    }
}

#[test]
fn long_path_is_not_mistaken_for_a_converged_estimate() {
    let result = TauSpectralPruner::builder()
        .build()
        .prune(&path(1000, 1.0), 0)
        .unwrap();
    assert!(!result.diagnostics.solver_converged);
    assert_eq!(result.diagnostics.solver_iterations, 10_000);
    assert!(result.diagnostics.relative_residual.unwrap() > 1e-9);
}

#[test]
fn unavailable_small_graph_eigenpair_cannot_authorize_connectivity_policy() {
    let result = TauSpectralPruner::builder()
        .system_start_idx(1)
        .spectral_only()
        .connectivity_threshold(0.0)
        .build()
        .prune(&path(2, 1.0), 1)
        .unwrap();
    assert_eq!(result.action, PolicyAction::FatalBlock);
    assert!(!result.diagnostics.solver_converged);
    assert_eq!(result.diagnostics.relative_residual, None);
    assert!(result.diagnostics.numerical_failure_triggered);
}

#[test]
fn no_system_connection_keeps_the_infinite_density_sentinel() {
    let mut graph = Topology::new(7);
    for u in 0..4 {
        for v in u + 1..4 {
            graph.add_edge(u, v);
        }
    }
    graph.add_edge(4, 5);
    let result = TauSpectralPruner::builder()
        .system_start_idx(6)
        .build()
        .prune(&graph, 6)
        .unwrap();
    assert_eq!(result.action, PolicyAction::FatalBlock);
    assert!(result.diagnostics.density_ratio.is_infinite());
    assert!(result.diagnostics.instruction_neglect_triggered);
}

#[test]
fn normalization_cannot_silently_drop_a_positive_edge() {
    let mut graph = Topology::new(4);
    graph.add_weighted_edge(0, 1, 1e200);
    graph.add_weighted_edge(2, 3, 1e-200);
    let error = TauSpectralPruner::builder()
        .build()
        .prune(&graph, 0)
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("underflows the normalized operator"));
}

#[test]
fn a_large_density_denominator_cannot_silently_zero_the_ratio() {
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(8)
        .threat_threshold(0.001)
        .build();
    for scale in [1.0, 1e306] {
        let mut graph = Topology::new(9);
        for u in 0..5 {
            for v in u + 1..5 {
                graph.add_weighted_edge(u, v, scale);
            }
        }
        for u in 5..8 {
            for v in u + 1..8 {
                graph.add_weighted_edge(u, v, 0.1 * scale);
            }
            graph.add_weighted_edge(u, 8, 20.0 * scale);
        }
        let result = pruner.prune(&graph, 8).unwrap();
        assert_eq!(result.island_nodes, vec![5, 6, 7]);
        assert!((result.diagnostics.density_ratio - 1.0 / 600.0).abs() < 1e-12);
        assert_eq!(result.action, PolicyAction::FatalBlock);
        assert!(result.diagnostics.density_triggered);
    }
}
