//! Empirical Challenger Stress Test Suite for Milestone 1
//! Testing BitSet and CsrGraph data structures.

use spectral_pruner::engine::Topology;
use spectral_pruner::graph::{BitSet, CsrGraph};
use std::collections::BTreeSet;

#[test]
fn test_bitset_word_boundaries_and_extreme_sizes() {
    let test_sizes = [
        0, 1, 2, 63, 64, 65, 127, 128, 129, 191, 192, 193, 255, 256, 257, 1024, 65536,
    ];

    for &len in &test_sizes {
        let mut bs = BitSet::new(len);
        assert_eq!(bs.len(), len);
        let expected_words = if len == 0 { 0 } else { (len - 1) / 64 + 1 };
        assert_eq!(bs.words.len(), expected_words);
        assert_eq!(bs.count_ones(), 0);
        assert!(bs.iter_ones().next().is_none());

        if len == 0 {
            assert!(bs.is_empty());
            assert!(!bs.contains(0));
            assert!(!bs.contains(1));
            assert!(!bs.contains(usize::MAX));
            bs.insert(0);
            bs.insert(100);
            bs.insert(usize::MAX);
            assert_eq!(bs.count_ones(), 0);
            assert!(!bs.contains(0));
            assert!(!bs.contains(usize::MAX));
            bs.remove(0);
            bs.remove(usize::MAX);
            continue;
        }

        assert!(!bs.is_empty());

        // Collect unique boundary indices within [0, len)
        let candidate_indices = [
            0,
            1,
            62,
            63,
            64,
            65,
            126,
            127,
            128,
            129,
            191,
            192,
            193,
            255,
            256,
            257,
            len.saturating_sub(2),
            len.saturating_sub(1),
        ];

        let mut unique_boundaries = BTreeSet::new();
        for &idx in &candidate_indices {
            if idx < len {
                unique_boundaries.insert(idx);
            }
        }

        // Verify initially unset
        for &idx in &unique_boundaries {
            assert!(
                !bs.contains(idx),
                "Bit {} should be unset initially for len {}",
                idx,
                len
            );
        }

        // Insert and verify one by one
        let mut count = 0;
        for &idx in &unique_boundaries {
            bs.insert(idx);
            count += 1;
            assert!(
                bs.contains(idx),
                "Bit {} should be set after insert for len {}",
                idx,
                len
            );
            assert_eq!(bs.count_ones(), count);
        }

        // Verify iteration order and exact content
        let iterated: Vec<usize> = bs.iter_ones().collect();
        let expected: Vec<usize> = unique_boundaries.iter().copied().collect();
        assert_eq!(iterated, expected, "Mismatch at len {}", len);

        // Out of bounds tests
        let oob_indices = [
            len,
            len + 1,
            len + 63,
            len + 64,
            len + 1000,
            usize::MAX - 1,
            usize::MAX,
        ];
        for &oob in &oob_indices {
            assert!(
                !bs.contains(oob),
                "OOB idx {} should return false for len {}",
                oob,
                len
            );
            bs.insert(oob);
            assert_eq!(
                bs.count_ones(),
                count,
                "OOB insert on {} altered count",
                oob
            );
            assert!(
                !bs.contains(oob),
                "OOB idx {} should remain false after insert",
                oob
            );
        }

        // Test remove
        for &idx in &unique_boundaries {
            bs.remove(idx);
            count -= 1;
            assert!(!bs.contains(idx));
            assert_eq!(bs.count_ones(), count);
        }
        assert_eq!(bs.count_ones(), 0);

        // Test OOB remove
        for &oob in &oob_indices {
            bs.remove(oob);
            assert_eq!(bs.count_ones(), 0);
        }
    }
}

