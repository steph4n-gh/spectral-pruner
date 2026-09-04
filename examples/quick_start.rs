//! A complete weighted audit with one protected node and an auditable verdict.
use spectral_pruner::{PolicyAction, PrunerError, TauSpectralPruner, Topology};

fn main() -> Result<(), PrunerError> {
    let mut graph = Topology::new(7);
    for u in 0..4 {
        for v in u + 1..4 {
            graph.add_edge(u, v);
        }
    }
    graph.add_weighted_edge(4, 5, 0.8);
    graph.add_weighted_edge(4, 6, 0.2);
    graph.add_weighted_edge(5, 6, 0.2);

    let pruner = TauSpectralPruner::builder()
        .system_start_idx(6)
        .threat_threshold(0.9)
        .try_build()?;
    let result = pruner.prune(&graph, 6)?;

    assert!(result.diagnostics.solver_converged);
    assert_eq!(result.action, PolicyAction::FatalBlock);
    assert_eq!(result.island_nodes, vec![4, 5]);
    assert!((result.diagnostics.density_ratio - 1.0).abs() < 1e-12);

    println!("Action: {}", result.action);
    println!("Mainland: {:?}", result.mainland_nodes);
    println!("Island: {:?}", result.island_nodes);
    println!("Density ratio: {}", result.diagnostics.density_ratio);
    println!("Converged: {}", result.diagnostics.solver_converged);
    Ok(())
}
