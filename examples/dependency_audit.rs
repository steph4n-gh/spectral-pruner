//! ⚡ [τ-Gate] Supply Chain Dependency Topology Auditor Example
//!
//! Reads local package names for context, then builds a separate synthetic graph.
//! It does not reconstruct actual dependency edges or audit the real lockfile.

use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};
use std::collections::{HashMap, HashSet};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Synthetic scenario: outputs are recommendations; no external system is changed.");
    println!("==========================================================================");
    println!("     ⚡ [τ-Gate] SOFTWARE SUPPLY CHAIN DEPENDENCY TOPOLOGY AUDITOR ⚡     ");
    println!("==========================================================================");
    println!("Demonstrating named nodes and policy output on a synthetic dependency graph.\n");

    // 1. Read local package names for display only; these do not define the audit graph.
    let lock_path = "Cargo.lock";
    println!("[+] Reading active lockfile from: {}", lock_path);
    let lock_content = fs::read_to_string(lock_path)?;

    // Simple line-by-line parser state machine
    let mut local_packages = Vec::new();
    for line in lock_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name = \"") {
            if let Some(name) = trimmed
                .strip_prefix("name = \"")
                .and_then(|s| s.strip_suffix("\""))
            {
                local_packages.push(name.to_string());
            }
        }
    }
    println!(
        "[+] Found local packages under tracking: {:?}",
        local_packages
    );

    // 2. Synthesize a realistic production workspace (since our core library is strictly zero-dependency)
    // We map a multi-crate systems workspace:
    // Mainland: axum -> tower -> tokio -> serde -> app-core
    // Island:   untrusted-leftpad-utility
    // System:   libc, compiler-linker
    println!("\n[+] Constructing the synthetic workspace dependency graph...");

    let mut registry = DependencyRegistry::new();

    // Add stable production mainland cluster
    registry.add_dependency("app-core", "axum");
    registry.add_dependency("app-core", "tower");
    registry.add_dependency("axum", "tower");
    registry.add_dependency("axum", "tokio");
    registry.add_dependency("tower", "tokio");
    registry.add_dependency("tokio", "serde");
    registry.add_dependency("serde", "serde_derive");

    // Add isolated supply-chain anomaly (malicious dependency bypass attempt)
    // Node "untrusted-leftpad" is imported, but it bypasses core app layout and builds a direct bridge
    // to secure system system linker boundaries ("compiler-linker").
    registry.add_dependency("untrusted-leftpad", "compiler-linker");

    // Declare system boundary sinks (unsafe low-level operations or build linker processes)
    registry.add_system_sink("compiler-linker");
    registry.add_system_sink("libc");

    // 3. Compile registry maps into a discrete numerical Topology
    let (topology, package_names, system_start_idx) = registry.compile();

    println!("[+] Dependency graph successfully compiled:");
    println!(
        "    -> Active unique dependency packages: {} nodes",
        package_names.len()
    );
    println!(
        "    -> System boundaries start at index : {}",
        system_start_idx
    );

    // Print dependency relationships
    println!("\n                    --- WORKSPACE DEPENDENCY LINKAGE GRAPH ---");
    for &(u, v) in &topology.edges {
        println!("      {:>22}  ───►  {}", package_names[u], package_names[v]);
    }

    // 4. Configure the Tau-Spectral Pruner
    let pruner = TauSpectralPruner::builder()
        .tau(0.0) // Numerical bisection partition boundary
        .threat_threshold(1.5) // Sensitivity density ratio
        .system_start_idx(system_start_idx)
        .build();

    // 5. Audit the dependency graph
    println!("\n[>] Initiating Power Iteration on Laplace-Beltrami Dependency Graph...");
    let resolution = pruner.prune(&topology, package_names.len())?;

    // 6. Output professional audit report
    println!("\n==========================================================================");
    println!("                  🚨 SUPPLY CHAIN SECURITY AUDIT REPORT 🚨                 ");
    println!("==========================================================================");
    println!("Audit Action Verdict       : {}", resolution.action);
    println!(
        "Algebraic Connectivity (λ₂): {:.8}",
        resolution.connectivity_score
    );
    println!("--------------------------------------------------------------------------");

    println!(
        "Mainland Packages ({} synthetic crates):",
        resolution.mainland_nodes.len()
    );
    for &id in &resolution.mainland_nodes {
        println!("  ✓ [{:02}] {}", id, package_names[id]);
    }
    println!("--------------------------------------------------------------------------");

    println!(
        "Quarantined Malicious/Anomalous Packages ({} crates isolated):",
        resolution.island_nodes.len()
    );
    for &id in &resolution.island_nodes {
        println!("  ⚠️ [{:02}] {}", id, package_names[id]);
    }
    println!("==========================================================================");

    if resolution.action == PolicyAction::FatalBlock {
        println!("\n[FATAL] 🚫 SUPPLY CHAIN COMPROMISE QUARANTINE ENFORCED!");
        println!("The Fiedler vector successfully isolated the decoupled dependency cluster:");
        for &id in &resolution.island_nodes {
            println!("  -> Quarantined: {}", package_names[id]);
        }
        println!("This cluster builds unauthorized direct bridges to system boundary sinks.");
        println!("Ending this simulation with exit 1; no build script was run or modified.");
        std::process::exit(1);
    }

    Ok(())
}

struct DependencyRegistry {
    dependencies: HashMap<String, HashSet<String>>,
    system_sinks: HashSet<String>,
}

impl DependencyRegistry {
    fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            system_sinks: HashSet::new(),
        }
    }

    fn add_dependency(&mut self, package: &str, dependency: &str) {
        self.dependencies
            .entry(package.to_string())
            .or_default()
            .insert(dependency.to_string());
        // Ensure dependency is also a registered node
        self.dependencies.entry(dependency.to_string()).or_default();
    }

    fn add_system_sink(&mut self, package: &str) {
        self.system_sinks.insert(package.to_string());
        self.dependencies.entry(package.to_string()).or_default();
    }

    /// Compiles registry into unique numerical indices, placing system nodes at the end
    fn compile(&self) -> (Topology, Vec<String>, usize) {
        let mut packages: Vec<String> = self.dependencies.keys().cloned().collect();
        // Separate system nodes to put them at the end of the indices list
        packages.sort_by(|a, b| {
            let a_sys = self.system_sinks.contains(a);
            let b_sys = self.system_sinks.contains(b);
            a_sys.cmp(&b_sys)
        });

        let mut name_to_idx = HashMap::new();
        for (idx, name) in packages.iter().enumerate() {
            name_to_idx.insert(name.clone(), idx);
        }

        let system_start_idx = packages
            .iter()
            .position(|name| self.system_sinks.contains(name))
            .unwrap_or(packages.len());

        let mut topology = Topology::new(packages.len());

        // Add sinks
        for name in &self.system_sinks {
            if let Some(&idx) = name_to_idx.get(name) {
                topology.add_sink(idx);
            }
        }

        // Add edges
        for (pkg, deps) in &self.dependencies {
            if let Some(&u) = name_to_idx.get(pkg) {
                for dep in deps {
                    if let Some(&v) = name_to_idx.get(dep) {
                        topology.add_edge(u, v);
                    }
                }
            }
        }

        (topology, packages, system_start_idx)
    }
}