#[test]
fn test_bitset_dense_alternating_and_full() {
    let len = 300;
    let mut bs = BitSet::new(len);

    // Set all even bits
    for i in (0..len).step_by(2) {
        bs.insert(i);
    }
    assert_eq!(bs.count_ones(), 150);

    for i in 0..len {
        assert_eq!(bs.contains(i), i % 2 == 0);
    }

    let ones: Vec<usize> = bs.iter_ones().collect();
    let expected: Vec<usize> = (0..len).filter(|x| x % 2 == 0).collect();
    assert_eq!(ones, expected);

    // Set all odd bits
    for i in (1..len).step_by(2) {
        bs.insert(i);
    }
    assert_eq!(bs.count_ones(), len);

    for i in 0..len {
        assert!(bs.contains(i));
    }

    // Clear
    bs.clear();
    assert_eq!(bs.count_ones(), 0);
    assert!(bs.iter_ones().next().is_none());
    for i in 0..len {
        assert!(!bs.contains(i));
    }
}

#[test]
fn test_bitset_reset_with_len_reusability() {
    let mut bs = BitSet::new(500);
    bs.insert(10);
    bs.insert(499);
    assert_eq!(bs.count_ones(), 2);

    // Reset to smaller
    bs.reset_with_len(10);
    assert_eq!(bs.len(), 10);
    assert_eq!(bs.words.len(), 1);
    assert_eq!(bs.count_ones(), 0);
    assert!(!bs.contains(10));
    bs.insert(5);
    assert!(bs.contains(5));
    assert_eq!(bs.count_ones(), 1);

    // Reset to 0
    bs.reset_with_len(0);
    assert_eq!(bs.len(), 0);
    assert_eq!(bs.words.len(), 0);
    assert!(bs.is_empty());
    assert_eq!(bs.count_ones(), 0);

    // Reset to large
    bs.reset_with_len(10000);
    assert_eq!(bs.len(), 10000);
    assert_eq!(bs.words.len(), (10000 - 1) / 64 + 1);
    assert_eq!(bs.count_ones(), 0);
    bs.insert(9999);
    assert!(bs.contains(9999));
    assert_eq!(bs.count_ones(), 1);
}

#[test]
fn test_bitset_adversarial_constructors_and_iterator_exhaustion() {
    // from_slice with duplicate and OOB elements
    let raw = vec![0, 63, 64, 127, 128, 500, usize::MAX, 64, 0];
    let bs = BitSet::from_slice(129, &raw);
    assert_eq!(bs.len(), 129);
    assert_eq!(bs.count_ones(), 5); // 0, 63, 64, 127, 128
    assert!(bs.contains(0));
    assert!(bs.contains(63));
    assert!(bs.contains(64));
    assert!(bs.contains(127));
    assert!(bs.contains(128));
    assert!(!bs.contains(500));
    assert!(!bs.contains(usize::MAX));

    // Iterator exhaustion / fused behavior check
    let mut it = bs.iter_ones();
    assert_eq!(it.next(), Some(0));
    assert_eq!(it.next(), Some(63));
    assert_eq!(it.next(), Some(64));
    assert_eq!(it.next(), Some(127));
    assert_eq!(it.next(), Some(128));
    assert_eq!(it.next(), None);
    assert_eq!(it.next(), None); // Calling again after None
    assert_eq!(it.next(), None);

    // from_iter parity
    let bs2 = BitSet::from_iter(129, raw);
    assert_eq!(bs, bs2);
}

