//! Empirical Challenger Stress Test Suite for Milestone 3
//! Rigorous stress-testing and empirical validation of:
//! 1. All boundary configurations (`system_boundary_len == 0`, `system_start_idx > system_boundary_len`,
//!    `system_start_idx == 0`, `system_boundary_len >= num_nodes`, single-node anchors, inverted indices).
//! 2. Advanced threat metrics on adversarial graphs:
//!    - Micro-steering single-token tripwire injections (N=1, internal=0, 0 < to_system < 2).
//!    - Instruction neglect decoupled clusters (to_system / N_island < 0.1).
//!    - Dense backdoor cliques (Scale-Invariant Density Ratio > threshold).
//!    - Benign cluster garbage-collect vs allow verdicts.
//! 3. Upfront and runtime validation errors for all numeric parameters (NaNs, negatives, zeroes, out-of-bounds).
//! 4. Randomized property-based fuzzing harnesses verifying telemetry separation, partition conservation,
//!    and workspace determinism across thousands of adversarial graphs.

use spectral_pruner::engine::{
    PolicyAction, PrunerWorkspace, TauSpectralPruner, Topology,
};
use spectral_pruner::error::PrunerError;
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

    #[inline]
    fn gen_f64(&mut self) -> f64 {
        let val = (self.next_u64() >> 11) as f64;
        val / ((1u64 << 53) as f64)
    }
}

// =========================================================================
// SECTION 1: BOUNDARY CONFIGURATIONS STRESS TESTS
// =========================================================================

/// Test boundary configuration: system_boundary_len == 0
/// When system_boundary_len == 0, policy MUST always resolve to Allow,
/// and no nodes (even if index >= system_start_idx) should be stripped as system nodes.
#[test]
fn test_boundary_zero_system_boundary_len_allows_all() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(0.01) // Extremely sensitive threshold
        .system_start_idx(0)    // Even if system_start_idx == 0
        .build();

    // Adversarial graph: disconnected island that would otherwise be FatalBlock
    let mut topo = Topology::new(8);
    // Mainland (0, 1, 2)
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Highly dense isolated backdoor clique (3, 4, 5, 6, 7)
    for u in 3..8 {
        for v in (u + 1)..8 {
            topo.add_edge(u, v);
        }
    }

    // With system_boundary_len == 0, policy MUST be Allow
    let res = pruner.prune(&topo, 0).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::Allow,
        "Zero system_boundary_len must always yield PolicyAction::Allow"
    );

    // All 8 nodes must be classified in mainland or island; none stripped
    let total_nodes = res.mainland_nodes.len() + res.island_nodes.len();
    assert_eq!(
        total_nodes, 8,
        "All 8 nodes must be retained when system_boundary_len == 0"
    );
    assert!(!res.mainland_nodes.contains(&0) || !res.island_nodes.contains(&0));
}

/// Test boundary configuration: system_start_idx > system_boundary_len
/// When system_start_idx > system_boundary_len > 0, the system interval [start, len] is empty.
/// No nodes are classified as system nodes, so to_system == 0.
/// Any isolated island decoupled from the mainland triggers FATAL_BLOCK via instruction neglect.
#[test]
fn test_boundary_inverted_system_start_idx_greater_than_boundary_len() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(20) // Inverted: 20 > 5
        .build();

    let mut topo = Topology::new(5);
    // Mainland
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Island cluster (3, 4)
    topo.add_edge(3, 4);

    let res = pruner.prune(&topo, 5).unwrap();

    // With boundary_len = 5 and system_start_idx = 20, system node set is empty.
    // Island (3, 4) has to_system = 0 -> instruction_neglect = 0.0 < 0.1 -> FatalBlock
    assert_eq!(
        res.action,
        PolicyAction::FatalBlock,
        "Inverted boundary range with isolated island should trigger FatalBlock"
    );

    // Since system interval [20, 5] is empty, no nodes are stripped
    let mut all_returned = res.mainland_nodes.clone();
    all_returned.extend(&res.island_nodes);
    all_returned.sort();
    assert_eq!(
        all_returned,
        (0..5).collect::<Vec<usize>>(),
        "No nodes should be stripped when system interval is empty"
    );
}

