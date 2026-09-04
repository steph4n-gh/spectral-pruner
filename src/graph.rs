//! High-performance graph representation layer featuring contiguous CSR sparse graphs
//! and zero-allocation bitsets.

use std::fmt;

/// Flat `[u64]` bitmask providing O(1) constant-time membership queries,
/// zero heap allocation in reuse loops, and POPCNT/SIMD hardware acceleration.
#[derive(Clone, PartialEq, Eq)]
pub struct BitSet {
    pub words: Vec<u64>,
    pub len: usize,
}

impl BitSet {
    /// Creates a new `BitSet` with capacity for `len` elements, all initialized to false (0).
    #[inline]
    pub fn new(len: usize) -> Self {
        let num_words = if len == 0 { 0 } else { (len - 1) / 64 + 1 };
        Self {
            words: vec![0u64; num_words],
            len,
        }
    }

    /// Creates a `BitSet` pre-allocated to hold `len` bits.
    #[inline]
    pub fn with_capacity(len: usize) -> Self {
        Self::new(len)
    }

    /// Returns the logical capacity (number of trackable items) in the bitset.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the bitset has a logical length of 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Checks if bit `idx` is set. Returns `false` if `idx >= self.len`.
    #[inline]
    pub fn contains(&self, idx: usize) -> bool {
        if idx >= self.len {
            return false;
        }
        let word_idx = idx >> 6;
        let bit_offset = idx & 63;
        (self.words[word_idx] & (1u64 << bit_offset)) != 0
    }

    /// Sets bit `idx` to 1. Silently ignores if `idx >= self.len`.
    #[inline]
    pub fn insert(&mut self, idx: usize) {
        if idx < self.len {
            let word_idx = idx >> 6;
            let bit_offset = idx & 63;
            self.words[word_idx] |= 1u64 << bit_offset;
        }
    }

    /// Clears bit `idx` (sets to 0). Silently ignores if `idx >= self.len`.
    #[inline]
    pub fn remove(&mut self, idx: usize) {
        if idx < self.len {
            let word_idx = idx >> 6;
            let bit_offset = idx & 63;
            self.words[word_idx] &= !(1u64 << bit_offset);
        }
    }

    /// Resets all bits to 0 without deallocating the underlying vector buffer.
    #[inline]
    pub fn clear(&mut self) {
        self.words.fill(0);
    }

    /// Resizes the bitset to track `new_len` bits, clearing all words.
    #[inline]
    pub fn reset_with_len(&mut self, new_len: usize) {
        self.len = new_len;
        let num_words = if new_len == 0 {
            0
        } else {
            (new_len - 1) / 64 + 1
        };
        self.words.clear();
        self.words.resize(num_words, 0u64);
    }

    /// Counts total number of set bits (population count) using hardware POPCNT.
    #[inline]
    pub fn count_ones(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Constructs a `BitSet` from an iterator of indices.
    pub fn from_iter<I: IntoIterator<Item = usize>>(len: usize, iter: I) -> Self {
        let mut set = Self::new(len);
        for idx in iter {
            set.insert(idx);
        }
        set
    }

    /// Constructs a `BitSet` from a slice of indices.
    pub fn from_slice(len: usize, slice: &[usize]) -> Self {
        let mut set = Self::new(len);
        for &idx in slice {
            set.insert(idx);
        }
        set
    }

    /// Returns an iterator yielding all indices where the bit is set to 1.
    pub fn iter_ones(&self) -> BitSetIter<'_> {
        BitSetIter {
            bitset: self,
            word_idx: 0,
            current_word: if self.words.is_empty() {
                0
            } else {
                self.words[0]
            },
        }
    }
}

impl fmt::Debug for BitSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ones: Vec<usize> = self.iter_ones().collect();
        f.debug_struct("BitSet")
            .field("len", &self.len)
            .field("ones", &ones)
            .finish()
    }
}

impl<'a> IntoIterator for &'a BitSet {
    type Item = usize;
    type IntoIter = BitSetIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_ones()
    }
}

/// Iterator over set bits in a `BitSet`.
pub struct BitSetIter<'a> {
    bitset: &'a BitSet,
    word_idx: usize,
    current_word: u64,
}

