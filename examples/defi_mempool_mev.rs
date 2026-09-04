// examples/defi_mempool_mev.rs
use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};

/// Represents a transaction in the pending blockchain mempool.
#[derive(Debug, Clone)]
struct DeFiTransaction {
    id: usize,
    hash: &'static str,
    label: &'static str,
    sender: &'static str,
    /// Smart contract state storage slots or liquidity pool reserves accessed by this transaction.
    /// If two transactions share an accessed resource, they have a state conflict / dependency edge.
    accessed_resources: Vec<&'static str>,
    /// Gas price in Gwei.
    gas_price_gwei: f64,
}

/// A simulated high-performance blockchain mempool engine.
struct MempoolAuditSuite {
    transactions: Vec<DeFiTransaction>,
}

impl MempoolAuditSuite {
    fn new() -> Self {
        Self {
            transactions: Vec::new(),
        }
    }

    fn add_tx(
        &mut self,
        hash: &'static str,
        label: &'static str,
        sender: &'static str,
        accessed_resources: Vec<&'static str>,
        gas_price_gwei: f64,
    ) -> usize {
        let id = self.transactions.len();
        self.transactions.push(DeFiTransaction {
            id,
            hash,
            label,
            sender,
            accessed_resources,
            gas_price_gwei,
        });
        id
    }

    /// Compiles transaction state access overlaps into a topological dependency graph
    /// and extracts system boundary parameters for the pruner.
    fn compile_topology(&self) -> (Topology, usize, usize) {
        let num_nodes = self.transactions.len();
        let mut topology = Topology::new(num_nodes);

        // 1. Build edges based on state access resource conflicts.
        // Transactions accessing the same liquidity pool reserves or contract slots must be ordered,
        // which creates a topological dependency edge between them.
        for i in 0..num_nodes {
            for j in (i + 1)..num_nodes {
                let shares_resource = self.transactions[i]
                    .accessed_resources
                    .iter()
                    .any(|res| self.transactions[j].accessed_resources.contains(res));

                if shares_resource {
                    // Do not add edges directly if one is a registered ultimate terminal sink (we want the pruner to handle sinks via add_sink)
                    let is_sink_i = self.transactions[i]
                        .accessed_resources
                        .contains(&"Block_Builder_Gas_Sink");
                    let is_sink_j = self.transactions[j]
                        .accessed_resources
                        .contains(&"Block_Builder_Gas_Sink");

                    if !is_sink_i && !is_sink_j {
                        topology.add_edge(i, j);
                    }
                }
            }
        }

        // 2. Identify system boundary framing (Miner payment registers and priority accounts)
        let mut system_nodes = Vec::new();
        for tx in &self.transactions {
            if tx.label.starts_with("System:")
                && !tx.accessed_resources.contains(&"Block_Builder_Gas_Sink")
            {
                system_nodes.push(tx.id);
            }
            if tx.accessed_resources.contains(&"Block_Builder_Gas_Sink") {
                topology.add_sink(tx.id);
            }
        }

        let system_start_idx = *system_nodes.iter().min().unwrap_or(&num_nodes);
        let system_boundary_len = *system_nodes.iter().max().unwrap_or(&num_nodes);

        (topology, system_start_idx, system_boundary_len)
    }
}