/// Test boundary configuration: system_start_idx == 0
/// When system_start_idx == 0, nodes 0..=system_boundary_len are system nodes.
/// They must participate in spectral bisection and metric calculations,
/// but must be completely stripped from output vectors.
#[test]
fn test_boundary_system_start_idx_zero_all_inclusive() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(0)
        .build();

    let mut topo = Topology::new(7);
    // System nodes: 0, 1
    // Mainland non-system nodes: 2, 3, 4, 5
    topo.add_edge(2, 3);
    topo.add_edge(3, 4);
    topo.add_edge(4, 5);
    topo.add_edge(5, 2);
    topo.add_edge(2, 0); // Connect mainland to system node 0

    // Island non-system node: 6
    topo.add_edge(6, 1); // Single micro-steering edge to system node 1

    let res = pruner.prune(&topo, 1).unwrap();

    // Micro-steering tripwire should trigger: island has node 6 (len=1), internal=0, to_system=1
    assert_eq!(res.action, PolicyAction::FatalBlock);
    assert_eq!(res.island_nodes, vec![6]);

    // System nodes 0, 1 must NOT appear in mainland or island
    for sys_node in 0..=1 {
        assert!(
            !res.mainland_nodes.contains(&sys_node),
            "System node {} must be stripped from mainland",
            sys_node
        );
        assert!(
            !res.island_nodes.contains(&sys_node),
            "System node {} must be stripped from island",
            sys_node
        );
    }
}

/// Test boundary configuration: system_boundary_len >= num_nodes
/// When system_boundary_len is greater than or equal to num_nodes:
/// All nodes in [system_start_idx, num_nodes) are treated as system nodes.
#[test]
fn test_boundary_system_boundary_len_greater_than_or_equal_to_num_nodes() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(4)
        .build();

    let mut topo = Topology::new(6);
    // Mainland: 0, 1, 2
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Island: 3 (pointing to system node 4)
    topo.add_edge(3, 4);

    // System boundary len = 100 (>> num_nodes = 6)
    // Nodes >= 4 (i.e. 4 and 5) are system nodes
    let res = pruner.prune(&topo, 100).unwrap();

    // Node 3 is island of size 1 with 1 system edge -> Single-Token Tripwire FatalBlock
    assert_eq!(res.action, PolicyAction::FatalBlock);
    assert_eq!(res.island_nodes, vec![3]);

    // System nodes 4 and 5 must be stripped
    assert!(!res.mainland_nodes.contains(&4));
    assert!(!res.mainland_nodes.contains(&5));
    assert!(!res.island_nodes.contains(&4));
    assert!(!res.island_nodes.contains(&5));
}

/// Test boundary configuration: entire graph consists of system nodes
/// (system_start_idx == 0, system_boundary_len >= num_nodes)
#[test]
fn test_boundary_entire_graph_is_system_domain() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(0)
        .build();

    let mut topo = Topology::new(6);
    for i in 0..5 {
        topo.add_edge(i, i + 1);
    }

    // Boundary covers all nodes 0..=5
    let res = pruner.prune(&topo, 5).unwrap();

    // When all nodes are system nodes, island_local_nodes is empty -> PolicyAction::Allow
    assert_eq!(res.action, PolicyAction::Allow);
    assert!(res.mainland_nodes.is_empty());
    assert!(res.island_nodes.is_empty());
}

/// Test boundary configuration: single-node system domain (system_start_idx == system_boundary_len)
#[test]
fn test_boundary_single_node_system_domain() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(5)
        .build();

    let mut topo = Topology::new(6);
    // Mainland: 0, 1, 2
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Island: 3
    topo.add_edge(3, 5); // edge to single system node 5

    // System boundary len = 5, start_idx = 5 => exactly node 5 is system
    let res = pruner.prune(&topo, 5).unwrap();

    assert_eq!(res.action, PolicyAction::FatalBlock);
    assert_eq!(res.island_nodes, vec![3]);
    assert!(!res.mainland_nodes.contains(&5));
    assert!(!res.island_nodes.contains(&5));
}