impl<'a> Iterator for BitSetIter<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.word_idx < self.bitset.words.len() {
            if self.current_word != 0 {
                let bit_pos = self.current_word.trailing_zeros() as usize;
                let global_idx = (self.word_idx << 6) + bit_pos;
                // Clear the lowest set bit
                self.current_word &= self.current_word.wrapping_sub(1);
                if global_idx < self.bitset.len {
                    return Some(global_idx);
                } else {
                    return None;
                }
            }
            self.word_idx += 1;
            if self.word_idx < self.bitset.words.len() {
                self.current_word = self.bitset.words[self.word_idx];
            }
        }
        None
    }
}

/// Contiguous Compressed Sparse Row (CSR) graph matrix representation.
///
/// Features:
/// - Single flat array `col_indices` for high cache-locality.
/// - Exact two-pass linear compilation ($O(N + E)$).
/// - Zero per-node heap allocations.
/// - Built-in sink, self-loop, and out-of-bounds filtering.
#[derive(Debug, Clone, PartialEq)]
pub struct CsrGraph {
    pub num_nodes: usize,
    pub row_ptrs: Vec<usize>,
    pub col_indices: Vec<usize>,
    pub degrees: Vec<f64>,
}

/// Weighted Compressed Sparse Row graph used by the spectral engine.
///
/// This representation intentionally lives alongside [`CsrGraph`] so the
/// original unweighted public API remains source-compatible. Unweighted edges
/// are represented with a weight of `1.0` when compiled into this structure.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedCsrGraph {
    pub num_nodes: usize,
    pub row_ptrs: Vec<usize>,
    pub col_indices: Vec<usize>,
    pub weights: Vec<f64>,
    pub degrees: Vec<f64>,
}

impl WeightedCsrGraph {
    /// Compiles unweighted and weighted topology edges into reusable CSR
    /// buffers. Invalid endpoints, self-loops, sink edges, and non-positive or
    /// non-finite weights are ignored. The pruning engine validates weights
    /// before calling this function so malformed weighted inputs are reported
    /// to callers rather than silently accepted.
    #[allow(clippy::too_many_arguments)]
    pub fn compile_into(
        topo: &crate::engine::Topology,
        sink_bits: &BitSet,
        row_ptrs: &mut Vec<usize>,
        col_indices: &mut Vec<usize>,
        weights: &mut Vec<f64>,
        degrees: &mut Vec<f64>,
        cursor: &mut Vec<usize>,
    ) {
        let n = topo.num_nodes;

        row_ptrs.clear();
        row_ptrs.resize(n + 1, 0);

        degrees.clear();
        degrees.resize(n, 0.0);

        let mut count_edge = |u: usize, v: usize, weight: f64| {
            if u < n
                && v < n
                && u != v
                && !sink_bits.contains(u)
                && !sink_bits.contains(v)
                && weight.is_finite()
                && weight > 0.0
            {
                row_ptrs[u + 1] += 1;
                row_ptrs[v + 1] += 1;
                degrees[u] += weight;
                degrees[v] += weight;
            }
        };

        for &(u, v) in &topo.edges {
            count_edge(u, v, 1.0);
        }
        for &(u, v, weight) in &topo.weighted_edges {
            count_edge(u, v, weight);
        }

        for i in 0..n {
            row_ptrs[i + 1] += row_ptrs[i];
        }

        let total_half_edges = row_ptrs[n];
        col_indices.clear();
        col_indices.resize(total_half_edges, 0);
        weights.clear();
        weights.resize(total_half_edges, 0.0);

        cursor.clear();
        cursor.extend_from_slice(&row_ptrs[..n]);

        let write_edge = |u: usize,
                          v: usize,
                          weight: f64,
                          col_indices: &mut Vec<usize>,
                          weights: &mut Vec<f64>,
                          cursor: &mut Vec<usize>| {
            if u < n
                && v < n
                && u != v
                && !sink_bits.contains(u)
                && !sink_bits.contains(v)
                && weight.is_finite()
                && weight > 0.0
            {
                let pos_u = cursor[u];
                col_indices[pos_u] = v;
                weights[pos_u] = weight;
                cursor[u] += 1;

                let pos_v = cursor[v];
                col_indices[pos_v] = u;
                weights[pos_v] = weight;
                cursor[v] += 1;
            }
        };

        for &(u, v) in &topo.edges {
            write_edge(u, v, 1.0, col_indices, weights, cursor);
        }
        for &(u, v, weight) in &topo.weighted_edges {
            write_edge(u, v, weight, col_indices, weights, cursor);
        }
    }
}