#[test]
fn test_csr_graph_boundary_n_0_1_2() {
    // N = 0
    let mut topo0 = Topology::new(0);
    topo0.edges.push((0, 1));
    topo0.edges.push((100, 200));
    topo0.add_sink(0);
    let sinks0 = BitSet::new(0);
    let csr0 = CsrGraph::from_topology(&topo0, &sinks0);
    assert_eq!(csr0.num_nodes, 0);
    assert_eq!(csr0.row_ptrs, vec![0]);
    assert_eq!(csr0.col_indices.len(), 0);
    assert_eq!(csr0.degrees.len(), 0);
    assert_eq!(csr0.max_degree(), 0.0);
    assert_eq!(csr0.neighbors(0), &[]);
    assert_eq!(csr0.neighbors(usize::MAX), &[]);
    assert_eq!(csr0.degree(0), 0.0);
    assert_eq!(csr0.degree(usize::MAX), 0.0);

    // N = 1
    let mut topo1 = Topology::new(1);
    topo1.add_edge(0, 0); // self-loop
    topo1.edges.push((0, 5)); // OOB
    topo1.edges.push((5, 0)); // OOB
    topo1.edges.push((usize::MAX, usize::MAX)); // extreme OOB
    let sinks1 = BitSet::new(1);
    let csr1 = CsrGraph::from_topology(&topo1, &sinks1);
    assert_eq!(csr1.num_nodes, 1);
    assert_eq!(csr1.row_ptrs, vec![0, 0]);
    assert_eq!(csr1.col_indices.len(), 0);
    assert_eq!(csr1.degrees, vec![0.0]);
    assert_eq!(csr1.max_degree(), 0.0);
    assert_eq!(csr1.neighbors(0), &[]);
    assert_eq!(csr1.neighbors(1), &[]);
    assert_eq!(csr1.degree(0), 0.0);
    assert_eq!(csr1.degree(1), 0.0);

    // N = 2
    let mut topo2 = Topology::new(2);
    topo2.add_edge(0, 1);
    topo2.add_edge(0, 0); // self loop
    topo2.add_edge(1, 1); // self loop
    topo2.edges.push((0, usize::MAX));
    let sinks2 = BitSet::new(2);
    let csr2 = CsrGraph::from_topology(&topo2, &sinks2);
    assert_eq!(csr2.num_nodes, 2);
    assert_eq!(csr2.row_ptrs, vec![0, 1, 2]);
    assert_eq!(csr2.col_indices, vec![1, 0]);
    assert_eq!(csr2.degrees, vec![1.0, 1.0]);
    assert_eq!(csr2.max_degree(), 1.0);
    assert_eq!(csr2.neighbors(0), &[1]);
    assert_eq!(csr2.neighbors(1), &[0]);
    assert_eq!(csr2.edge_count(), 1);
    assert_eq!(csr2.half_edge_count(), 2);
}

#[test]
fn test_csr_graph_large_scale_n10000_stress() {
    let n = 10000;
    let mut topo = Topology::new(n);

    // Add 20,000 edges in a ring topology with cross-chords
    for i in 0..n {
        topo.add_edge(i, (i + 1) % n);
        topo.add_edge(i, (i + 50) % n);
    }

    // Add 500 sinks
    for i in (0..n).step_by(20) {
        topo.add_sink(i);
    }

    let sink_bits = topo.to_sink_bitset();
    assert_eq!(sink_bits.count_ones(), 500);

    let csr = CsrGraph::from_topology(&topo, &sink_bits);
    assert_eq!(csr.num_nodes, n);

    // Sinks must have degree 0.0 and empty neighbor slices
    for i in (0..n).step_by(20) {
        assert_eq!(csr.degree(i), 0.0);
        assert_eq!(csr.neighbors(i), &[]);
    }

    // Nodes not connected to any sink must have degree 4.0
    for i in 0..n {
        if i % 20 != 0
            && (i + 1) % 20 != 0
            && (i + n - 1) % 20 != 0
            && (i + 50) % 20 != 0
            && (i + n - 50) % 20 != 0
        {
            assert_eq!(csr.degree(i), 4.0, "Node {} should have degree 4.0", i);
            assert_eq!(csr.neighbors(i).len(), 4);
        }
    }
}

#[test]
fn test_csr_graph_large_scale_n5000_disconnected() {
    let n = 5000;
    let topo = Topology::new(n);
    let sinks = BitSet::new(n);
    let csr = CsrGraph::from_topology(&topo, &sinks);

    assert_eq!(csr.num_nodes, n);
    assert_eq!(csr.row_ptrs.len(), n + 1);
    assert_eq!(csr.col_indices.len(), 0);
    assert_eq!(csr.degrees.len(), n);
    assert_eq!(csr.max_degree(), 0.0);
    assert_eq!(csr.edge_count(), 0);
    assert_eq!(csr.half_edge_count(), 0);

    for i in 0..n {
        assert_eq!(csr.degree(i), 0.0);
        assert_eq!(csr.neighbors(i), &[]);
    }
}

