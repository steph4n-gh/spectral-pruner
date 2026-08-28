//! ⚡ Adversarial Fuzzing & Property Invariant Harness
//!
//! Executes 10,000+ randomized, adversarial topological fuzzing iterations
//! using a pure-Rust deterministic PRNG (zero external dependencies).
//!
//! Mathematical invariants verified across all 10,000+ topologies:
//! 1. Partition Conservation: V_mainland ⊎ V_island = V_active_non_system
//! 2. Partition Disjointness: V_mainland ∩ V_island = ∅
//! 3. Sink Isolation: Sinks are strictly excluded from output partitions
//! 4. Telemetry Separation: System boundary nodes are strictly excluded from output partitions
//! 5. Algebraic Connectivity Bound: λ_2 >= -1e-9 and !λ_2.is_nan()
//! 6. Zero-Allocation Workspace Parity: prune() == prune_with_workspace()
//! 7. Zero Panic Guarantee: No unexpected unwrap failures or index out of bounds

use spectral_pruner::{BitSet, CsrGraph, PrunerWorkspace, TauSpectralPruner, Topology};

/// Pure-Rust deterministic 64-bit Linear Congruential Generator (LCG)
struct AdversarialLcg {
    state: u64,
}

impl AdversarialLcg {
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
    fn next_bool(&mut self, p_numer: u64, p_denom: u64) -> bool {
        (self.next_u64() % p_denom) < p_numer
    }

    #[inline]
    fn next_f64_range(&mut self, min: f64, max: f64) -> f64 {
        let frac = (self.next_u64() & 0xFFFFFFFFFFFF) as f64 / 281474976710656.0;
        min + frac * (max - min)
    }
}