/// Test boundary configuration: system_start_idx exceeds num_nodes
#[test]
fn test_boundary_system_start_idx_exceeds_num_nodes() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(50) // > num_nodes = 6
        .build();

    let mut topo = Topology::new(6);
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);
    topo.add_edge(3, 4);

    let res = pruner.prune(&topo, 100).unwrap();

    // Since system_start_idx = 50 and num_nodes = 6, no actual graph node is in [50, 100].
    // System node set is empty -> to_system = 0 -> instruction neglect triggers FatalBlock
    assert_eq!(res.action, PolicyAction::FatalBlock);
    assert_eq!(res.mainland_nodes.len() + res.island_nodes.len(), 6);
}

/// Test boundary configuration with small graphs (N = 0, 1, 2) across arbitrary boundaries
#[test]
fn test_boundary_small_graphs_exhaustive_boundaries() {
    let pruners = [
        TauSpectralPruner::builder().system_start_idx(0).build(),
        TauSpectralPruner::builder().system_start_idx(1).build(),
        TauSpectralPruner::builder().system_start_idx(5).build(),
    ];

    for pruner in &pruners {
        for n in 0..3 {
            let mut topo = Topology::new(n);
            if n == 2 {
                topo.add_edge(0, 1);
            }

            for sys_len in 0..10 {
                let res = pruner.prune(&topo, sys_len).unwrap();
                assert_eq!(res.action, PolicyAction::Allow);
                assert_eq!(res.connectivity_score, 0.0);

                // All returned nodes must be valid and non-system
                for &node in &res.mainland_nodes {
                    assert!(node < n);
                    if sys_len > 0 && node >= pruner.system_start_idx() && node <= sys_len {
                        panic!("System node {} leaked in mainland for N={}", node, n);
                    }
                }
                assert!(res.island_nodes.is_empty());
            }
        }
    }
}

// =========================================================================
// SECTION 2: ADVERSARIAL THREAT METRICS STRESS TESTS
// =========================================================================

/// Test micro-steering single-token tripwire injections:
/// Exact trigger: N_island == 1, internal == 0, 0.0 < to_system < 2.0
#[test]
fn test_adversarial_single_token_tripwire_exact_trigger() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(4)
        .threat_threshold(100.0) // high threshold to prove tripwire fires independently
        .build();

    let mut topo = Topology::new(6);
    // Mainland (0, 1, 2)
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Single-token island (3) with exactly 1 link to system node 4
    topo.add_edge(3, 4);

    let res = pruner.prune(&topo, 4).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::FatalBlock,
        "Micro-steering single token must trigger FatalBlock"
    );
    assert_eq!(res.island_nodes, vec![3]);
}

/// Test micro-steering tripwire boundary condition:
/// When to_system >= 2.0 (e.g. 2 edges to system nodes), single-token tripwire does NOT fire.
/// If ratio <= threshold and neglect >= 0.1, it resolves to GarbageCollect.
#[test]
fn test_adversarial_single_token_tripwire_boundary_to_system_2_bypass() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(4)
        .threat_threshold(10.0)
        .build();

    let mut topo = Topology::new(7);
    // Mainland (0, 1, 2)
    topo.add_edge(0, 1);
    topo.add_edge(1, 2);
    topo.add_edge(2, 0);

    // Single-token island (3) with 2 edges to system nodes 4 and 5
    // internal = 0, to_system = 2.0, island_len = 1.0, system_len = 5.0
    // is_control_vector: to_system < 2.0 is FALSE (to_system == 2.0)
    // neglect = 2.0 / 1.0 = 2.0 >= 0.1 (PASS)
    // ratio = (0 * 5) / (2 * 1) = 0.0 <= 10.0 (PASS)
    topo.add_edge(3, 4);
    topo.add_edge(3, 5);

    let res = pruner.prune(&topo, 5).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::GarbageCollect,
        "Single token with to_system == 2.0 should bypass tripwire and resolve to GarbageCollect"
    );
    assert_eq!(res.island_nodes, vec![3]);
}

