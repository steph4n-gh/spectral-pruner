use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};

fn weighted_boundary_topology() -> Topology {
    let mut topology = Topology::new(7);

    // Four-node mainland clique.
    for u in 0..4 {
        for v in (u + 1)..4 {
            topology.add_edge(u, v);
        }
    }

    // Two-token attention island and one protected system token.
    topology.add_weighted_edge(4, 5, 0.8);
    topology.add_weighted_edge(4, 6, 0.2);
    topology.add_weighted_edge(5, 6, 0.2);
    topology
}

#[test]
fn weighted_density_diagnostics_match_possible_edge_normalization() {
    let topology = weighted_boundary_topology();
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .threat_threshold(0.9)
        .build();

    let result = pruner.prune(&topology, 6).unwrap();
    let diagnostics = &result.diagnostics;

    assert_eq!(result.action, PolicyAction::FatalBlock);
    assert_eq!(result.island_nodes, vec![4, 5]);
    assert_eq!(diagnostics.island_node_count, 2);
    assert_eq!(diagnostics.system_node_count, 1);
    assert!((diagnostics.internal_weight - 0.8).abs() < 1e-12);
    assert!((diagnostics.system_weight - 0.4).abs() < 1e-12);
    assert!((diagnostics.partition_cut_weight - 0.4).abs() < 1e-12);
    assert!((diagnostics.island_volume - 2.0).abs() < 1e-12);
    assert!((diagnostics.conductance - 0.2).abs() < 1e-12);
    assert!((diagnostics.internal_density - 0.8).abs() < 1e-12);
    assert!((diagnostics.boundary_density - 0.2).abs() < 1e-12);
    assert!((diagnostics.possible_edge_density_ratio - 4.0).abs() < 1e-12);
    assert!((diagnostics.density_ratio - 1.0).abs() < 1e-12);
    assert!((diagnostics.instruction_connection - 0.2).abs() < 1e-12);
    assert!(diagnostics.density_triggered);
    assert!(!diagnostics.instruction_neglect_triggered);
    assert!(!diagnostics.single_token_triggered);
}

#[test]
fn boundary_endpoint_is_not_misused_as_system_node_count() {
    let topology = weighted_boundary_topology();
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .threat_threshold(0.9)
        .build();

    // The endpoint extends beyond the graph, but only node 6 exists in the
    // declared interval. The density metric must therefore use one node.
    let result = pruner.prune(&topology, 100).unwrap();
    assert_eq!(result.diagnostics.system_node_count, 1);
    assert!((result.diagnostics.density_ratio - 1.0).abs() < 1e-12);
}

#[test]
fn protected_system_nodes_remain_active_even_if_marked_as_sinks() {
    let mut topology = weighted_boundary_topology();
    topology.add_sink(6);
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .threat_threshold(0.9)
        .build();

    let result = pruner.prune(&topology, 6).unwrap();
    assert_eq!(result.action, PolicyAction::FatalBlock);
    assert_eq!(result.diagnostics.system_node_count, 1);
    assert!((result.diagnostics.system_weight - 0.4).abs() < 1e-12);
}

#[test]
fn spectral_only_mode_supports_reproducible_ablation() {
    let topology = weighted_boundary_topology();
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .threat_threshold(0.9)
        .spectral_only()
        .build();

    let result = pruner.prune(&topology, 6).unwrap();
    assert_eq!(result.action, PolicyAction::GarbageCollect);
    assert!((result.diagnostics.density_ratio - 1.0).abs() < 1e-12);
    assert!(!result.diagnostics.density_triggered);
    assert!(!result.diagnostics.instruction_neglect_triggered);
    assert!(!result.diagnostics.single_token_triggered);
}

#[test]
fn calibrated_connectivity_threshold_can_drive_spectral_only_policy() {
    let topology = weighted_boundary_topology();
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .spectral_only()
        .connectivity_threshold(100.0)
        .build();

    let result = pruner.prune(&topology, 6).unwrap();
    assert_eq!(result.action, PolicyAction::FatalBlock);
    assert!(result.diagnostics.connectivity_triggered);
    assert!(!result.diagnostics.density_triggered);
    assert!(!result.diagnostics.instruction_neglect_triggered);
    assert!(!result.diagnostics.single_token_triggered);
}

#[test]
fn calibrated_thresholds_reject_non_finite_or_negative_values() {
    for threshold in [-0.1, f64::INFINITY, f64::NAN] {
        assert!(TauSpectralPruner::builder()
            .connectivity_threshold(threshold)
            .try_build()
            .is_err());
        assert!(TauSpectralPruner::builder()
            .instruction_connection_threshold(threshold)
            .try_build()
            .is_err());
    }
}

#[test]
fn individual_density_ablation_leaves_other_policy_checks_active() {
    let topology = weighted_boundary_topology();
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .threat_threshold(0.9)
        .density_ratio_enabled(false)
        .build();

    let result = pruner.prune(&topology, 6).unwrap();
    assert_eq!(result.action, PolicyAction::GarbageCollect);
    assert!(!result.diagnostics.density_triggered);
}

#[test]
fn instruction_connection_threshold_is_calibratable() {
    let topology = weighted_boundary_topology();
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .density_ratio_enabled(false)
        .single_token_tripwire_enabled(false)
        .instruction_connection_threshold(0.3)
        .build();

    let result = pruner.prune(&topology, 6).unwrap();
    assert_eq!(result.action, PolicyAction::FatalBlock);
    assert!((result.diagnostics.instruction_connection - 0.2).abs() < 1e-12);
    assert!(result.diagnostics.instruction_neglect_triggered);
}

#[test]
fn invalid_weight_is_rejected_at_the_pruning_boundary() {
    for weight in [0.0, -0.1, f64::INFINITY, f64::NAN] {
        let mut topology = Topology::new(3);
        topology.add_weighted_edge(0, 1, weight);
        let error = TauSpectralPruner::builder()
            .build()
            .prune(&topology, 0)
            .unwrap_err();
        assert!(error.to_string().contains("positive finite weight"));
    }
}

#[test]
fn uniformly_scaled_weights_scale_the_fiedler_value() {
    let mut light = Topology::new(4);
    let mut heavy = Topology::new(4);
    for u in 0..4 {
        for v in (u + 1)..4 {
            light.add_weighted_edge(u, v, 0.25);
            heavy.add_weighted_edge(u, v, 1.0);
        }
    }

    let pruner = TauSpectralPruner::builder().build();
    let light_result = pruner.prune(&light, 0).unwrap();
    let heavy_result = pruner.prune(&heavy, 0).unwrap();
    let observed_scale = heavy_result.connectivity_score / light_result.connectivity_score;

    assert!((observed_scale - 4.0).abs() < 1e-8);
}