impl CsrGraph {
    /// Creates an empty CSR graph with `num_nodes` and 0 edges.
    pub fn empty(num_nodes: usize) -> Self {
        Self {
            num_nodes,
            row_ptrs: vec![0; num_nodes + 1],
            col_indices: Vec::new(),
            degrees: vec![0.0; num_nodes],
        }
    }

    /// Compiles a `Topology` and `sink_bits` into a contiguous `CsrGraph` in 2 linear passes.
    ///
    /// - **Pass 1**: Counts degrees of active non-sink edges and computes prefix sums in `row_ptrs`.
    /// - **Pass 2**: Fills contiguous `col_indices` using row cursors.
    ///
    /// Edge invariants:
    /// - Self-loops ($u == v$) are skipped.
    /// - Out-of-bounds nodes ($u \ge N$ or $v \ge N$) are skipped.
    /// - Edges connected to sinks (`sink_bits.contains(u)` or `sink_bits.contains(v)`) are omitted.
    /// - Isolated nodes ($d_i = 0$) are preserved with `degrees[i] = 0.0` and empty slices.
    pub fn from_topology(topo: &crate::engine::Topology, sink_bits: &BitSet) -> Self {
        let n = topo.num_nodes;
        let mut row_ptrs = vec![0usize; n + 1];
        let mut degrees = vec![0.0f64; n];

        // Pass 1: Count active non-sink degrees
        for &(u, v) in &topo.edges {
            if u < n && v < n && u != v && !sink_bits.contains(u) && !sink_bits.contains(v) {
                row_ptrs[u + 1] += 1;
                row_ptrs[v + 1] += 1;
                degrees[u] += 1.0;
                degrees[v] += 1.0;
            }
        }

        // In-place prefix sum to establish row boundaries
        for i in 0..n {
            row_ptrs[i + 1] += row_ptrs[i];
        }

        let total_half_edges = row_ptrs[n];
        let mut col_indices = vec![0usize; total_half_edges];

        // Cursor array tracking current insertion write heads per row
        let mut cursor = row_ptrs[..n].to_vec();

        // Pass 2: Write column indices contiguously
        for &(u, v) in &topo.edges {
            if u < n && v < n && u != v && !sink_bits.contains(u) && !sink_bits.contains(v) {
                let pos_u = cursor[u];
                col_indices[pos_u] = v;
                cursor[u] += 1;

                let pos_v = cursor[v];
                col_indices[pos_v] = u;
                cursor[v] += 1;
            }
        }

        Self {
            num_nodes: n,
            row_ptrs,
            col_indices,
            degrees,
        }
    }

    /// Zero-allocation in-place compilation using pre-allocated workspace buffers.
    pub fn compile_into(
        topo: &crate::engine::Topology,
        sink_bits: &BitSet,
        row_ptrs: &mut Vec<usize>,
        col_indices: &mut Vec<usize>,
        degrees: &mut Vec<f64>,
        cursor: &mut Vec<usize>,
    ) {
        let n = topo.num_nodes;

        row_ptrs.clear();
        row_ptrs.resize(n + 1, 0);

        degrees.clear();
        degrees.resize(n, 0.0);

        // Pass 1: Degree counting
        for &(u, v) in &topo.edges {
            if u < n && v < n && u != v && !sink_bits.contains(u) && !sink_bits.contains(v) {
                row_ptrs[u + 1] += 1;
                row_ptrs[v + 1] += 1;
                degrees[u] += 1.0;
                degrees[v] += 1.0;
            }
        }

        // Prefix sum
        for i in 0..n {
            row_ptrs[i + 1] += row_ptrs[i];
        }

        let total_half_edges = row_ptrs[n];
        col_indices.clear();
        col_indices.resize(total_half_edges, 0);

        cursor.clear();
        cursor.extend_from_slice(&row_ptrs[..n]);

        // Pass 2: Fill column indices
        for &(u, v) in &topo.edges {
            if u < n && v < n && u != v && !sink_bits.contains(u) && !sink_bits.contains(v) {
                let pos_u = cursor[u];
                col_indices[pos_u] = v;
                cursor[u] += 1;

                let pos_v = cursor[v];
                col_indices[pos_v] = u;
                cursor[v] += 1;
            }
        }
    }

    /// Returns a contiguous slice of neighbor node indices for node `u`.
    #[inline]
    pub fn neighbors(&self, u: usize) -> &[usize] {
        if u >= self.num_nodes {
            &[]
        } else {
            let start = self.row_ptrs[u];
            let end = self.row_ptrs[u + 1];
            &self.col_indices[start..end]
        }
    }

