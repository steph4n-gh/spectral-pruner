use spectral_pruner::{PolicyAction, PrunerWorkspace, TauSpectralPruner, Topology};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct TrackingAllocator;

static TRACKING: AtomicBool = AtomicBool::new(false);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) {
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) {
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

fn measured_prune(topology: &Topology) -> (usize, PolicyAction) {
    let pruner = TauSpectralPruner::builder().build();
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    TRACKING.store(true, Ordering::Relaxed);
    let resolution = pruner.prune(topology, 0).expect("prune should succeed");
    TRACKING.store(false, Ordering::Relaxed);
    (ALLOCATED_BYTES.load(Ordering::Relaxed), resolution.action)
}

#[test]
fn ignored_edge_floods_do_not_preallocate_raw_edge_storage() {
    const EDGE_COUNT: usize = 10_000;
    const SMALL_GRAPH_ALLOCATION_CEILING: usize = 4 * 1024;

    let mut self_loops = Topology::new(3);
    for _ in 0..EDGE_COUNT {
        self_loops.add_edge(0, 0);
    }

    let mut sink_edges = Topology::new(3);
    for _ in 0..EDGE_COUNT {
        sink_edges.add_edge(0, 2);
    }
    sink_edges.add_sink(2);

    let mut out_of_bounds = Topology::new(3);
    out_of_bounds.edges.resize(EDGE_COUNT, (0, usize::MAX));

    for topology in [&self_loops, &sink_edges, &out_of_bounds] {
        let (allocated_bytes, action) = measured_prune(topology);
        assert_eq!(action, PolicyAction::Allow);
        assert!(
            allocated_bytes < SMALL_GRAPH_ALLOCATION_CEILING,
            "ignored edges caused {allocated_bytes} bytes of prune-time allocation"
        );
    }

    let mut valid_parallel_edges = Topology::new(3);
    for _ in 0..EDGE_COUNT {
        valid_parallel_edges.add_edge(0, 1);
    }
    let pruner = TauSpectralPruner::builder().build();
    let direct = pruner.prune(&valid_parallel_edges, 0).unwrap();
    let mut workspace = PrunerWorkspace::with_capacity(3, EDGE_COUNT);
    let reused = pruner
        .prune_with_workspace(&valid_parallel_edges, 0, &mut workspace)
        .unwrap();
    assert_eq!(direct, reused);
    assert!(workspace.csr_col_indices.capacity() >= EDGE_COUNT * 2);
}
