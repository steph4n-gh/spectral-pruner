// examples/ics_segmentation.rs
use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};

fn main() -> Result<(), spectral_pruner::PrunerError> {
    println!("=== [τ-Gate] Industrial Control Systems (ICS) OT Segmentation Audit ===");

    // 1. Configure the Pruner for strict OT network segregation boundaries
    let pruner = TauSpectralPruner::builder()
        .tau(0.0) // Fiedler zero-crossing partition
        .threat_threshold(1.5) // Segment density isolation threshold
        .system_start_idx(4) // Nodes 4 and 5 represent protected DMZ/WAN boundary space
        .build();

    // 2. Build the Physical Factory Network Topology Graph
    // 6 Nodes total:
    // - Nodes [0, 1, 2]: Core Operational Technology (OT) Safety Control Ring
    // - Node 3: Rogue Maintenance Laptop (unauthorized physical tap)
    // - Nodes [4, 5]: Secure DMZ Data Diode & Corporate WAN Gateway
    let mut topology = Topology::new(6);

    // --- OT Core Ring (Mainland) ---
    topology.add_edge(0, 1); // SCADA Server HMI <-> PLC Master
    topology.add_edge(1, 2); // PLC Master <-> Safety Interlock Controller
    topology.add_edge(2, 0); // Safety Interlock Controller <-> SCADA Server HMI

    // --- The DMZ Boundary Space (WAN Sink) ---
    topology.add_sink(5); // WAN Gateway is a strict data exit terminal

    // --- Anomalous Breach Connection ---
    // A rogue contractor laptop (Node 3) bridges directly to the secure DMZ Data Diode (Node 4)
    topology.add_edge(3, 4);

    // 3. Execute the Topological Spectral Audit
    let system_boundary_len = 5;
    let resolution = pruner.prune(&topology, system_boundary_len)?;

    // 4. Print Results
    println!("\n[Audit Results]");
    println!("--------------------------------------------------");
    println!("Containment Action Verdict: {}", resolution.action);
    println!(
        "Secured Safety Ring Nodes : {:?}",
        resolution.mainland_nodes
    );
    println!("Quarantined Intrusion Set : {:?}", resolution.island_nodes);
    println!("--------------------------------------------------");

    // 5. Enforce Safety Lockout Policy
    match resolution.action {
        PolicyAction::FatalBlock => {
            println!("[SAFETY WARNING] 🚫 CRITICAL SEGMENTATION BREACH DETECTED!");
            println!(
                "Rogue device at index {:?} is attempting direct unsegmented access \
                 to the DMZ boundary space. Tripping safety network interlocks.",
                resolution.island_nodes
            );
            std::process::exit(1);
        }
        PolicyAction::GarbageCollect => {
            println!(
                "[ADVISORY] ⚠️ Unconfigured legacy segment found at index {:?}. \
                 Disabling port on active switch configuration.",
                resolution.island_nodes
            );
        }
        PolicyAction::Allow => {
            println!("[NOMINAL] ✅ Network segmentation alignment verified. All zones secure.");
        }
    }

    Ok(())
}
