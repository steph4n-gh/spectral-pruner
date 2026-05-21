// examples/supply_chain.rs
use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology}; // Imports directly from the crate namespace

fn main() -> Result<(), spectral_pruner::PrunerError> {
    println!("=== [\u{03C4}-Gate] Dependency Topology Security Audit ===");

    // 1. Initialize the Pruner with rigid Zero-Trust constraints
    let pruner = TauSpectralPruner::builder()
        .tau(0.0) // Fixed mathematical center-of-mass threshold split
        .threat_threshold(1.5) // Strict internal-to-external density ratio
        .momentum_beta(0.5) // Heavy-ball acceleration past local graph noise
        .system_start_idx(4) // System boundaries start at our anchor infrastructure
        .build();

    // 2. Map the Software Dependency Graph Matrix
    let mut topology = Topology::new(6);

    // --- TOPOLOGY SECTION A: The Production Mainland ---
    topology.add_edge(0, 1); // app-core pulls in axum-router
    topology.add_edge(1, 2); // axum-router utilizes tower-middleware
    topology.add_edge(2, 0); // tower-middleware returns context to app-core

    // --- TOPOLOGY SECTION B: The Anomalous "Island" Exploit ---
    topology.add_edge(3, 5); // leftpad-utils attempts direct outbound execution to native-libc-linker

    // Mark Node 5 as a definitive topological terminal sink (The Boundary Wall)
    topology.add_sink(5);

    // 3. Execute the Spectral Bisection & Scale-Invariant Threat Audit
    let system_boundary_len = 5;
    let resolution = pruner.prune(&topology, system_boundary_len)?;

    // 4. Evaluate Container Resolution Payload
    println!("\n[Audit Results]");
    println!("--------------------------------------------------");
    println!("Security Action Verdict  : {}", resolution.action);
    println!(
        "Algebraic Conn Score (\u{03BB}\u{2082}): {:.6}",
        resolution.connectivity_score
    );
    println!("Secured Mainland Nodes  : {:?}", resolution.mainland_nodes);
    println!("Quarantined Anomaly Set  : {:?}", resolution.island_nodes);
    println!("--------------------------------------------------");

    // 5. Enforce Infrastructure Containment Contract
    match resolution.action {
        PolicyAction::FatalBlock => {
            eprintln!(
                "[CRITICAL] \u{1F6AB} ALERT: Topologically isolated malicious node \
                detected at index {:?}. Upstream supply chain compromise suspected.",
                resolution.island_nodes
            );
            std::process::exit(1);
        }
        PolicyAction::GarbageCollect => {
            println!(
                "[ADVISORY] \u{26A0}\u{FE0F} Dead code or unlinked sub-graph found at index {:?}. \
                Purging from active compilation memory.",
                resolution.island_nodes
            );
        }
        PolicyAction::Allow => {
            println!(
                "[NOMINAL] \u{2705} Topology architecture verified. Gateway opened successfully."
            );
        }
    }

    Ok(())
}