    /// Returns the pre-calculated degree of node `u`.
    #[inline]
    pub fn degree(&self, u: usize) -> f64 {
        if u >= self.num_nodes {
            0.0
        } else {
            self.degrees[u]
        }
    }

    /// Returns the maximum degree across all nodes in the graph.
    #[inline]
    pub fn max_degree(&self) -> f64 {
        self.degrees.iter().copied().fold(0.0, f64::max)
    }

    /// Returns the total number of directed half-edges stored in CSR format.
    #[inline]
    pub fn half_edge_count(&self) -> usize {
        self.col_indices.len()
    }

    /// Returns the number of undirected edges in the CSR graph ($E = \text{half\_edges} / 2$).
    #[inline]
    pub fn edge_count(&self) -> usize {
        self.col_indices.len() / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Topology;

    #[test]
    fn test_bitset_basic_and_boundaries() {
        let mut bs = BitSet::new(130);
        assert_eq!(bs.len(), 130);
        assert_eq!(bs.words.len(), 3);
        assert!(!bs.contains(0));
        assert!(!bs.contains(63));
        assert!(!bs.contains(64));
        assert!(!bs.contains(129));
        assert!(!bs.contains(130)); // OOB

        bs.insert(0);
        bs.insert(63);
        bs.insert(64);
        bs.insert(129);
        bs.insert(200); // OOB insert should not panic

        assert!(bs.contains(0));
        assert!(bs.contains(63));
        assert!(bs.contains(64));
        assert!(bs.contains(129));
        assert!(!bs.contains(200));
        assert_eq!(bs.count_ones(), 4);

        let ones: Vec<usize> = bs.iter_ones().collect();
        assert_eq!(ones, vec![0, 63, 64, 129]);

        bs.remove(64);
        assert!(!bs.contains(64));
        assert_eq!(bs.count_ones(), 3);

        bs.clear();
        assert_eq!(bs.count_ones(), 0);
        assert!(!bs.contains(0));
    }

    #[test]
    fn test_bitset_empty_and_reset() {
        let mut bs = BitSet::new(0);
        assert_eq!(bs.len(), 0);
        assert!(bs.is_empty());
        assert!(!bs.contains(0));
        bs.insert(0);
        assert_eq!(bs.count_ones(), 0);
        let ones: Vec<usize> = bs.iter_ones().collect();
        assert!(ones.is_empty());

        bs.reset_with_len(65);
        assert_eq!(bs.len(), 65);
        assert_eq!(bs.words.len(), 2);
        assert!(!bs.is_empty());
        bs.insert(64);
        assert!(bs.contains(64));
        assert_eq!(bs.count_ones(), 1);
    }

    #[test]
    fn test_bitset_constructors_and_into_iter() {
        let indices = vec![1, 5, 63, 70];
        let bs = BitSet::from_slice(100, &indices);
        assert_eq!(bs.count_ones(), 4);
        assert!(bs.contains(1));
        assert!(bs.contains(5));
        assert!(bs.contains(63));
        assert!(bs.contains(70));

        let bs2 = BitSet::from_iter(100, indices.iter().copied());
        assert_eq!(bs, bs2);

        let collected: Vec<usize> = (&bs).into_iter().collect();
        assert_eq!(collected, indices);

        let debug_str = format!("{:?}", bs);
        assert!(debug_str.contains("BitSet"));
        assert!(debug_str.contains("len: 100"));
    }

    #[test]
    fn test_csr_graph_star_topology() {
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(0, 2);
        topo.add_edge(0, 3);
        topo.add_edge(0, 4);

        let sink_bits = BitSet::new(5);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        assert_eq!(csr.num_nodes, 5);
        assert_eq!(csr.degree(0), 4.0);
        assert_eq!(csr.degree(1), 1.0);
        assert_eq!(csr.max_degree(), 4.0);
        assert_eq!(csr.edge_count(), 4);
        assert_eq!(csr.half_edge_count(), 8);

        assert_eq!(csr.neighbors(0), &[1, 2, 3, 4]);
        assert_eq!(csr.neighbors(1), &[0]);
        assert_eq!(csr.neighbors(2), &[0]);
        assert_eq!(csr.neighbors(3), &[0]);
        assert_eq!(csr.neighbors(4), &[0]);
        assert_eq!(csr.neighbors(10), &[]); // OOB
        assert_eq!(csr.degree(10), 0.0); // OOB
    }

    #[test]
    fn test_csr_graph_sinks_and_self_loops() {
        let mut topo = Topology::new(5);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 2); // Self-loop, should be ignored
        topo.add_edge(3, 4); // Edge to sink node 4
        topo.add_sink(4);

        let sink_bits = topo.to_sink_bitset();
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        assert_eq!(csr.degree(0), 1.0);
        assert_eq!(csr.degree(1), 2.0);
        assert_eq!(csr.degree(2), 1.0);
        assert_eq!(csr.degree(3), 0.0); // Edge (3, 4) omitted because 4 is sink
        assert_eq!(csr.degree(4), 0.0); // Sink node has 0 degree

        assert_eq!(csr.neighbors(0), &[1]);
        assert_eq!(csr.neighbors(1), &[0, 2]);
        assert_eq!(csr.neighbors(2), &[1]);
        assert_eq!(csr.neighbors(3), &[]);
        assert_eq!(csr.neighbors(4), &[]);
    }

