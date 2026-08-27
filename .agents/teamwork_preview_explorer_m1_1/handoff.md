# Milestone 1 Plan & Blueprint: CSR Graph & BitSet Data Structures

**Document Path**: `/Volumes/Storage/bigworkspace/spectral-pruner/.agents/teamwork_preview_explorer_m1_1/handoff.md`  
**Author**: Architecture & Data Structures Specialist (`teamwork_preview_explorer_m1_1`)  
**Target Repository**: `spectral-pruner` (Rust)  
**Milestone**: M1 (CSR Graph & BitSet Data Structures)  
**Date**: 2026-08-27  

---

## 1. Observation

Direct observations from codebase inspection, specification manifests, and baseline testing:

### 1.1 Existing Graph Representation Bottlenecks (`src/engine.rs:150-161`)
In the existing baseline implementation:
```rust
let mut adj = vec![Vec::new(); n];
let mut degrees = vec![0.0; n];

for &(u, v) in &topology.edges {
    if !topology.sinks.contains(&u) && !topology.sinks.contains(&v) && u != v {
        adj[u].push(v);
        adj[v].push(u);
        degrees[u] += 1.0;
        degrees[v] += 1.0;
    }
}
```
- **Heap Allocation Overhead**: Allocates $N + 1$ independent heap vectors on every call to `prune()`. For a graph with $N = 10,000$, this performs 10,001 heap allocations and reallocations.
- **Cache Eviction & Pointer Indirection**: Iterating over `adj[i]` performs an indirect pointer dereference for every node, scattering memory accesses across heap segments and stalling L1/L2 cache prefetchers.
- **Sink Lookup Inefficiency**: Sinks are stored as `BTreeSet<usize>` in `Topology`. Checking `topology.sinks.contains(&u)` executes an $O(\log S)$ tree traversal per edge endpoint, resulting in $2E \log S$ branch-heavy lookups during graph compilation and $O(\log S)$ lookups in every iteration of the eigensolver.
- **Island Node Lookup Inefficiency (`src/engine.rs:300-326`)**: Island nodes are collected into `BTreeSet<usize>`, causing additional heap allocation and tree traversal during threat metric evaluation.

### 1.2 Invariant & Zero-Assumption Requirements (`AGENTS.md`)
1. **Zero Dependencies**: Pure standard library Rust (`edition = "2021"`), zero external linear algebra or graph crates (`petgraph`, `ndarray`, etc.).
2. **Zero-Degree Regularization Clamping (Arrington Clamping)**: Nodes with $\text{degree} == 0$ must have their degree explicitly calculated as `0.0` and be preserved in the graph structure so that eigensolver initialization can clamp $v_i = 1.0$.
3. **Self-Loop & OOB Invariance**: Self-loops ($u == v$) and out-of-bounds nodes ($u \ge N$ or $v \ge N$) must be filtered out without panicking.
4. **Sink Exclusion**: Sinks must have degree `0.0` and empty neighbor lists in the CSR graph, while non-sink edges connected to sinks are pruned.
5. **Full Backward Compatibility**: The public API of `Topology`, `PrunerBuilder`, `TauSpectralPruner`, `PolicyAction`, `PrunerResolution`, and `PrunerError` must remain 100% compatible.

---

## 2. Logic Chain

### 2.1 Contiguous Compressed Sparse Row (`CsrGraph`) Mechanics
1. **Memory Layout**:
   - `row_ptrs: Vec<usize>` of length $N + 1$: `row_ptrs[i]` stores the starting offset of node $i$'s neighbors in `col_indices`, and `row_ptrs[i+1]` stores the end offset (half-open range `row_ptrs[i]..row_ptrs[i+1]`).
   - `col_indices: Vec<usize>` of length $2E_{\text{active}}$: Stores neighbor indices in a single contiguous memory block.
   - `degrees: Vec<f64>` of length $N$: Pre-computed active node degrees.