/// Test instruction neglect decoupled clusters:
/// When to_system / N_island < 0.1, triggers FATAL_BLOCK.
#[test]
fn test_adversarial_instruction_neglect_sub_threshold() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(25)
        .threat_threshold(100.0) // high threshold to isolate neglect metric
        .build();

    let mut topo = Topology::new(30);
    // Mainland (0..10)
    for i in 0..10 {
        for j in (i + 1)..10 {
            topo.add_edge(i, j);
        }
    }
    // Connect mainland to system
    topo.add_edge(0, 25);
    topo.add_edge(1, 26);

    // Large island cluster (10..25) of 15 nodes connected in a cycle
    for i in 10..25 {
        topo.add_edge(i, (i + 1 - 10) % 15 + 10);
    }

    // Connect only 1 edge from island to system node 25
    // N_island = 15, to_system = 1
    // neglect = 1 / 15 = 0.0667 < 0.10 => FATAL_BLOCK
    topo.add_edge(10, 25);

    let res = pruner.prune(&topo, 28).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::FatalBlock,
        "Instruction neglect (neglect = 0.0667 < 0.10) must trigger FatalBlock"
    );
}

/// Test instruction neglect exact boundary:
/// N_island = 10, to_system = 1 => neglect = 1/10 = 0.10 (not < 0.10).
/// When ratio is configured <= threshold, should resolve to GarbageCollect.
#[test]
fn test_adversarial_instruction_neglect_at_threshold_exact_boundary() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .system_start_idx(30)
        .threat_threshold(100.0) // Set threshold high enough so ratio = 54.0 does not trigger FatalBlock
        .build();

    let mut topo = Topology::new(31);
    // Mainland (0..20) - 20 nodes
    for i in 0..20 {
        for j in (i + 1)..20 {
            topo.add_edge(i, j);
        }
    }
    topo.add_edge(0, 30);

    // Island cluster (20..30) - 10 nodes connected in a path: 20..29 (9 edges -> internal = 18)
    for i in 20..29 {
        topo.add_edge(i, i + 1);
    }

    // Island has 1 connection to system node 30: to_system = 1
    // N_island = 10, to_system = 1
    // neglect = 1 / 10 = 0.10 (neglect < 0.1 is false!)
    // internal = 18
    // system_len = 30
    // ratio = (18 * 30) / (1 * 10) = 540 / 10 = 54.0 <= 100.0
    // is_control_vector = false (island_len == 10)
    // => GarbageCollect
    topo.add_edge(20, 30);

    let res = pruner.prune(&topo, 30).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::GarbageCollect,
        "Neglect == 0.10 is at boundary (not < 0.10) and should GarbageCollect"
    );
    assert_eq!(res.island_nodes.len(), 10);
}

/// Test dense backdoor cliques (Scale-Invariant Density Ratio > threshold):
/// Ratio = (internal * N_system) / (to_system * N_island)
#[test]
fn test_adversarial_dense_backdoor_clique_fatal_block() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(2.0)
        .system_start_idx(10)
        .build();

    let mut topo = Topology::new(15);
    // Mainland (0..4)
    for i in 0..4 {
        for j in (i + 1)..4 {
            topo.add_edge(i, j);
        }
    }
    topo.add_edge(0, 10);

    // Highly dense backdoor clique on island (4..9) of 5 nodes -> 10 edges (internal = 20)
    for i in 4..9 {
        for j in (i + 1)..9 {
            topo.add_edge(i, j);
        }
    }

    // Island connected weakly to system node 10 and 11 (to_system = 2)
    topo.add_edge(4, 10);
    topo.add_edge(4, 11);

    // island_len = 5, system_len = 12, internal = 20, to_system = 2
    // neglect = 2 / 5 = 0.4 >= 0.1
    // ratio = (20 * 12) / (2 * 5) = 240 / 10 = 24.0 > 2.0 threshold => FatalBlock
    let res = pruner.prune(&topo, 12).unwrap();
    assert_eq!(
        res.action,
        PolicyAction::FatalBlock,
        "Dense backdoor clique with ratio=24.0 > 2.0 must trigger FatalBlock"
    );
}

/// Test scale-invariance of the Semantic Density Ratio metric
#[test]
fn test_adversarial_scale_invariance_proportional_behavior() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(15.0)
        .system_start_idx(10)
        .build();

    // Scale 1: Ratio 18.0 > 15.0 -> FatalBlock
    let mut topo1 = Topology::new(15);
    for i in 0..4 {
        for j in (i + 1)..4 {
            topo1.add_edge(i, j);
        }
    }
    // Island (4..8) K4 clique: 6 edges
    for i in 4..8 {
        for j in (i + 1)..8 {
            topo1.add_edge(i, j);
        }
    }
    topo1.add_edge(4, 10);
    topo1.add_edge(5, 11);

    let res1 = pruner.prune(&topo1, 12).unwrap();
    // island_len = 4, system_len = 12, internal = 12, to_system = 2
    // ratio = (12 * 12) / (2 * 4) = 144 / 8 = 18.0 > 15.0 => FatalBlock
    assert_eq!(res1.action, PolicyAction::FatalBlock);
}