#[test]
fn test_csr_graph_all_sinks_scenario() {
    let n = 500;
    let mut topo = Topology::new(n);
    // Fully connected clique
    for i in 0..n {
        for j in (i + 1)..n {
            topo.add_edge(i, j);
        }
    }
    // Mark ALL nodes as sinks
    for i in 0..n {
        topo.add_sink(i);
    }

    let sink_bits = topo.to_sink_bitset();
    let csr = CsrGraph::from_topology(&topo, &sink_bits);

    assert_eq!(csr.num_nodes, n);
    assert_eq!(csr.edge_count(), 0);
    assert_eq!(csr.half_edge_count(), 0);
    assert_eq!(csr.max_degree(), 0.0);
    for i in 0..n {
        assert_eq!(csr.degree(i), 0.0);
        assert_eq!(csr.neighbors(i), &[]);
    }
}

#[test]
fn test_csr_graph_large_scale_n5000_components_and_sinks() {
    let n = 5000;
    let mut topo = Topology::new(n);

    // Create 100 disconnected cliques of size 10 each in nodes 0..1000
    for c in 0..100 {
        let base = c * 10;
        for i in 0..10 {
            for j in (i + 1)..10 {
                topo.add_edge(base + i, base + j);
            }
        }
    }

    // Nodes 1000..2000: Line chain graph
    for i in 1000..1999 {
        topo.add_edge(i, i + 1);
    }

    // Nodes 2000..3000: Star graphs centered at 2000, 2100, 2200...
    for s in 0..10 {
        let center = 2000 + s * 100;
        for leaf in (center + 1)..(center + 100) {
            topo.add_edge(center, leaf);
        }
    }

    // Nodes 3000..4000: Sink nodes and edges pointing to sinks
    for i in 3000..4000 {
        topo.add_sink(i);
        topo.add_edge(i, i.wrapping_sub(1)); // edge connected to sink
        topo.add_edge(i, 4001); // edge connected to sink
    }

    // Nodes 4000..5000: Isolated nodes + self-loops
    for i in 4000..5000 {
        topo.add_edge(i, i); // self-loop
    }

    let sink_bits = topo.to_sink_bitset();
    assert_eq!(sink_bits.count_ones(), 1000);

    let csr = CsrGraph::from_topology(&topo, &sink_bits);

    assert_eq!(csr.num_nodes, n);

    // Verify cliques
    for c in 0..100 {
        let base = c * 10;
        for i in 0..10 {
            let u = base + i;
            assert_eq!(csr.degree(u), 9.0, "Clique node {} degree mismatch", u);
            assert_eq!(csr.neighbors(u).len(), 9);
        }
    }

    // Verify line chain
    assert_eq!(csr.degree(1000), 1.0);
    assert_eq!(csr.degree(1999), 1.0);
    for i in 1001..1999 {
        assert_eq!(csr.degree(i), 2.0);
    }

    // Verify star centers
    for s in 0..10 {
        let center = 2000 + s * 100;
        assert_eq!(csr.degree(center), 99.0);
        assert_eq!(csr.neighbors(center).len(), 99);
    }

    // Verify sinks and sink-connected nodes
    for i in 3000..4000 {
        assert_eq!(csr.degree(i), 0.0);
        assert_eq!(csr.neighbors(i), &[]);
    }
    // Node 4001 should have degree 0 because its only edge was connected to sink
    assert_eq!(csr.degree(4001), 0.0);

    // Verify self-loops in 4000..5000
    for i in 4000..5000 {
        assert_eq!(csr.degree(i), 0.0);
        assert_eq!(csr.neighbors(i), &[]);
    }
}

#[test]
fn test_csr_graph_dense_clique_k300() {
    let n = 300;
    let mut topo = Topology::new(n);
    for i in 0..n {
        for j in (i + 1)..n {
            topo.add_edge(i, j);
        }
    }

    let sinks = BitSet::new(n);
    let csr = CsrGraph::from_topology(&topo, &sinks);

    let expected_edges = n * (n - 1) / 2;
    assert_eq!(csr.edge_count(), expected_edges);
    assert_eq!(csr.half_edge_count(), expected_edges * 2);
    assert_eq!(csr.max_degree(), (n - 1) as f64);

    for i in 0..n {
        assert_eq!(csr.degree(i), (n - 1) as f64);
        assert_eq!(csr.neighbors(i).len(), n - 1);
        assert!(
            !csr.neighbors(i).contains(&i),
            "Self-loop found in neighbors"
        );
    }
}

