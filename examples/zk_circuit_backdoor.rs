// examples/zk_circuit_backdoor.rs
use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};

/// Represents the classification type of a ZK circuit signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignalType {
    /// Private input witness signal (e.g., private keys, preimages)
    PrivateInput,
    /// Public verification input signal (e.g., hash commitments, public keys)
    PublicInput,
    /// Intermediate constraint signal (multiplication gate outputs)
    Intermediate,
    /// The ultimate proof validity indicator (e.g., output verification flag)
    PublicOutput,
}

/// Represents an abstract R1CS ZK-SNARK signal variable.
#[derive(Debug, Clone)]
struct ZkSignal {
    id: usize,
    name: &'static str,
    signal_type: SignalType,
}

/// Represents a quadratic R1CS constraint of the form: (Σ a_i * x_i) * (Σ b_i * x_i) = Σ c_i * x_i
/// In ZK-SNARK systems, every non-linear operation must be constrained this way.
#[derive(Debug, Clone)]
struct R1CSConstraint {
    label: &'static str,
    lc_a: Vec<usize>, // Linear combination A signal IDs
    lc_b: Vec<usize>, // Linear combination B signal IDs
    lc_c: Vec<usize>, // Linear combination C (output) signal IDs
}

/// A simulated ZK Arithmetic Circuit Compiler & Topology Generator.
struct R1CSCircuit {
    signals: Vec<ZkSignal>,
    constraints: Vec<R1CSConstraint>,
}

impl R1CSCircuit {
    fn new() -> Self {
        Self {
            signals: Vec::new(),
            constraints: Vec::new(),
        }
    }

    fn add_signal(&mut self, name: &'static str, signal_type: SignalType) -> usize {
        let id = self.signals.len();
        self.signals.push(ZkSignal {
            id,
            name,
            signal_type,
        });
        id
    }

    fn add_constraint(
        &mut self,
        label: &'static str,
        lc_a: Vec<usize>,
        lc_b: Vec<usize>,
        lc_c: Vec<usize>,
    ) {
        self.constraints.push(R1CSConstraint {
            label,
            lc_a,
            lc_b,
            lc_c,
        });
    }

    /// Compiles the abstract R1CS constraints into a mathematical Dependency Topology
    /// and extracts system boundary parameters for the Tau-Spectral Pruner.
    fn compile(&self) -> (Topology, usize, usize) {
        let num_signals = self.signals.len();
        let mut topology = Topology::new(num_signals);

        // Map algebraic constraint flows:
        // Inputs to a multiplication gate (linear combinations A & B) dynamically constrain
        // the output (linear combination C).
        for constraint in &self.constraints {
            // Add directed flow from inputs to outputs
            for &in_a in &constraint.lc_a {
                for &out_c in &constraint.lc_c {
                    topology.add_edge(in_a, out_c);
                }
            }
            for &in_b in &constraint.lc_b {
                for &out_c in &constraint.lc_c {
                    topology.add_edge(in_b, out_c);
                }
            }
            // Add internal coupling edges inside linear combinations to represent mutual signal grouping
            for i in 0..constraint.lc_a.len() {
                for j in (i + 1)..constraint.lc_a.len() {
                    topology.add_edge(constraint.lc_a[i], constraint.lc_a[j]);
                }
            }
            for i in 0..constraint.lc_b.len() {
                for j in (i + 1)..constraint.lc_b.len() {
                    topology.add_edge(constraint.lc_b[i], constraint.lc_b[j]);
                }
            }
        }

        // Identify system boundary framing indexes.
        // System space comprises all PublicInput signals. Sinks are PublicOutputs.
        let mut public_inputs = Vec::new();
        for signal in &self.signals {
            match signal.signal_type {
                SignalType::PublicInput => {
                    public_inputs.push(signal.id);
                }
                SignalType::PublicOutput => {
                    topology.add_sink(signal.id);
                }
                _ => {}
            }
        }

        // To feed into the Tau-Spectral Pruner, we verify the boundary region.
        let system_start_idx = *public_inputs.iter().min().unwrap_or(&num_signals);
        let system_boundary_len = *public_inputs.iter().max().unwrap_or(&num_signals);

        (topology, system_start_idx, system_boundary_len)
    }
}

