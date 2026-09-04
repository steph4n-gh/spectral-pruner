// examples/service_mesh_audit.rs
use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};

fn main() -> Result<(), spectral_pruner::PrunerError> {
    println!("Synthetic scenario: outputs are recommendations; no external system is changed.");
    println!("=== [τ-Gate] Kubernetes Service Mesh & Microservice Segregation Audit ===");

    // 1. Configure the pruner for zero-trust microservice call auditing.
    // Core system APIs start at node 4.
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(1.5) // Density ratio for service-call isolation
        .system_start_idx(4) // Nodes 4 & 5 represent protected control-plane/DNS routers
        .build();

    // 2. Define the Active Service Mesh Call Graph
    // 6 Services total:
    // - Nodes [0, 1, 2]: Legitimate Production Mainland (Frontend -> Auth -> Database -> Frontend)
    // - Node 3: Compromised Sidecar container (attempting unauthorized egress bypass)
    // - Nodes [4, 5]: System Control Plane / DNS Core Router
    let mut topology = Topology::new(6);

    // --- Legitimate Mesh Traffic (Mainland) ---
    topology.add_edge(0, 1); // frontend calls auth-service
    topology.add_edge(1, 2); // auth-service calls transaction-db
    topology.add_edge(2, 0); // transaction-db returns payload to frontend

    // --- Protected Control-Plane Gateway ---
    topology.add_sink(5); // WAN/DNS Core Router is a strict external terminal

    // --- Compromised Container Bypass Attempt ---
    // A compromised logger sidecar (Node 3) attempts to establish direct, out-of-mesh TCP connections
    // to the System Control Plane API (Node 4) to exfiltrate cached database secrets.
    topology.add_edge(3, 4);

    // 3. Audit Mesh Graph Topology
    let system_boundary_len = 5;
    let resolution = pruner.prune(&topology, system_boundary_len)?;

    // 4. Print Containment Decision
    println!("\n[Audit Results]");
    println!("--------------------------------------------------");
    println!("Service Mesh Action Verdict: {}", resolution.action);
    println!(
        "Mainland Service Cluster    : {:?}",
        resolution.mainland_nodes
    );
    println!("Candidate Container Set  : {:?}", resolution.island_nodes);
    println!("--------------------------------------------------");

    // 5. Illustrate a caller response; no network policy is changed.
    match resolution.action {
        PolicyAction::FatalBlock => {
            println!("[SIMULATED POLICY TRIGGER] Candidate service island identified.");
            println!(
                "Candidate container indices {:?} triggered the configured topology policy.\n\
                 A service-mesh integration could review this recommendation before containment.",
                resolution.island_nodes
            );
            std::process::exit(1);
        }
        PolicyAction::GarbageCollect => {
            println!(
                "[ADVISORY] ⚠️ Orphanded microservice or dead route detected at index {:?}. \
                 No service is deregistered by this example.",
                resolution.island_nodes
            );
        }
        PolicyAction::Allow => {
            println!("[ALLOW] No containment action selected under this topology policy.");
        }
    }

    Ok(())
}