#[test]
fn test_compile_into_exact_parity_with_from_topology_fuzz() {
    // Pseudo-random LCG generator for deterministic fuzzing without external rand crate
    let mut rng_state = 0x123456789abcdef0u64;
    let mut lcg_rand = || -> u64 {
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        rng_state
    };

    let mut workspace_row_ptrs = Vec::new();
    let mut workspace_col_indices = Vec::new();
    let mut workspace_degrees = Vec::new();
    let mut workspace_cursor = Vec::new();

    // Run 1000 fuzz iterations with diverse graph shapes
    for iter in 0..1000 {
        let n = (lcg_rand() % 250) as usize; // 0 to 249 nodes
        let mut topo = Topology::new(n);

        if n > 0 {
            let num_sinks = (lcg_rand() % (n as u64 + 1)) as usize;
            for _ in 0..num_sinks {
                let sink_idx = (lcg_rand() % (n as u64)) as usize;
                topo.add_sink(sink_idx);
            }

            let num_edges = (lcg_rand() % ((n * 4 + 10) as u64)) as usize;
            for _ in 0..num_edges {
                let u = (lcg_rand() % ((n + 15) as u64)) as usize; // allow OOB
                let v = (lcg_rand() % ((n + 15) as u64)) as usize; // allow OOB
                topo.edges.push((u, v));
            }
        }

        let sink_bits = topo.to_sink_bitset();

        // 1. Build via from_topology
        let csr_from_topo = CsrGraph::from_topology(&topo, &sink_bits);

        // 2. Build via compile_into (reusing workspace filled with previous iteration data)
        CsrGraph::compile_into(
            &topo,
            &sink_bits,
            &mut workspace_row_ptrs,
            &mut workspace_col_indices,
            &mut workspace_degrees,
            &mut workspace_cursor,
        );

        let csr_compiled = CsrGraph {
            num_nodes: n,
            row_ptrs: workspace_row_ptrs.clone(),
            col_indices: workspace_col_indices.clone(),
            degrees: workspace_degrees.clone(),
        };

        // Assert exact structural parity
        assert_eq!(
            csr_from_topo.num_nodes, csr_compiled.num_nodes,
            "Iteration {}: num_nodes mismatch",
            iter
        );
        assert_eq!(
            csr_from_topo.row_ptrs, csr_compiled.row_ptrs,
            "Iteration {}: row_ptrs mismatch",
            iter
        );
        assert_eq!(
            csr_from_topo.col_indices, csr_compiled.col_indices,
            "Iteration {}: col_indices mismatch",
            iter
        );
        assert_eq!(
            csr_from_topo.degrees, csr_compiled.degrees,
            "Iteration {}: degrees mismatch",
            iter
        );
        assert_eq!(
            csr_from_topo.max_degree(),
            csr_compiled.max_degree(),
            "Iteration {}: max_degree mismatch",
            iter
        );
        assert_eq!(
            csr_from_topo.edge_count(),
            csr_compiled.edge_count(),
            "Iteration {}: edge_count mismatch",
            iter
        );
        assert_eq!(
            csr_from_topo.half_edge_count(),
            csr_compiled.half_edge_count(),
            "Iteration {}: half_edge_count mismatch",
            iter
        );

        // Check query methods across all nodes and OOB
        for u in 0..(n + 25) {
            assert_eq!(
                csr_from_topo.degree(u),
                csr_compiled.degree(u),
                "Iteration {}, node {}: degree query mismatch",
                iter,
                u
            );
            assert_eq!(
                csr_from_topo.neighbors(u),
                csr_compiled.neighbors(u),
                "Iteration {}, node {}: neighbors query mismatch",
                iter,
                u
            );
        }
    }
}

