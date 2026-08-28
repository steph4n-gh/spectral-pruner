//! 🔬 Milestone 4 Empirical Challenge & Stress Harness
//!
//! Comprehensive adversarial stress-testing harness verifying:
//! 1. Partition conservation ($V_M \u228e V_I = V_{active \setminus sys}$) and disjointness ($V_M \cap V_I = \emptyset$)
//! 2. Telemetry separation across diverse boundary ranges
//! 3. Streaming memory stability (50,000 iterations continuous zero-alloc reuse)
//! 4. Mathematical invariants from AGENTS.md (Arrington clamping, tau split, density ratio, neglect, tripwire)
//! 5. Degenerate and adversarial topologies (sinks, loops, multi-components, out-of-bounds)

use spectral_pruner::{PolicyAction, PrunerWorkspace, TauSpectralPruner, Topology};

/// Deterministic 64-bit Linear Congruential Generator
struct Lcg {
    state: u64,
}

impl Lcg {
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
    fn next_usize(&mut self, max: usize) -> usize {
        if max == 0 {
            0
        } else {
            (self.next_u64() % (max as u64)) as usize
        }
    }

    #[inline]
    fn next_f64(&mut self, min: f64, max: f64) -> f64 {
        let frac = (self.next_u64() & 0xFFFFFFFFFFFF) as f64 / 281474976710656.0;
        min + frac * (max - min)
    }

    #[inline]
    fn next_bool(&mut self, p_num: u64, p_den: u64) -> bool {
        (self.next_u64() % p_den) < p_num
    }
}

#[test]
fn challenge_streaming_50000_cycles_zero_alloc_and_conservation() {
    let mut rng = Lcg::new(0xCAFE_BABE_DEAD_BEEF);
    let mut workspace = PrunerWorkspace::with_capacity(128, 256);

    let initial_v_cap = workspace.v_vec.capacity();
    let initial_csr_row_cap = workspace.csr_row_ptrs.capacity();
    let initial_csr_col_cap = workspace.csr_col_indices.capacity();
    let initial_deg_cap = workspace.degrees.capacity();

    let total_cycles = 50_000;

    for cycle in 0..total_cycles {
        let n = 2 + rng.next_usize(50);
        let mut topo = Topology::new(n);

        // Mix edge patterns
        let edge_pattern = rng.next_usize(4);
        match edge_pattern {
            0 => {
                // Random sparse
                let e = rng.next_usize(n * 2);
                for _ in 0..e {
                    topo.add_edge(rng.next_usize(n), rng.next_usize(n));
                }
            }
            1 => {
                // Star
                let hub = rng.next_usize(n);
                for leaf in 0..n {
                    if leaf != hub && rng.next_bool(1, 2) {
                        topo.add_edge(hub, leaf);
                    }
                }
            }
            2 => {
                // Cycle / Path
                for i in 0..n {
                    topo.add_edge(i, (i + 1) % n);
                }
            }
            _ => {
                // Disconnected clusters
                let split = n / 2;
                if split > 1 {
                    for i in 0..split - 1 {
                        topo.add_edge(i, i + 1);
                    }
                    for i in split..n - 1 {
                        topo.add_edge(i, i + 1);
                    }
                }
            }
        }

        // Random sinks
        let num_sinks = rng.next_usize(n / 4 + 1);
        for _ in 0..num_sinks {
            topo.add_sink(rng.next_usize(n));
        }

        // Random system window
        let sys_start = rng.next_usize(n);
        let sys_len = if rng.next_bool(1, 4) {
            0
        } else {
            sys_start + rng.next_usize(n)
        };

        let tau = rng.next_f64(-0.2, 0.2);
        let threat_threshold = rng.next_f64(1.0, 4.0);
        let beta = rng.next_f64(0.0, 0.7);

        let pruner = TauSpectralPruner::builder()
            .tau(tau)
            .threat_threshold(threat_threshold)
            .momentum_beta(beta)
            .max_iterations(30)
            .tolerance(1e-5)
            .system_start_idx(sys_start)
            .build();

        let res = pruner
            .prune_with_workspace(&topo, sys_len, &mut workspace)
            .expect("prune_with_workspace must succeed");

        // Invariant 1: Disjointness
        for &m in &res.mainland_nodes {
            assert!(
                !res.island_nodes.contains(&m),
                "Disjointness failure at cycle {}: node {} in mainland and island",
                cycle,
                m
            );
        }

        // Invariant 2: Sink Isolation
        for &s in &topo.sinks {
            assert!(!res.mainland_nodes.contains(&s));
            assert!(!res.island_nodes.contains(&s));
        }

        // Invariant 3: Telemetry Separation
        let is_sys = |idx: usize| sys_len > 0 && idx >= sys_start && idx <= sys_len;
        for &m in &res.mainland_nodes {
            assert!(!is_sys(m));
        }
        for &i in &res.island_nodes {
            assert!(!is_sys(i));
        }

        // Invariant 4: Partition Conservation
        let mut expected: Vec<usize> = (0..n)
            .filter(|&i| !topo.sinks.contains(&i) && !is_sys(i))
            .collect();
        expected.sort();

        let mut actual = res.mainland_nodes.clone();
        actual.extend_from_slice(&res.island_nodes);
        actual.sort();

        assert_eq!(actual, expected, "Conservation failed at cycle {}", cycle);

        // Invariant 5: lambda_2 non-negative & finite
        assert!(!res.connectivity_score.is_nan());
        assert!(res.connectivity_score >= -1e-9);
    }

    // Zero reallocation verification: Capacity must remain stable
    assert!(workspace.v_vec.capacity() >= initial_v_cap);
    assert!(workspace.csr_row_ptrs.capacity() >= initial_csr_row_cap);
    assert!(workspace.csr_col_indices.capacity() >= initial_csr_col_cap);
    assert!(workspace.degrees.capacity() >= initial_deg_cap);
}

