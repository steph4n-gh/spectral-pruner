//! Deterministic, zero-dependency weighted spectral graph auditing.
//!
//! The crate provides injected-τ Fiedler partitioning, protected-boundary
//! diagnostics, weighted conductance and density measures, configurable policy
//! triggers, and reusable numeric/CSR workspace buffers.
//!
//! Audits return measurements and a policy recommendation; they do not change
//! the supplied graph or execute containment. Inspect convergence before using
//! connectivity estimates quantitatively. The optional Python research harness
//! is separate from this dependency-free Rust crate.
#![doc = concat!("\n# Quick start\n\n```rust\n", include_str!("../examples/quick_start.rs"), "\n```\n")]

pub mod engine;
pub mod error;
pub mod graph;

// Re-export core items for library clean top-level paths
pub use engine::{
    PolicyAction, PrunerBuilder, PrunerDiagnostics, PrunerResolution, PrunerWorkspace,
    TauSpectralPruner, Topology,
};
pub use error::{PrunerError, Result};
pub use graph::{BitSet, CsrGraph, WeightedCsrGraph};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_nominal_flow() {
        let pruner = TauSpectralPruner::builder()
            .tau(0.0)
            .threat_threshold(2.0)
            .build();

        let mut topology = Topology::new(5);
        topology.add_edge(0, 1);
        topology.add_edge(1, 2);
        topology.add_edge(2, 0); // Structured cluster

        let res = pruner.prune(&topology, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
    }

    #[test]
    fn test_control_vector_override() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        let mut topology = Topology::new(6);

        // Mainland base layout
        topology.add_edge(0, 1);
        topology.add_edge(1, 2);
        topology.add_edge(2, 0);

        // Single isolated island node pointing directly at system space
        topology.add_edge(3, 5);

        let res = pruner.prune(&topology, 5).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
        assert_eq!(res.island_nodes, vec![3]);
    }

    #[test]
    fn test_isolated_node_tripwire_regression() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        let mut topology = Topology::new(5);

        // Connected mainland mass
        topology.add_edge(0, 1);
        topology.add_edge(1, 2);
        topology.add_edge(2, 0);

        // Node 3 is completely isolated (degree == 0).
        // It must be classified into a partition, not skipped!
        let res = pruner.prune(&topology, 0).unwrap();

        // The isolated node must be caught in the island partition
        assert!(
            res.island_nodes.contains(&3) || res.mainland_nodes.contains(&3),
            "Regression: Isolated nodes must not be ignored during classification!"
        );
    }

    #[test]
    fn test_custom_system_boundary_framing() {
        let pruner = TauSpectralPruner::builder()
            .tau(0.0)
            .system_start_idx(2)
            .build();
        let mut topology = Topology::new(5);

        // Mainland base layout
        topology.add_edge(0, 1);

        // Single isolated island node pointing directly at system space
        topology.add_edge(4, 2);

        let res = pruner.prune(&topology, 3).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
        assert_eq!(res.island_nodes, vec![4]);

        // System nodes (2 and 3) must be filtered out from mainland
        let mut expected_mainland = vec![0, 1];
        let mut actual_mainland = res.mainland_nodes.clone();
        actual_mainland.sort();
        expected_mainland.sort();
        assert_eq!(actual_mainland, expected_mainland);
    }

    #[test]
    fn test_tiny_topology_with_sink() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topology = Topology::new(2);
        topology.add_sink(0);

        let res = pruner.prune(&topology, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        // Sink node 0 must be filtered out, leaving only node 1
        assert_eq!(res.mainland_nodes, vec![1]);
        assert!(res.island_nodes.is_empty());
    }

    #[test]
    fn test_dense_clique_nominal() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topology = Topology::new(4);
        // Fully connected clique of 4 nodes
        topology.add_edge(0, 1);
        topology.add_edge(0, 2);
        topology.add_edge(0, 3);
        topology.add_edge(1, 2);
        topology.add_edge(1, 3);
        topology.add_edge(2, 3);

        let res = pruner.prune(&topology, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        // Fiedler bisection mathematically partitions the clique into two balanced halves
        assert_eq!(res.island_nodes.len(), 2);
        assert_eq!(res.mainland_nodes.len(), 2);
    }

    #[test]
    fn test_large_star_topology() {
        let pruner = TauSpectralPruner::builder().build();
        let mut topology = Topology::new(6);
        // Node 0 is the central hub connecting all other leaf nodes
        topology.add_edge(0, 1);
        topology.add_edge(0, 2);
        topology.add_edge(0, 3);
        topology.add_edge(0, 4);
        topology.add_edge(0, 5);

        let res = pruner.prune(&topology, 0).unwrap();
        assert_eq!(res.action, PolicyAction::Allow);
        // Star topology is successfully bisected mathematically,
        // and safely allowed as there is no system boundary or threat density.
        assert_eq!(res.island_nodes.len() + res.mainland_nodes.len(), 6);
        assert!(!res.island_nodes.is_empty());
        assert!(!res.mainland_nodes.is_empty());
    }

    #[test]
    fn test_prune_with_workspace_streaming_and_equivalence() {
        let pruner = TauSpectralPruner::builder().tau(0.0).build();
        let mut ws = PrunerWorkspace::with_capacity(10, 20);

        // Test multiple topologies through the same workspace instance
        let mut topo1 = Topology::new(6);
        topo1.add_edge(0, 1);
        topo1.add_edge(1, 2);
        topo1.add_edge(2, 0);
        topo1.add_edge(3, 5);

        let res1_ws = pruner.prune_with_workspace(&topo1, 5, &mut ws).unwrap();
        let res1_direct = pruner.prune(&topo1, 5).unwrap();
        assert_eq!(res1_ws, res1_direct);
        assert_eq!(res1_ws.action, PolicyAction::FatalBlock);
        assert_eq!(res1_ws.island_nodes, vec![3]);

        let mut topo2 = Topology::new(4);
        topo2.add_edge(0, 1);
        topo2.add_edge(1, 2);
        topo2.add_edge(2, 3);
        let res2_ws = pruner.prune_with_workspace(&topo2, 0, &mut ws).unwrap();
        let res2_direct = pruner.prune(&topo2, 0).unwrap();
        assert_eq!(res2_ws, res2_direct);
        assert_eq!(res2_ws.action, PolicyAction::Allow);
    }
}
