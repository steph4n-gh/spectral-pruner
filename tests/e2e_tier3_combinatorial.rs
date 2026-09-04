//! 🧪 E2E Tier 3: Combinatorial & Multi-Feature Interaction Test Suite
//!
//! Evaluates complex cross-feature interactions under stressful multi-dimensional conditions:
//! dynamic workspace reuse with fluctuating sinks, sliding telemetry windows,
//! custom tau boundaries paired with single-token tripwires, and multi-tenant isolation.
//!
//! Zero external test dependencies: pure Rust stdlib.

use spectral_pruner::{PolicyAction, PrunerWorkspace, TauSpectralPruner, Topology};
use std::sync::Arc;
use std::thread;

// Deterministic 64-bit LCG PRNG
struct ComboLcg(u64);
impl ComboLcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
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
fn test_combo_workspace_streaming_varying_sink_and_system_masks() {
    let mut rng = ComboLcg::new(0xC0FFEE123456);
    let pruner = TauSpectralPruner::builder().max_iterations(100).build();
    let mut ws = PrunerWorkspace::with_capacity(50, 100);

    for _ in 0..500 {
        let n = 5 + rng.next_usize(25); // N in [5, 30]
        let mut topo = Topology::new(n);

        // Build random connected structure
        for i in 0..n - 1 {
            topo.add_edge(i, i + 1);
        }
        let extra_edges = rng.next_usize(n);
        for _ in 0..extra_edges {
            topo.add_edge(rng.next_usize(n), rng.next_usize(n));
        }

        // Random sink count
        let sink_count = rng.next_usize(n / 3);
        for _ in 0..sink_count {
            topo.add_sink(rng.next_usize(n));
        }

        let sys_start = rng.next_usize(n);
        let sys_len = sys_start + rng.next_usize(n - sys_start + 5);

        let res_ws = pruner
            .prune_with_workspace(&topo, sys_len, &mut ws)
            .unwrap();
        let res_direct = pruner.prune(&topo, sys_len).unwrap();

        assert_eq!(res_ws, res_direct);

        // Partition conservation verification
        let is_system = |i: usize| sys_len > 0 && i >= pruner.system_start_idx() && i <= sys_len;
        for &node in &res_ws.mainland_nodes {
            assert!(!topo.sinks.contains(&node));
            assert!(!is_system(node));
        }
        for &node in &res_ws.island_nodes {
            assert!(!topo.sinks.contains(&node));
            assert!(!is_system(node));
        }
    }
}

#[test]
fn test_combo_custom_tau_with_single_token_tripwire() {
    let taus = [-0.1, -0.05, 0.0, 0.05, 0.1];

    for &tau in &taus {
        let pruner = TauSpectralPruner::builder()
            .tau(tau)
            .system_start_idx(5)
            .build();

        let mut topo = Topology::new(6);
        // Mainland cycle: 0..3
        for i in 0..4 {
            topo.add_edge(i, (i + 1) % 4);
        }
        // Single isolated token (node 4) connected with 1 edge to system anchor 5
        topo.add_edge(4, 5);

        let res = pruner.prune(&topo, 5).unwrap();
        assert_eq!(res.action, PolicyAction::FatalBlock);
        assert_eq!(res.island_nodes, vec![4]);
    }
}

#[test]
fn test_combo_density_ratio_under_momentum_and_tolerance_variations() {
    let betas = [0.0, 0.25, 0.5, 0.75, 0.85];
    let tols = [1e-4, 1e-6, 1e-8];

    for &beta in &betas {
        for &tol in &tols {
            let pruner = TauSpectralPruner::builder()
                .momentum_beta(beta)
                .tolerance(tol)
                .threat_threshold(2.0)
                .system_start_idx(8)
                .build();

            // Topology with a dense backdoor island
            let mut topo = Topology::new(9);
            // Mainland: 0..3 clique
            for i in 0..4 {
                for j in (i + 1)..4 {
                    topo.add_edge(i, j);
                }
            }
            // Dense Island: 4..7 clique
            for i in 4..8 {
                for j in (i + 1)..8 {
                    topo.add_edge(i, j);
                }
            }
            // 1 link to system node 8
            topo.add_edge(4, 8);

            let res = pruner.prune(&topo, 8).unwrap();
            assert_eq!(
                res.action,
                PolicyAction::FatalBlock,
                "Eigensolver config (beta={}, tol={}) must preserve FatalBlock",
                beta,
                tol
            );
            assert_eq!(res.island_nodes.len(), 4);
        }
    }
}

#[test]
fn test_combo_dense_backdoor_with_sink_severed_bridge() {
    let pruner = TauSpectralPruner::builder().system_start_idx(10).build();

    let mut topo = Topology::new(12);
    // Mainland: 0..4 connected to system node 10
    for i in 0..5 {
        topo.add_edge(i, (i + 1) % 5);
        topo.add_edge(i, 10);
    }

    // Bridge node 5 connecting mainland to backdoor cluster
    topo.add_edge(0, 5);
    topo.add_edge(5, 6);

    // Backdoor cluster: 6..9
    for i in 6..10 {
        for j in (i + 1)..10 {
            topo.add_edge(i, j);
        }
    }

    // Mark the bridge node 5 as a sink, effectively severing communication
    topo.add_sink(5);

    let res = pruner.prune(&topo, 10).unwrap();
    // With bridge severed by sink, backdoor cluster has 0 system edges -> FatalBlock by Instruction Neglect
    assert_eq!(res.action, PolicyAction::FatalBlock);
    assert!(!res.mainland_nodes.contains(&5));
    assert!(!res.island_nodes.contains(&5));
    for i in 6..10 {
        assert!(res.island_nodes.contains(&i));
    }
}

#[test]
fn test_combo_sliding_system_window_on_barbell() {
    let mut topo = Topology::new(10);
    // Left Bell: 0..4
    for i in 0..4 {
        for j in (i + 1)..4 {
            topo.add_edge(i, j);
        }
    }
    // Right Bell: 5..9
    for i in 5..9 {
        for j in (i + 1)..9 {
            topo.add_edge(i, j);
        }
    }
    // Bridge between 3 and 5
    topo.add_edge(3, 5);

    // Slide system window across the graph: [0..2], [3..5], [6..8]
    for start in [0, 3, 6] {
        let end = start + 2;
        let pruner = TauSpectralPruner::builder().system_start_idx(start).build();

        let res = pruner.prune(&topo, end).unwrap();
        // Verify system nodes are stripped
        for &node in &res.mainland_nodes {
            assert!(node < start || node > end);
        }
        for &node in &res.island_nodes {
            assert!(node < start || node > end);
        }
    }
}

#[test]
fn test_combo_multi_tenant_workspace_isolation() {
    let pruner = Arc::new(
        TauSpectralPruner::builder()
            .threat_threshold(2.0)
            .system_start_idx(5)
            .build(),
    );

    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let pruner = Arc::clone(&pruner);
            thread::spawn(move || {
                let mut ws = PrunerWorkspace::with_capacity(30, 60);
                let mut rng = ComboLcg::new((thread_id as u64 + 1) * 0x9E3779B9);

                for _ in 0..100 {
                    let n = 8 + rng.next_usize(12);
                    let mut topo = Topology::new(n);
                    for i in 0..n - 1 {
                        topo.add_edge(i, i + 1);
                    }
                    topo.add_edge(0, 5); // Mainland to system

                    let res = pruner.prune_with_workspace(&topo, 5, &mut ws).unwrap();
                    assert!(!res.connectivity_score.is_nan());
                    assert!(res.connectivity_score >= -1e-9);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