/// Test benign cluster garbage-collect verdict
#[test]
fn test_adversarial_benign_cluster_garbage_collect_verdict() {
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(50.0) // high threshold
        .system_start_idx(8)
        .build();

    let mut topo = Topology::new(12);
    // Mainland (0..4)
    for i in 0..4 {
        for j in (i + 1)..4 {
            topo.add_edge(i, j);
        }
    }

    // Benign island (4..8) with simple ring: 4 nodes, 4 edges (internal = 8)
    for i in 4..8 {
        topo.add_edge(i, (i + 1 - 4) % 4 + 4);
    }

    // Island well-connected to system (4 system edges to nodes 8, 9)
    topo.add_edge(4, 8);
    topo.add_edge(5, 8);
    topo.add_edge(6, 9);
    topo.add_edge(7, 9);

    let res = pruner.prune(&topo, 10).unwrap();
    // island_len = 4, system_len = 10, internal = 8, to_system = 4
    // neglect = 4 / 4 = 1.0 >= 0.1
    // ratio = (8 * 10) / (4 * 4) = 80 / 16 = 5.0 <= 50.0
    // is_control_vector = false (island_len == 4)
    // => GarbageCollect
    assert_eq!(res.action, PolicyAction::GarbageCollect);
    assert_eq!(res.island_nodes.len(), 4);
}

// =========================================================================
// SECTION 3: VALIDATION ERRORS & PARAMETER BOUNDS TESTS
// =========================================================================

#[test]
fn test_validation_tolerance_bounds() {
    // Zero tolerance
    let err_zero = TauSpectralPruner::builder().tolerance(0.0).try_build();
    assert!(matches!(err_zero, Err(PrunerError::MathError(_))));

    // Negative tolerance
    let err_neg = TauSpectralPruner::builder().tolerance(-1e-6).try_build();
    assert!(matches!(err_neg, Err(PrunerError::MathError(_))));

    // NaN tolerance
    let err_nan = TauSpectralPruner::builder()
        .tolerance(f64::NAN)
        .try_build();
    assert!(matches!(err_nan, Err(PrunerError::MathError(_))));

    // Positive tolerance must succeed
    assert!(TauSpectralPruner::builder().tolerance(1e-12).try_build().is_ok());
}

#[test]
fn test_validation_max_iterations_bounds() {
    // Zero max iterations
    let err_zero = TauSpectralPruner::builder().max_iterations(0).try_build();
    assert!(matches!(err_zero, Err(PrunerError::MathError(_))));

    // Non-zero max iterations must succeed
    assert!(TauSpectralPruner::builder().max_iterations(1).try_build().is_ok());
    assert!(TauSpectralPruner::builder().max_iterations(100_000).try_build().is_ok());
}

#[test]
fn test_validation_momentum_beta_bounds() {
    // Negative beta
    let err_neg = TauSpectralPruner::builder().momentum_beta(-0.01).try_build();
    assert!(matches!(err_neg, Err(PrunerError::MathError(_))));

    // Beta == 1.0 (boundary exclusion: must be in [0.0, 1.0))
    let err_one = TauSpectralPruner::builder().momentum_beta(1.0).try_build();
    assert!(matches!(err_one, Err(PrunerError::MathError(_))));

    // Beta > 1.0
    let err_large = TauSpectralPruner::builder().momentum_beta(1.05).try_build();
    assert!(matches!(err_large, Err(PrunerError::MathError(_))));

    // NaN beta
    let err_nan = TauSpectralPruner::builder()
        .momentum_beta(f64::NAN)
        .try_build();
    assert!(matches!(err_nan, Err(PrunerError::MathError(_))));

    // Valid beta boundaries [0.0, 1.0)
    assert!(TauSpectralPruner::builder().momentum_beta(0.0).try_build().is_ok());
    assert!(TauSpectralPruner::builder().momentum_beta(0.9999).try_build().is_ok());
}