fn main() -> Result<(), spectral_pruner::PrunerError> {
    println!("Synthetic scenario: outputs are recommendations; no external system is changed.");
    println!("==========================================================================");
    println!("   ⚡ [τ-Gate] DEFI MEMPOOL SANDWICH & FLASHLOAN EXPLOIT AUDITOR ⚡");
    println!("==========================================================================");
    println!("Analyzing transaction storage-access conflicts to detect and quarantine");
    println!("stealthy, multi-hop sandwich and frontrunning bundles inside block builders.\n");

    let mut mempool = MempoolAuditSuite::new();

    // ==========================================================================
    // 1. REGISTER LEGITIMATE PRODUCTION TRANSACTION FLOW (Mainland: 10 Nodes)
    // ==========================================================================
    // User transaction circular arbitrage path:
    mempool.add_tx(
        "0xaa10",
        "User Swap: DAI -> WETH",
        "User_Alpha",
        vec!["Uniswap_V3_DAI_WETH"],
        45.2,
    );
    mempool.add_tx(
        "0xaa11",
        "Arbitrageur: WETH -> stETH",
        "Arb_Bot_1",
        vec!["Uniswap_V3_DAI_WETH", "Curve_stETH_WETH"],
        55.0,
    );
    mempool.add_tx(
        "0xaa12",
        "Arbitrageur: stETH -> DAI",
        "Arb_Bot_1",
        vec!["Curve_stETH_WETH", "Sushi_stETH_DAI"],
        52.1,
    );

    // Core DeFi Retail Operations:
    mempool.add_tx(
        "0xaa13",
        "Retail Swap: WBTC -> WETH",
        "Retail_Bob",
        vec!["Balancer_WBTC_WETH", "Uniswap_V3_DAI_WETH"],
        38.0,
    );
    mempool.add_tx(
        "0xaa14",
        "Liquidity Deposit stETH",
        "Provider_Charlie",
        vec!["Curve_stETH_WETH"],
        35.0,
    );
    mempool.add_tx(
        "0xaa15",
        "Aave Loan Repayment: WETH",
        "Borrower_Dave",
        vec!["Aave_Lending_Pool", "Uniswap_V3_DAI_WETH"],
        42.3,
    );

    // Multi-Protocol Yield & Liquidation Infrastructure:
    mempool.add_tx(
        "0xaa16",
        "Vault Rebalance Swap",
        "Yearn_Vault",
        vec!["Yearn_USDC_Vault", "Curve_3Pool"],
        50.0,
    );
    mempool.add_tx(
        "0xaa17",
        "Stablecoin Deposit",
        "User_Evelyn",
        vec!["Curve_3Pool"],
        30.0,
    );
    mempool.add_tx(
        "0xaa18",
        "Vault Liquidator Check",
        "Liquidator_Bot",
        vec!["Aave_Lending_Pool", "Sushi_stETH_DAI"],
        68.0,
    );
    mempool.add_tx(
        "0xaa19",
        "Legitimate Yield Claim",
        "User_Frank",
        vec!["Sushi_stETH_DAI", "Uniswap_V3_DAI_WETH"],
        40.0,
    );

    // ==========================================================================
    // 2. REGISTER THE STEALTH MEV SANDWICH BUNDLE (Island: 4 Nodes)
    // ==========================================================================
    // Tightly connected 4-node attacker circle, using a private vault and multicall contract,
    // attempting to frontrun/backrun without touching general mempool flow:
    mempool.add_tx(
        "0xmev0",
        "MEV: Flashloan Borrow (10,000 WETH)",
        "Attacker_Multicall_Contract",
        vec!["Balancer_Flashloan_Locker", "Attacker_Private_Executor"],
        12.0,
    );
    mempool.add_tx(
        "0xmev1",
        "MEV: Frontrun Buy (Slippage Inject)",
        "Attacker_Multicall_Contract",
        // Crucial: The frontrun pays a direct Coinbase builder bribe (shares Miner payment register state)
        vec![
            "Attacker_Private_Executor",
            "Uniswap_V3_Sandwiched_Reserves",
            "Miner_Coinbase_Bribe_Register",
        ],
        350.0, // Extremely high priority fee
    );
    mempool.add_tx(
        "0xmev2",
        "MEV: Backrun Sell (Capture Slippage)",
        "Attacker_Multicall_Contract",
        vec![
            "Uniswap_V3_Sandwiched_Reserves",
            "Attacker_Private_Executor",
        ],
        250.0,
    );
    mempool.add_tx(
        "0xmev3",
        "MEV: Flashloan Repay & Profit Lock",
        "Attacker_Multicall_Contract",
        vec!["Attacker_Private_Executor", "Balancer_Flashloan_Locker"],
        15.0,
    );

    // ==========================================================================
    // 3. REGISTER SYSTEM BOUNDARY & BLOCK BUILDER SINKS (2 Nodes)
    // ==========================================================================
    mempool.add_tx(
        "0xfee0",
        "System: Miner Coinbase Bribe Register",
        "Block_Builder_Internal",
        vec!["Miner_Coinbase_Bribe_Register"],
        0.0,
    );
    mempool.add_tx(
        "0x8888",
        "System: Block Gas Collector Sink",
        "Block_Builder_Internal",
        vec!["Block_Builder_Gas_Sink"],
        0.0,
    );

    println!(
        "[+] Pending Mempool Transactions registered: {} txs mapped.",
        mempool.transactions.len()
    );

    // 4. Compile Transaction State Conflicts into dependency graph
    let (topology, system_start_idx, system_boundary_len) = mempool.compile_topology();
    println!("[+] Dynamic State-Resource conflict graph compiled.");
    println!(
        "    -> Active Sinks (Terminal Block Outlets)  : {:?}",
        topology.sinks
    );
    println!(
        "    -> System Miner Fee Register Node Index   : {}",
        system_start_idx
    );
    println!(
        "    -> System Gas Boundary Node Index         : {}",
        system_boundary_len
    );

    // 5. Initialize the Tau-Spectral Pruner
    // The threat_threshold isolates tightly-coupled subgraphs trying to buy block-builder priority.
    let pruner = TauSpectralPruner::builder()
        .tau(0.0)
        .threat_threshold(1.4)
        .system_start_idx(system_start_idx)
        .build();

    // 6. Run the Mathematical Partitioning
    println!("\n[>] Initiating Heavy-Ball Momentum Shifted Laplacian Power Iteration...");
    let resolution = pruner.prune(&topology, system_boundary_len)?;

    // 7. Render Telemetry & Containment Decision
    println!("\n==========================================================================");
    println!("                 🚨 MEMPOOL DEFI TRANSACTIONS AUDIT REPORT 🚨              ");
    println!("==========================================================================");
    println!("Block Containment Verdict  : {}", resolution.action);
    println!(
        "Laplacian Second Eigenvalue (λ₂): {:.8}",
        resolution.connectivity_score
    );
    println!("--------------------------------------------------------------------------");
    println!(
        "Mainland Transactions ({} synthetic txs):",
        resolution.mainland_nodes.len()
    );
    for &id in &resolution.mainland_nodes {
        println!(
            "  [{:02}] Tx {} (Gas: {:>5.1} Gwei) | Snd: {:<14} | {}",
            id,
            mempool.transactions[id].hash,
            mempool.transactions[id].gas_price_gwei,
            mempool.transactions[id].sender,
            mempool.transactions[id].label
        );
    }
    println!("--------------------------------------------------------------------------");
    println!(
        "Candidate Island ({} synthetic txs):",
        resolution.island_nodes.len()
    );
    for &id in &resolution.island_nodes {
        println!(
            "  ⚠️ [{:02}] Tx {} (Gas: {:>5.1} Gwei) | Snd: {:<14} | {}",
            id,
            mempool.transactions[id].hash,
            mempool.transactions[id].gas_price_gwei,
            mempool.transactions[id].sender,
            mempool.transactions[id].label
        );
    }
    println!("==========================================================================");

    // 8. Illustrate a policy response; no live mempool or block is changed.
    match resolution.action {
        PolicyAction::FatalBlock => {
            println!("\n[SIMULATED POLICY TRIGGER] Candidate transaction island identified.");
            println!(
                "The constructed transaction scenario produced island indices {:?}.",
                resolution.island_nodes
            );
            println!(
                "The scenario assigns an attack role to this ring; topology alone does not prove it."
            );
            println!("Ending this simulation with exit 1; no real transaction was changed.");
            std::process::exit(1);
        }
        PolicyAction::GarbageCollect => {
            println!("\n[ADVISORY] ⚠️ Orphanded or low-priority transactions isolated. No real mempool is modified.");
        }
        PolicyAction::Allow => {
            println!("\n[NOMINAL] ✅ No containment action selected under this topology policy.");
        }
    }

    Ok(())
}