    #[test]
    fn test_csr_graph_disconnected_isolated_nodes() {
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        // Nodes 2 and 3 are isolated

        let sink_bits = BitSet::new(4);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        assert_eq!(csr.degree(0), 1.0);
        assert_eq!(csr.degree(1), 1.0);
        assert_eq!(csr.degree(2), 0.0);
        assert_eq!(csr.degree(3), 0.0);
        assert_eq!(csr.neighbors(2), &[]);
        assert_eq!(csr.neighbors(3), &[]);
    }

    #[test]
    fn test_csr_graph_empty_and_out_of_bounds_edges() {
        let empty_csr = CsrGraph::empty(3);
        assert_eq!(empty_csr.num_nodes, 3);
        assert_eq!(empty_csr.edge_count(), 0);
        assert_eq!(empty_csr.half_edge_count(), 0);
        assert_eq!(empty_csr.max_degree(), 0.0);
        assert_eq!(empty_csr.neighbors(0), &[]);

        let mut topo = Topology::new(3);
        topo.edges.push((0, 10)); // out of bounds
        topo.edges.push((10, 1)); // out of bounds
        topo.edges.push((0, 1)); // valid
        let sink_bits = BitSet::new(3);
        let csr = CsrGraph::from_topology(&topo, &sink_bits);
        assert_eq!(csr.edge_count(), 1);
        assert_eq!(csr.neighbors(0), &[1]);
        assert_eq!(csr.neighbors(1), &[0]);
    }

    #[test]
    fn test_csr_graph_compile_into() {
        let mut topo = Topology::new(4);
        topo.add_edge(0, 1);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);

        let sink_bits = BitSet::new(4);
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

        let csr = CsrGraph {
            num_nodes: 4,
            row_ptrs,
            col_indices,
            degrees,
        };

        assert_eq!(csr.edge_count(), 3);
        assert_eq!(csr.neighbors(0), &[1]);
        assert_eq!(csr.neighbors(1), &[0, 2]);
        assert_eq!(csr.neighbors(2), &[1, 3]);
        assert_eq!(csr.neighbors(3), &[2]);
    }

    #[test]
    fn test_csr_graph_equivalence_with_legacy_adj() {
        // Arbitrary graph with sinks, multi-degrees, isolated nodes
        let mut topo = Topology::new(6);
        topo.add_edge(0, 1);
        topo.add_edge(0, 2);
        topo.add_edge(1, 2);
        topo.add_edge(2, 3);
        topo.add_edge(3, 4);
        topo.add_edge(4, 5);
        topo.add_sink(5); // sink

        let sink_bits = topo.to_sink_bitset();
        let csr = CsrGraph::from_topology(&topo, &sink_bits);

        // Compute legacy adj
        let mut legacy_adj = vec![Vec::new(); 6];
        let mut legacy_degrees = [0.0; 6];
        for &(u, v) in &topo.edges {
            if !topo.sinks.contains(&u) && !topo.sinks.contains(&v) && u != v {
                legacy_adj[u].push(v);
                legacy_adj[v].push(u);
                legacy_degrees[u] += 1.0;
                legacy_degrees[v] += 1.0;
            }
        }

        for i in 0..6 {
            assert_eq!(
                csr.degree(i),
                legacy_degrees[i],
                "Degree mismatch for node {}",
                i
            );
            assert_eq!(
                csr.neighbors(i),
                &legacy_adj[i][..],
                "Neighbors mismatch for node {}",
                i
            );
        }
    }
}
