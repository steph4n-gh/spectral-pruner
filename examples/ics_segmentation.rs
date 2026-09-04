// examples/ics_segmentation.rs
use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};

fn main() -> Result<(), spectral_pruner::PrunerError> {
    println!("Synthetic scenario: outputs are recommendations; no external system is changed.");
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
    println!("Mainland Ring Nodes : {:?}", resolution.mainland_nodes);
    println!("Candidate Island Nodes : {:?}", resolution.island_nodes);
    println!("--------------------------------------------------");

    // 5. Illustrate a caller response; no device configuration is changed.
    match resolution.action {
        PolicyAction::FatalBlock => {
            println!("[SIMULATED POLICY TRIGGER] Candidate network segment identified.");
            println!(
                "Candidate device indices {:?} have links \
                 to the DMZ boundary space. No safety interlock is controlled by this example.",
                resolution.island_nodes
            );
            std::process::exit(1);
        }
        PolicyAction::GarbageCollect => {
            println!(
                "[ADVISORY] ⚠️ Unconfigured legacy segment found at index {:?}. \
                 No switch port is changed by this example.",
                resolution.island_nodes
            );
        }
        PolicyAction::Allow => {
            println!("[ALLOW] No containment action selected under this topology policy.");
        }
    }

    Ok(())
}