#[test]
fn test_fuzz_10000_adversarial_topologies_and_invariant_conservation() {
    let mut rng = AdversarialLcg::new(0xDEADBEEFCAFE1337);
    let mut workspace = PrunerWorkspace::with_capacity(128, 256);

    let total_iterations = 10_000;

    for iter in 0..total_iterations {
        // Generate random node count N in [0, 80] with emphasis on boundary sizes
        let n = if iter < 100 {
            iter % 10 // small graphs 0..9
        } else if iter % 50 == 0 {
            0 // empty graph
        } else {
            3 + rng.next_usize(70)
        };

        let mut topo = Topology::new(n);

        // Edge generation: mix of sparse, dense, cyclic, star, and disconnected
        if n > 1 {
            let edge_density_mode = rng.next_usize(5);
            match edge_density_mode {
                0 => {
                    // Sparse random edges
                    let e_count = rng.next_usize(n * 2);
                    for _ in 0..e_count {
                        topo.add_edge(rng.next_usize(n + 3), rng.next_usize(n + 3));
                    }
                }
                1 => {
                    // Star-like topology
                    let hub = rng.next_usize(n);
                    for leaf in 0..n {
                        if leaf != hub && rng.next_bool(3, 4) {
                            topo.add_edge(hub, leaf);
                        }
                    }
                }
                2 => {
                    // Chain / Cycle
                    for i in 0..n {
                        topo.add_edge(i, (i + 1) % n);
                    }
                }
                3 => {
                    // Clustered components
                    let mid = n / 2;
                    if mid > 0 {
                        for i in 0..mid {
                            for j in (i + 1)..mid {
                                if rng.next_bool(1, 2) {
                                    topo.add_edge(i, j);
                                }
                            }
                        }
                        for i in mid..n {
                            for j in (i + 1)..n {
                                if rng.next_bool(1, 2) {
                                    topo.add_edge(i, j);
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Random graph
                    let e_count = rng.next_usize(n * 3);
                    for _ in 0..e_count {
                        topo.add_edge(rng.next_usize(n), rng.next_usize(n));
                    }
                }
            }
        }

        // Add random sinks
        if n > 0 {
            let sink_count = rng.next_usize(n / 3 + 1);
            for _ in 0..sink_count {
                topo.add_sink(rng.next_usize(n + 2));
            }
        }

        // Configure system boundary
        let sys_start = rng.next_usize(n + 2);
        let sys_len = if rng.next_bool(1, 5) {
            0
        } else {
            sys_start + rng.next_usize(n + 5)
        };

        // Randomize pruner builder parameters
        let tau = rng.next_f64_range(-0.5, 0.5);
        let threat_threshold = rng.next_f64_range(0.5, 5.0);
        let momentum_beta = rng.next_f64_range(0.0, 0.8);
        let max_iterations = 20 + rng.next_usize(50); // fast iteration budget for 10k runs

        let pruner = TauSpectralPruner::builder()
            .tau(tau)
            .threat_threshold(threat_threshold)
            .momentum_beta(momentum_beta)
            .max_iterations(max_iterations)
            .tolerance(1e-5)
            .system_start_idx(sys_start)
            .build();

        // 1. Zero Panic Guarantee
        let res_ws = pruner.prune_with_workspace(&topo, sys_len, &mut workspace);
        assert!(
            res_ws.is_ok(),
            "prune_with_workspace failed on iter {}: {:?}",
            iter,
            res_ws.err()
        );
        let res = res_ws.unwrap();

        // 2. Parity with prune() on sampled iterations
        if iter % 200 == 0 {
            let res_direct = pruner.prune(&topo, sys_len).unwrap();
            assert_eq!(res, res_direct, "Workspace parity failure on iter {}", iter);
        }

        // 3. Algebraic connectivity bound
        assert!(
            !res.connectivity_score.is_nan(),
            "λ_2 is NaN on iter {}",
            iter
        );
        assert!(
            res.connectivity_score >= -1e-9,
            "λ_2 is negative ({}) on iter {}",
            res.connectivity_score,
            iter
        );

        // System node predicate
        let is_sys = |idx: usize| sys_len > 0 && idx >= sys_start && idx <= sys_len;

        // 4. Partition Disjointness
        for &m_node in &res.mainland_nodes {
            assert!(
                !res.island_nodes.contains(&m_node),
                "Disjointness violation on iter {}: node {} in both partitions",
                iter,
                m_node
            );
        }

        // 5. Sink Isolation
        for &s_node in &topo.sinks {
            assert!(
                !res.mainland_nodes.contains(&s_node),
                "Sink isolation violation (mainland) on iter {}: sink {}",
                iter,
                s_node
            );
            assert!(
                !res.island_nodes.contains(&s_node),
                "Sink isolation violation (island) on iter {}: sink {}",
                iter,
                s_node
            );
        }

        // 6. Telemetry Separation
        for &m_node in &res.mainland_nodes {
            assert!(
                !is_sys(m_node),
                "Telemetry separation violation (mainland) on iter {}: system node {}",
                iter,
                m_node
            );
        }
        for &i_node in &res.island_nodes {
            assert!(
                !is_sys(i_node),
                "Telemetry separation violation (island) on iter {}: system node {}",
                iter,
                i_node
            );
        }

        // 7. Partition Conservation
        let mut expected_active_non_system: Vec<usize> = (0..n)
            .filter(|&i| !topo.sinks.contains(&i) && !is_sys(i))
            .collect();
        expected_active_non_system.sort();

        let mut actual_partition_union = res.mainland_nodes.clone();
        actual_partition_union.extend_from_slice(&res.island_nodes);
        actual_partition_union.sort();

        assert_eq!(
            actual_partition_union, expected_active_non_system,
            "Partition conservation violation on iter {}: expected {:?}, got {:?}",
            iter, expected_active_non_system, actual_partition_union
        );
    }
}

#[test]
fn test_fuzz_csr_symmetry_and_degree_conservation_5000() {
    let mut rng = AdversarialLcg::new(0x1337BEEFCAFED00D);
    let mut row_ptrs = Vec::new();
    let mut col_indices = Vec::new();
    let mut degrees = Vec::new();
    let mut cursor = Vec::new();
    let mut sink_bits = BitSet::new(0);

    for _ in 0..5000 {
        let n = 2 + rng.next_usize(60);
        let mut topo = Topology::new(n);
        let e = rng.next_usize(n * 3);
        for _ in 0..e {
            topo.add_edge(rng.next_usize(n + 3), rng.next_usize(n + 3));
        }
        let s = rng.next_usize(n / 4 + 1);
        for _ in 0..s {
            topo.add_sink(rng.next_usize(n + 3));
        }

        topo.populate_sink_bitset(&mut sink_bits);

        CsrGraph::compile_into(
            &topo,
            &sink_bits,
            &mut row_ptrs,
            &mut col_indices,
            &mut degrees,
            &mut cursor,
        );

        let csr = CsrGraph {
            num_nodes: n,
            row_ptrs: row_ptrs.clone(),
            col_indices: col_indices.clone(),
            degrees: degrees.clone(),
        };

        // Edge symmetry: if v in neighbors(u), then u in neighbors(v)
        for u in 0..n {
            if sink_bits.contains(u) {
                assert_eq!(csr.degree(u), 0.0);
                assert_eq!(csr.neighbors(u), &[]);
                continue;
            }
            let neighbors_u = csr.neighbors(u);
            assert_eq!(neighbors_u.len() as f64, csr.degree(u));

            for &v in neighbors_u {
                assert!(!sink_bits.contains(v));
                assert_ne!(u, v, "Self-loops must be excluded from CSR");
                let neighbors_v = csr.neighbors(v);
                assert!(
                    neighbors_v.contains(&u),
                    "Undirected symmetry violation: ({}, {}) in CSR but ({}, {}) missing",
                    u,
                    v,
                    v,
                    u
                );
            }
        }
    }
}