#[test]
fn test_property_1_undirected_edge_symmetry_randomized_fuzz() {
    let mut rng = 0x9876543210fedcbau64;
    let mut rand = || -> u64 {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        rng
    };

    // Fuzz 1000 topologies across various sizes, edge densities, sink ratios, and multi-edge configurations
    for iter in 0..1000 {
        let n = (rand() % 150) as usize; // 0 to 149
        let mut topo = Topology::new(n);

        if n > 0 {
            let sink_count = (rand() % (n as u64 + 1)) as usize;
            for _ in 0..sink_count {
                let s = (rand() % (n as u64)) as usize;
                topo.add_sink(s);
            }

            let edge_count = (rand() % ((n * 6 + 20) as u64)) as usize;
            for _ in 0..edge_count {
                let u = (rand() % ((n + 5) as u64)) as usize;
                let v = (rand() % ((n + 5) as u64)) as usize;
                topo.edges.push((u, v));
            }
        }

        let sink_bits = topo.to_sink_bitset();
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        // Property 1: Undirected edge symmetry
        // For all u, for all v in neighbors(u), u must be in neighbors(v) with exact matching multiplicity
        for u in 0..n {
            let u_neighbors = csr.neighbors(u);
            for &v in u_neighbors {
                assert!(v < n, "Neighbor {} out of bounds for node {}", v, u);
                let v_neighbors = csr.neighbors(v);

                // Count multiplicity of v in neighbors(u) vs u in neighbors(v)
                let count_v_in_u = u_neighbors.iter().filter(|&&x| x == v).count();
                let count_u_in_v = v_neighbors.iter().filter(|&&x| x == u).count();
                assert_eq!(
                    count_v_in_u, count_u_in_v,
                    "Edge symmetry multiplicity mismatch between node {} and node {} in iter {}",
                    u, v, iter
                );
            }
        }
    }
}

#[test]
fn test_property_2_degree_conservation_randomized_fuzz() {
    let mut rng = 0xabcdef0123456789u64;
    let mut rand = || -> u64 {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        rng
    };

    for iter in 0..1000 {
        let n = (rand() % 200) as usize;
        let mut topo = Topology::new(n);

        if n > 0 {
            let sink_count = (rand() % (n as u64 + 1)) as usize;
            for _ in 0..sink_count {
                let s = (rand() % (n as u64)) as usize;
                topo.add_sink(s);
            }

            let edge_count = (rand() % ((n * 5 + 15) as u64)) as usize;
            for _ in 0..edge_count {
                let u = (rand() % ((n + 10) as u64)) as usize;
                let v = (rand() % ((n + 10) as u64)) as usize;
                topo.edges.push((u, v));
            }
        }

        let sink_bits = topo.to_sink_bitset();
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        // Property 2: Degree conservation
        // Sum of all degrees == 2 * edge_count == half_edge_count == col_indices.len()
        let sum_degrees: f64 = (0..n).map(|u| csr.degree(u)).sum();
        let expected_half_edges = csr.half_edge_count();
        let expected_edges = csr.edge_count();

        assert_eq!(
            sum_degrees as usize, expected_half_edges,
            "Degree sum {} != half edge count {} in iter {}",
            sum_degrees, expected_half_edges, iter
        );
        assert_eq!(
            expected_half_edges,
            expected_edges * 2,
            "Half edges {} != 2 * edges {} in iter {}",
            expected_half_edges,
            expected_edges,
            iter
        );
        assert_eq!(
            csr.col_indices.len(),
            expected_half_edges,
            "col_indices length mismatch in iter {}",
            iter
        );

        // Individual node degree == neighbor count
        for u in 0..n {
            assert_eq!(
                csr.degree(u) as usize,
                csr.neighbors(u).len(),
                "Node {} degree mismatch with neighbors slice length in iter {}",
                u,
                iter
            );
        }
    }
}