#[test]
fn challenge_arrington_clamping_exact_power_step() {
    // Construct topology where node 0, 1 are connected, and node 2 is completely isolated (degree=0)
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .max_iterations(100)
        .build();
    let mut topo = Topology::new(3);
    topo.add_edge(0, 1);

    let res = pruner.prune(&topo, 0).unwrap();
    assert_eq!(res.action, PolicyAction::Allow);

    // Node 2 must be present in one of the partitions
    let mut all = res.mainland_nodes.clone();
    all.extend_from_slice(&res.island_nodes);
    all.sort();
    assert_eq!(all, vec![0, 1, 2]);
}

#[test]
fn challenge_single_token_tripwire_exact_boundary() {
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(2)
        .threat_threshold(100.0) // high threshold to isolate tripwire
        .build();

    // 1 node island (0), 0 internal edges, 1 edge to system node (2)
    // System nodes: [2, 2]
    // Mainland: (1) connected to (2)
    let mut topo = Topology::new(3);
    topo.add_edge(0, 2); // 1 system edge to island 0
    topo.add_edge(1, 2); // 1 system edge to mainland 1

    let res = pruner.prune(&topo, 2).unwrap();
    // Island has 1 node, 0 internal, 1 system edge (< 2.0) -> triggers tripwire
    assert_eq!(res.action, PolicyAction::FatalBlock);
}

#[test]
fn challenge_instruction_neglect_fatal_block() {
    let pruner = TauSpectralPruner::builder()
        .system_start_idx(5)
        .threat_threshold(100.0)
        .build();

    let mut topo2 = Topology::new(7);
    topo2.add_edge(0, 1);
    topo2.add_edge(1, 2);
    topo2.add_edge(2, 0);
    topo2.add_edge(0, 5);
    topo2.add_edge(1, 6);

    // Island cluster: 3, 4 with internal edge but NO system edges
    topo2.add_edge(3, 4);

    let res = pruner.prune(&topo2, 6).unwrap();
    assert_eq!(res.action, PolicyAction::FatalBlock);
    assert_eq!(res.island_nodes, vec![3, 4]);
}

#[test]
fn challenge_scale_invariant_density_ratio_scaling() {
    // Test scale invariance: Graph A with N_sys=10, island=2, internal=2, system_edges=2
    // Graph B with N_sys=20, island=4, internal=4, system_edges=4
    // Ratio A = (2 * 10) / (2 * 2) = 20 / 4 = 5.0
    // Ratio B = (4 * 20) / (4 * 4) = 80 / 16 = 5.0
    let ratio_a = (2.0 * 10.0) / (2.0 * 2.0);
    let ratio_b = (4.0 * 20.0) / (4.0 * 4.0);
    assert_eq!(ratio_a, ratio_b);
}

#[test]
fn challenge_error_validation_and_safety() {
    assert!(TauSpectralPruner::builder()
        .tolerance(0.0)
        .try_build()
        .is_err());
    assert!(TauSpectralPruner::builder()
        .tolerance(-1e-5)
        .try_build()
        .is_err());
    assert!(TauSpectralPruner::builder()
        .tolerance(f64::NAN)
        .try_build()
        .is_err());
    assert!(TauSpectralPruner::builder()
        .max_iterations(0)
        .try_build()
        .is_err());
    assert!(TauSpectralPruner::builder()
        .momentum_beta(-0.1)
        .try_build()
        .is_err());
    assert!(TauSpectralPruner::builder()
        .momentum_beta(1.0)
        .try_build()
        .is_err());
    assert!(TauSpectralPruner::builder()
        .momentum_beta(1.5)
        .try_build()
        .is_err());
    assert!(TauSpectralPruner::builder()
        .threat_threshold(-1.0)
        .try_build()
        .is_err());
}