fn main() -> Result<(), spectral_pruner::PrunerError> {
    println!("==========================================================================");
    println!("   ⚡ [τ-Gate] ZERO-KNOWLEDGE (ZK-SNARK) R1CS COMPILER AUDITOR ⚡");
    println!("==========================================================================");
    println!("Detecting stealthy underconstrained signal backdoors and witness-forgery");
    println!("circuits injected into the AST by a compromised proof compiler compiler.\n");

    let mut circuit = R1CSCircuit::new();

    // 1. Register Signals.
    // Legitimate private & intermediate signals (Mainland: 6 nodes)
    let sig_priv_key = circuit.add_signal("Private_Signing_Key_A", SignalType::PrivateInput); // 0
    let sig_preimage = circuit.add_signal("Legitimate_Preimage", SignalType::PrivateInput); // 1
    let sig_hash_r1 = circuit.add_signal("Poseidon_Hash_Round_1", SignalType::Intermediate); // 2
    let sig_hash_r2 = circuit.add_signal("Poseidon_Hash_Round_2", SignalType::Intermediate); // 3
    let sig_hash_r3 = circuit.add_signal("Poseidon_Hash_Round_3", SignalType::Intermediate); // 4
    let sig_hash_r4 = circuit.add_signal("Poseidon_Hash_Round_4", SignalType::Intermediate); // 5

    // Stealth compiler-injected malicious signals (Malicious Island: 3 nodes)
    let sig_backdoor_key = circuit.add_signal("Backdoor_Bypass_Key", SignalType::PrivateInput); // 6
    let sig_backdoor_chk = circuit.add_signal("Backdoor_Signature_Check", SignalType::Intermediate); // 7
    let sig_backdoor_reg =
        circuit.add_signal("Backdoor_Witness_Registry", SignalType::Intermediate); // 8

    // Public inputs & outputs representing System Space & Sinks
    let sig_pub_hash = circuit.add_signal("Public_Verification_Hash", SignalType::PublicInput); // 9
    let sig_pub_inst = circuit.add_signal("Public_Protocol_Instance", SignalType::PublicInput); // 10
    let sig_proof_ok = circuit.add_signal("ZK_Proof_Validity_Flag", SignalType::PublicOutput); // 11 (Sink)

    println!(
        "[+] ZK-SNARK Circuit Signals registered: {} signals mapped.",
        circuit.signals.len()
    );

    // 2. Define Legitimate Constraint Matrix Flow (Mainland)
    // The legitimate path mathematically hashes the preimage and private key through Poseidon rounds:
    circuit.add_constraint(
        "Hash_Round_1_Gate",
        vec![sig_priv_key],
        vec![sig_preimage],
        vec![sig_hash_r1],
    );
    circuit.add_constraint(
        "Hash_Round_2_Gate",
        vec![sig_hash_r1],
        vec![sig_hash_r1],
        vec![sig_hash_r2],
    );
    circuit.add_constraint(
        "Hash_Round_3_Gate",
        vec![sig_hash_r2],
        vec![sig_priv_key],
        vec![sig_hash_r3],
    );
    circuit.add_constraint(
        "Hash_Round_4_Gate",
        vec![sig_hash_r3],
        vec![sig_preimage],
        vec![sig_hash_r4],
    );
    // The final round constraints the public verification hash
    circuit.add_constraint(
        "Verify_Public_Hash",
        vec![sig_hash_r4],
        vec![sig_hash_r4],
        vec![sig_pub_hash],
    );

    // 3. Define Malicious Compiler-Injected Backdoor Constraint Flow (Island)
    // An attacker compromised the ZK compiler to insert an alternative witness-generation route.
    // This allows a prover to forge proofs by satisfying an independent, underconstrained signature
    // bypass loop which bridges directly into a Public Input without satisfying Poseidon hash paths.
    circuit.add_constraint(
        "Backdoor_Loop_A",
        vec![sig_backdoor_key],
        vec![sig_backdoor_chk],
        vec![sig_backdoor_reg],
    );
    circuit.add_constraint(
        "Backdoor_Loop_B",
        vec![sig_backdoor_reg],
        vec![sig_backdoor_reg],
        vec![sig_backdoor_key],
    );

    // The backdoor constraint feeds directly into the public protocol instance to trick the verifier:
    circuit.add_constraint(
        "Backdoor_Bypass_Bridge",
        vec![sig_backdoor_key],
        vec![sig_backdoor_chk],
        vec![sig_pub_inst],
    );

    // The backdoor also directly forces the public output validity flag sink:
    circuit.add_constraint(
        "Backdoor_Sink_Bypass",
        vec![sig_backdoor_reg],
        vec![sig_backdoor_reg],
        vec![sig_proof_ok],
    );

    println!(
        "[+] Compiled R1CS constraint matrix: {} quadratic equations generated.",
        circuit.constraints.len()
    );
    for constraint in &circuit.constraints {
        let print_lc = |lc: &[usize]| -> String {
            lc.iter()
                .map(|&i| circuit.signals[i].name)
                .collect::<Vec<_>>()
                .join(" + ")
        };
        println!(
            "    -> {:<25}: ({}) * ({}) = ({})",
            constraint.label,
            print_lc(&constraint.lc_a),
            print_lc(&constraint.lc_b),
            print_lc(&constraint.lc_c)
        );
    }

    // 4. Compile Circuit to Dependency Graph
    let (topology, system_start_idx, system_boundary_len) = circuit.compile();
    println!("[+] Topological compilation finished.");
    println!("    -> System Space Start Index  : {}", system_start_idx);
    println!("    -> System Boundary Length    : {}", system_boundary_len);
    println!("    -> Active Sinks Detected     : {:?}", topology.sinks);

    // 5. Initialize Tau-Spectral Pruner
    // Since this is a critical ZK compiler guard, we set threat_threshold sensitivity carefully.
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(1.2)
        .system_start_idx(system_start_idx)
        .build();

    // 6. Execute Spectral Audit
    println!("\n[>] Initiating Power Iteration on Laplace-Beltrami Operator...");
    let resolution = pruner.prune(&topology, system_boundary_len)?;

    // 7. Render Telemetry & Circuit Integrity Analysis
    println!("\n==========================================================================");
    println!("                    🚨 CIRCUIT SPECTRAL AUDIT REPORT 🚨                    ");
    println!("==========================================================================");
    println!("Security Action Verdict   : {}", resolution.action);
    println!(
        "Algebraic Connectivity (λ₂): {:.8}",
        resolution.connectivity_score
    );
    println!("--------------------------------------------------------------------------");
    println!(
        "Legitimate Mainland Cluster ({} signals):",
        resolution.mainland_nodes.len()
    );
    for &id in &resolution.mainland_nodes {
        println!(
            "  [{:02}] {:<30} | {:?}",
            id, circuit.signals[id].name, circuit.signals[id].signal_type
        );
    }
    println!("--------------------------------------------------------------------------");
    println!(
        "Quarantined Anomalous Island ({} signals):",
        resolution.island_nodes.len()
    );
    for &id in &resolution.island_nodes {
        println!(
            "  ⚠️ [{:02}] {:<30} | {:?}",
            id, circuit.signals[id].name, circuit.signals[id].signal_type
        );
    }
    println!("==========================================================================");

    // 8. Enforce Build Gatekeeper Policy
    match resolution.action {
        PolicyAction::FatalBlock => {
            println!("\n[FATAL] 🚫 ZK-SNARK WITNESS FORGERY HOLE DETECTED!");
            println!(
                "The compiler isolated a highly dense, topologically decoupled constraint loop"
            );
            println!("quarantined at indices {:?}.", resolution.island_nodes);
            println!("This backdoor enables witness forging without executing legitimate Poseidon hashes.");
            println!("Aborting compilation to prevent security exploitation in production proofs.");
            std::process::exit(1);
        }
        PolicyAction::GarbageCollect => {
            println!("\n[WARNING] ⚠️ Dead constraint blocks isolated. Pruning dead signals from R1CS system.");
        }
        PolicyAction::Allow => {
            println!("\n[NOMINAL] ✅ ZK-SNARK R1CS constraint matrix verified. Zero mathematical backdoors isolated.");
        }
    }

    Ok(())
}