#[test]
fn test_validation_threat_threshold_bounds() {
    // Negative threat threshold
    let err_neg = TauSpectralPruner::builder()
        .threat_threshold(-0.001)
        .try_build();
    assert!(matches!(err_neg, Err(PrunerError::MathError(_))));

    // NaN threat threshold
    let err_nan = TauSpectralPruner::builder()
        .threat_threshold(f64::NAN)
        .try_build();
    assert!(matches!(err_nan, Err(PrunerError::MathError(_))));

    // Non-negative threat threshold must succeed
    assert!(TauSpectralPruner::builder().threat_threshold(0.0).try_build().is_ok());
    assert!(TauSpectralPruner::builder().threat_threshold(1000.0).try_build().is_ok());
}

#[test]
#[should_panic(expected = "Invalid PrunerBuilder configuration: Mathematical solver failure: Tolerance must be strictly positive")]
fn test_validation_build_panic_on_negative_tolerance() {
    let _ = TauSpectralPruner::builder().tolerance(-1.0).build();
}

#[test]
#[should_panic(expected = "Invalid PrunerBuilder configuration: Mathematical solver failure: max_iterations must be greater than 0")]
fn test_validation_build_panic_on_zero_max_iterations() {
    let _ = TauSpectralPruner::builder().max_iterations(0).build();
}

#[test]
#[should_panic(expected = "Invalid PrunerBuilder configuration: Mathematical solver failure: Momentum beta must be in [0.0, 1.0)")]
fn test_validation_build_panic_on_invalid_momentum_beta() {
    let _ = TauSpectralPruner::builder().momentum_beta(1.0).build();
}

#[test]
#[should_panic(expected = "Invalid PrunerBuilder configuration: Mathematical solver failure: threat_threshold must be non-negative")]
fn test_validation_build_panic_on_negative_threat_threshold() {
    let _ = TauSpectralPruner::builder().threat_threshold(-1.0).build();
}

// =========================================================================
// SECTION 4: PROPERTY-BASED RANDOMIZED HARNESSES (1,000+ ITERATIONS)
// =========================================================================

/// Invariant 1: Telemetry Separation Invariant
/// For any graph, boundary parameters, sinks, and edges, no system node
/// in [system_start_idx, system_boundary_len] (when boundary_len > 0)
/// shall ever be present in mainland_nodes or island_nodes.
#[test]
fn test_property_1_telemetry_separation_fuzz_1000() {
    let mut rng = FuzzRng::new(0xABCDEF0123456789);
    let mut workspace = PrunerWorkspace::with_capacity(100, 300);

    for iter in 0..1000 {
        let n = rng.gen_range(0, 50);
        let mut topo = Topology::new(n);

        if n > 0 {
            let sinks = rng.gen_range(0, n / 4 + 1);
            for _ in 0..sinks {
                topo.add_sink(rng.gen_range(0, n));
            }
            let edges = rng.gen_range(0, n * 2 + 5);
            for _ in 0..edges {
                topo.edges.push((rng.gen_range(0, n), rng.gen_range(0, n)));
            }
        }

        let sys_start = rng.gen_range(0, 60);
        let sys_len = rng.gen_range(0, 60);

        let pruner = TauSpectralPruner::builder()
            .system_start_idx(sys_start)
            .build();

        let res = pruner
            .prune_with_workspace(&topo, sys_len, &mut workspace)
            .unwrap_or_else(|e| panic!("Iter {} failed: {:?}", iter, e));

        if sys_len > 0 && sys_start <= sys_len {
            for &node in &res.mainland_nodes {
                assert!(
                    !(sys_start..=sys_len).contains(&node),
                    "Iter {}: System node {} leaked in mainland! Range: [{}, {}]",
                    iter,
                    node,
                    sys_start,
                    sys_len
                );
            }
            for &node in &res.island_nodes {
                assert!(
                    !(sys_start..=sys_len).contains(&node),
                    "Iter {}: System node {} leaked in island! Range: [{}, {}]",
                    iter,
                    node,
                    sys_start,
                    sys_len
                );
            }
        }
    }
}