#[test]
fn test_property_3_sink_isolation_randomized_fuzz() {
    let mut rng = 0x55aa55aa33cc33ccu64;
    let mut rand = || -> u64 {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        rng
    };

    for iter in 0..1000 {
        let n = (rand() % 180) as usize;
        let mut topo = Topology::new(n);

        let mut sinks_vec = Vec::new();
        if n > 0 {
            let sink_count = (rand() % (n as u64 + 1)) as usize;
            for _ in 0..sink_count {
                let s = (rand() % (n as u64)) as usize;
                topo.add_sink(s);
                sinks_vec.push(s);
            }

            // Generate edges, explicitly including edges to/from sinks
            let edge_count = (rand() % ((n * 8 + 30) as u64)) as usize;
            for _ in 0..edge_count {
                let u = (rand() % (n as u64)) as usize;
                let v = (rand() % (n as u64)) as usize;
                topo.edges.push((u, v));
            }
        }

        let sink_bits = topo.to_sink_bitset();
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        // Property 3: Sink isolation
        // 1. Sinks have degree 0.0
        // 2. Sinks have empty neighbors slice
        // 3. No sink node appears in any neighbor list anywhere in the entire graph
        for &s in &sinks_vec {
            assert_eq!(
                csr.degree(s),
                0.0,
                "Sink node {} has non-zero degree {} in iter {}",
                s,
                csr.degree(s),
                iter
            );
            assert_eq!(
                csr.neighbors(s),
                &[],
                "Sink node {} has non-empty neighbors in iter {}",
                s,
                iter
            );
        }

        for u in 0..n {
            for &neighbor in csr.neighbors(u) {
                assert!(
                    !sink_bits.contains(neighbor),
                    "Sink node {} appeared in neighbor list of node {} in iter {}",
                    neighbor,
                    u,
                    iter
                );
            }
        }
    }
}

#[test]
fn test_bitset_oracle_differential_vs_btreeset() {
    let mut rng = 0xdeadbeefcafebabeu64;
    let mut rand = || -> u64 {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        rng
    };

    for _test in 0..200 {
        let len = (rand() % 1000) as usize; // 0 to 999
        let mut bs = BitSet::new(len);
        let mut btree = std::collections::BTreeSet::new();

        // Perform 500 random operations per test
        for _op in 0..500 {
            let op_type = rand() % 5;
            let idx = (rand() % ((len + 50) as u64 + 1)) as usize;

            match op_type {
                0 | 1 => {
                    // Insert
                    bs.insert(idx);
                    if idx < len {
                        btree.insert(idx);
                    }
                }
                2 => {
                    // Remove
                    bs.remove(idx);
                    btree.remove(&idx);
                }
                3 => {
                    // Contains
                    let bs_contains = bs.contains(idx);
                    let btree_contains = if idx < len {
                        btree.contains(&idx)
                    } else {
                        false
                    };
                    assert_eq!(bs_contains, btree_contains);
                }
                4 => {
                    // Clear
                    if rand() % 20 == 0 {
                        bs.clear();
                        btree.clear();
                    }
                }
                _ => unreachable!(),
            }

            assert_eq!(bs.count_ones(), btree.len());
        }

        // Full iteration check
        let bs_ones: Vec<usize> = bs.iter_ones().collect();
        let btree_ones: Vec<usize> = btree.iter().copied().collect();
        assert_eq!(bs_ones, btree_ones);
    }
}

#[test]
fn test_high_volume_streaming_workspace_compilation_stress() {
    let mut row_ptrs = Vec::new();
    let mut col_indices = Vec::new();
    let mut degrees = Vec::new();
    let mut cursor = Vec::new();

    let mut rng = 0x1122334455667788u64;
    let mut rand = || -> u64 {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        rng
    };

    // 10,000 rapid graph compilations simulating streaming zero-allocation workloads
    for _step in 0..10_000 {
        let n = (rand() % 50) as usize + 1;
        let mut topo = Topology::new(n);
        let num_edges = (rand() % 100) as usize;
        for _ in 0..num_edges {
            let u = (rand() % (n as u64)) as usize;
            let v = (rand() % (n as u64)) as usize;
            topo.add_edge(u, v);
        }
        let num_sinks = (rand() % (n as u64)) as usize;
        for _ in 0..num_sinks {
            let s = (rand() % (n as u64)) as usize;
            topo.add_sink(s);
        }

        let sink_bits = topo.to_sink_bitset();
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

        // Assert basic invariants on each iteration
        assert_eq!(csr.num_nodes, n);
        let sum_deg: f64 = degrees.iter().sum();
        assert_eq!(sum_deg as usize, col_indices.len());
        assert_eq!(col_indices.len() % 2, 0);
    }
}