2. **Two-Pass Compilation Algorithm**:
   - **Pass 1 (Degree Accumulation & Prefix Sum)**:
     - Initialize `row_ptrs` of size $N + 1$ with zeros, and `degrees` of size $N$ with zeros.
     - Scan `topology.edges`: for each valid non-sink edge $(u, v)$ where $u \ne v$, increment `row_ptrs[u + 1] += 1`, `row_ptrs[v + 1] += 1`, `degrees[u] += 1.0`, and `degrees[v] += 1.0`.
     - In-place prefix sum: `for i in 0..N { row_ptrs[i + 1] += row_ptrs[i]; }`.
     - Total half-edges $2E = \text{row\_ptrs}[N]$.
   - **Pass 2 (Contiguous Column Index Filling)**:
     - Allocate `col_indices` of exact length $2E$ (single flat allocation).
     - Initialize a cursor array `cursor` of length $N$ where `cursor[u] = row_ptrs[u]`.
     - Scan `topology.edges` again: for each valid non-sink edge $(u, v)$ where $u \ne v$, write `col_indices[cursor[u]] = v; cursor[u] += 1;` and `col_indices[cursor[v]] = u; cursor[v] += 1;`.
   - **Complexity**: Exactly $O(N + E)$ time, $O(N + E)$ space, and strictly **two** vector allocations total (3 including `degrees`), with **zero** per-node heap allocations.
3. **Workspace In-Place Mutation**:
   - By supplying mutable references `&mut Vec<usize>` and `&mut Vec<f64>` from a `PrunerWorkspace`, buffers are cleared and resized in-place, achieving **zero heap allocations** across repeated runs.

### 2.2 Dense BitSet (`BitSet`) Mechanics
1. **Memory Layout**:
   - `words: Vec<u64>` where each word represents 64 bits.
   - `len: usize`: Total number of trackable bits ($N$).
   - Number of words: $W = \lceil N / 64 \rceil = (N + 63) / 64$.
2. **Operations**:
   - `contains(idx)`: `if idx < len { (words[idx >> 6] & (1u64 << (idx & 63))) != 0 } else { false }` ($O(1)$ single bitwise AND).
   - `insert(idx)`: `if idx < len { words[idx >> 6] |= (1u64 << (idx & 63)); }` ($O(1)$ single bitwise OR).
   - `remove(idx)`: `if idx < len { words[idx >> 6] &= !(1u64 << (idx & 63)); }` ($O(1)$ single bitwise AND-NOT).
   - `clear()`: Sets all words to 0 ($O(W)$ word-level reset).
   - `count_ones()`: `words.iter().map(|w| w.count_ones() as usize).sum()` (uses CPU POPCNT instruction).
   - `iter_ones()`: Efficiently iterates over set bit indices using trailing zeros instruction (`w.trailing_zeros()`).

---

## 3. Caveats

1. **Undirected Edge Doubling**:
   - Because `spectral-pruner` models undirected causal graphs, each undirected edge $(u, v)$ produces two directed entries in `col_indices` (one in row $u$ pointing to $v$, one in row $v$ pointing to $u$). `col_indices.len()` will equal $2 \times \text{valid\_edges}$.
2. **Sinks Representation in Topology**:
   - `Topology.sinks` is currently a `BTreeSet<usize>`. To maintain 100% backward compatibility for code accessing `topology.sinks`, `Topology` retains `sinks: BTreeSet<usize>`, while `CsrGraph::from_topology` accepts `&BitSet` (or converts `topology.sinks` to `BitSet` via a helper method `topology.to_sink_bitset()`).
3. **Order of Neighbors**:
   - The order of neighbors in `col_indices[row_ptrs[u]..row_ptrs[u+1]]` reflects the insertion order of edges in `Topology.edges`. Because Laplacian matrix-vector multiplication is commutative with respect to summation ($\sum_{j \in \text{adj}[u]} v_j$), neighbor permutation does not affect eigensolver mathematical outcomes.

---

## 4. Conclusion & Complete Implementation Blueprints