/// Invariant 2: Partition Conservation & Sink Isolation
/// Active non-system nodes must be partitioned into either mainland or island (disjoint),
/// and no sink nodes may ever appear in either output partition.
#[test]
fn test_property_2_partition_conservation_and_sink_isolation_fuzz_1000() {
    let mut rng = FuzzRng::new(0x7766554433221100);
    let mut workspace = PrunerWorkspace::new();

    for iter in 0..1000 {
        let n = rng.gen_range(0, 45);
        let mut topo = Topology::new(n);

        let mut sinks = BTreeSet::new();
        if n > 0 {
            let num_sinks = rng.gen_range(0, n / 3 + 1);
            for _ in 0..num_sinks {
                let s = rng.gen_range(0, n);
                topo.add_sink(s);
                sinks.insert(s);
            }
            let edges = rng.gen_range(0, n * 3);
            for _ in 0..edges {
                topo.add_edge(rng.gen_range(0, n), rng.gen_range(0, n));
            }
        }

        let sys_start = rng.gen_range(0, n + 5);
        let sys_len = rng.gen_range(0, n + 5);

        let is_sys = |i: usize| -> bool {
            sys_len > 0 && i >= sys_start && i <= sys_len
        };

        let pruner = TauSpectralPruner::builder()
            .tau(rng.gen_f64() * 2.0 - 1.0)
            .system_start_idx(sys_start)
            .build();

        let res = pruner
            .prune_with_workspace(&topo, sys_len, &mut workspace)
            .unwrap();

        let mainland_set: BTreeSet<usize> = res.mainland_nodes.iter().copied().collect();
        let island_set: BTreeSet<usize> = res.island_nodes.iter().copied().collect();

        // 1. Disjointness
        for node in &island_set {
            assert!(
                !mainland_set.contains(node),
                "Iter {}: Node {} in both mainland and island!",
                iter,
                node
            );
        }

        // 2. Sink isolation
        for &s in &sinks {
            assert!(
                !mainland_set.contains(&s),
                "Iter {}: Sink node {} in mainland!",
                iter,
                s
            );
            assert!(
                !island_set.contains(&s),
                "Iter {}: Sink node {} in island!",
                iter,
                s
            );
        }

        // 3. Node conservation
        let mut expected_active_non_system = 0usize;
        for i in 0..n {
            if !sinks.contains(&i) && !is_sys(i) {
                expected_active_non_system += 1;
                assert!(
                    mainland_set.contains(&i) || island_set.contains(&i),
                    "Iter {}: Active non-system node {} missing from partitions!",
                    iter,
                    i
                );
            }
        }

        assert_eq!(
            mainland_set.len() + island_set.len(),
            expected_active_non_system,
            "Iter {}: Partition conservation mismatch!",
            iter
        );
    }
}

/// Invariant 3: Workspace Determinism & Streaming State Purity
/// Continuous execution through a reused workspace yields bit-for-bit identical results
/// to fresh one-shot execution across 1,000 diverse graphs.
#[test]
fn test_property_3_streaming_workspace_exact_parity_fuzz_1000() {
    let mut rng = FuzzRng::new(0x13579BDF02468ACE);
    let mut workspace = PrunerWorkspace::new();

    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(2.5)
        .max_iterations(800)
        .system_start_idx(4)
        .build();

    for iter in 0..1000 {
        let n = rng.gen_range(0, 40);
        let mut topo = Topology::new(n);

        if n > 0 {
            let num_sinks = rng.gen_range(0, n / 4 + 1);
            for _ in 0..num_sinks {
                topo.add_sink(rng.gen_range(0, n));
            }
            let edges = rng.gen_range(0, n * 2);
            for _ in 0..edges {
                topo.add_edge(rng.gen_range(0, n), rng.gen_range(0, n));
            }
        }

        let sys_len = rng.gen_range(0, 20);

        let res_ws = pruner
            .prune_with_workspace(&topo, sys_len, &mut workspace)
            .unwrap();
        let res_direct = pruner.prune(&topo, sys_len).unwrap();

        assert_eq!(
            res_ws, res_direct,
            "Iter {}: Workspace reuse divergence from direct prune!",
            iter
        );
    }
}