### 4.1 Module Architecture
We will introduce `src/graph.rs` and re-export `CsrGraph` and `BitSet` in `src/lib.rs` and `src/engine.rs`.

```text
src/
├── lib.rs       # Public crate re-exports and unit tests
├── engine.rs    # TauSpectralPruner, PrunerBuilder, PrunerResolution, PolicyAction
├── graph.rs     # BitSet, CsrGraph, and Topology extensions
└── error.rs     # PrunerError, Result
```

### 4.2 Blueprint 1: `src/graph.rs` (Complete Pure Rust Implementation)

```rust
//! High-performance graph representation layer featuring contiguous CSR sparse graphs
//! and zero-allocation bitsets.

use std::fmt;

/// Flat `[u64]` bitmask providing O(1) constant-time membership queries,
/// zero heap allocation in reuse loops, and SIMD/POPCNT acceleration.
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
        for word in &mut self.words {
            *word = 0;
        }
    }

    /// Resizes the bitset to track `new_len` bits, clearing all words.
    #[inline]
    pub fn reset_with_len(&mut self, new_len: usize) {
        self.len = new_len;
        let num_words = if new_len == 0 { 0 } else { (new_len - 1) / 64 + 1 };
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
            current_word: if self.words.is_empty() { 0 } else { self.words[0] },
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
                self.current_word &= self.current_word - 1;
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
/// - Built-in sink and self-loop filtering.
#[derive(Debug, Clone, PartialEq)]
pub struct CsrGraph {
    pub num_nodes: usize,
    pub row_ptrs: Vec<usize>,
    pub col_indices: Vec<usize>,
    pub degrees: Vec<f64>,
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
```

### 4.3 Blueprint 2: `Topology` Helper in `src/engine.rs`

```rust
impl Topology {
    /// Constructs a `BitSet` bitmask representation of the graph's sinks.
    #[inline]
    pub fn to_sink_bitset(&self) -> crate::graph::BitSet {
        let mut bitset = crate::graph::BitSet::new(self.num_nodes);
        for &sink in &self.sinks {
            bitset.insert(sink);
        }
        bitset
    }
}
```

### 4.4 Blueprint 3: `src/lib.rs` Public Exports

```rust
pub mod engine;
pub mod error;
pub mod graph;

// Re-export core items for library clean top-level paths
pub use engine::{PolicyAction, PrunerBuilder, PrunerResolution, TauSpectralPruner, Topology};
pub use error::{PrunerError, Result};
pub use graph::{BitSet, CsrGraph};
```

---

## 5. Verification Method

### 5.1 Step-by-Step Verification Plan for Implementer
1. **Compilation & Dependency Check**:
   ```bash
   cargo check
   cargo tree
   ```
   Verify 0 external dependencies.
2. **Unit Test Regression Check**:
   ```bash
   cargo test
   ```
   All 7 existing invariant tests in `src/lib.rs` must pass with 0 modifications.
3. **M1 Dedicated Unit Test Suite**:
   Create unit tests in `src/graph.rs` or `tests/test_m1_graph_bitset.rs` covering:
   - `BitSet` empty, boundary words (63, 64, 65, 128), out-of-bounds queries, `count_ones`, `iter_ones`.
   - `CsrGraph` 2-pass compilation on Clique, Star, Cycle, Decoupled clusters, and Empty graphs.
   - Self-loops ($u == v$) skipped.
   - Out-of-bounds edges ($u \ge N$ or $v \ge N$) skipped.
   - Sinks omitted from neighbor slices and degrees.
   - Disconnected nodes preserved with degree `0.0`.
   - Equivalence test: `csr.neighbors(u)` produces identical elements to legacy `adj[u]` for arbitrary topologies.
4. **Clippy and Formatting Check**:
   ```bash
   cargo clippy --all-targets -- -D warnings
   cargo fmt --check
   ```

### 5.2 Test Code Blueprint for M1 Verification

```rust
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
}
```
