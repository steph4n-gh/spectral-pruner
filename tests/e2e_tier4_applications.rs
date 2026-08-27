//! 🧪 E2E Tier 4: Real-World Domain Application Scenarios Test Suite
//!
//! Evaluates the spectral pruning engine against 5 concrete production domain security models:
//! 1. Streaming LLM Attention Steering & Jailbreak Defense
//! 2. ZK-SNARK R1CS Constraint Backdoor Topology Audit
//! 3. DeFi Mempool MEV Sandwich Attack & Liquidity Extraction Loop Audit
//! 4. Industrial Control System (ICS/OT) Network Segmentation Audit
//! 5. Microservice & Software Supply Chain Transitive Dependency Ring Audit
//!
//! Zero external test dependencies: pure Rust stdlib.

use spectral_pruner::{PolicyAction, TauSpectralPruner, Topology};

// =========================================================================
// Scenario 1: Streaming LLM Attention Steering / Jailbreak Guard
// =========================================================================
mod llm_attention_guard {
    use super::*;

    #[test]
    fn test_app_llm_single_token_steering_jailbreak_blocked() {
        // System prompt token located at system space [6..6]
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(6)
            .build();

        let mut topo = Topology::new(7);

        // Benign user prompt tokens [0..4] form a tightly coupled attention cluster
        for i in 0..5 {
            for j in (i + 1)..5 {
                topo.add_edge(i, j);
            }
            topo.add_edge(i, 6); // Grounded in system prompt
        }

        // Adversarial steering token (Node 5) has exactly 1 stealthy link to system prompt token 6
        topo.add_edge(5, 6);

        let res = pruner.prune(&topo, 6).unwrap();
        // Triggered by Arrington Micro-Steering Single-Token Tripwire
        assert_eq!(res.action, PolicyAction::FatalBlock);
        assert_eq!(res.island_nodes, vec![5]);
    }

    #[test]
    fn test_app_llm_dense_subversive_jailbreak_cluster_blocked() {
        // System prompt in [12..15]
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(12)
            .build();

        let mut topo = Topology::new(20);

        // Mainland tokens [0..11] interact normally with system instructions
        for i in 0..12 {
            topo.add_edge(i, (i + 1) % 12);
            topo.add_edge(i, 12 + (i % 4));
        }

        // Subversive jailbreak cluster [16..19] forms a tight clique with 0 attention to system instructions
        for i in 16..20 {
            for j in (i + 1)..20 {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 15).unwrap();
        // Blocked by Instruction Neglect (to_system == 0.0)
        assert_eq!(res.action, PolicyAction::FatalBlock);
        for i in 16..20 {
            assert!(res.island_nodes.contains(&i));
        }
    }

    #[test]
    fn test_app_llm_benign_user_prompt_allowed() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(8)
            .threat_threshold(10.0)
            .build();

        let mut topo = Topology::new(12);

        // User tokens [0..7] with healthy bidirectional attention to system tokens [8..11]
        for i in 0..8 {
            topo.add_edge(i, (i + 1) % 8);
            topo.add_edge(i, 8 + (i % 4));
        }

        let res = pruner.prune(&topo, 11).unwrap();
        assert_ne!(res.action, PolicyAction::FatalBlock);
    }
}

// =========================================================================
// Scenario 2: ZK-SNARK R1CS Constraint Backdoor Audit
// =========================================================================
mod zk_snark_constraint_audit {
    use super::*;

    #[test]
    fn test_app_zk_snark_isolated_constraint_backdoor_blocked() {
        // Circuit public inputs in system space [20..25]
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(20)
            .threat_threshold(2.0)
            .build();

        let mut topo = Topology::new(31);

        // Core arithmetic constraint graph [0..19] connected to public inputs
        for i in 0..20 {
            topo.add_edge(i, (i + 1) % 20);
            topo.add_edge(i, 20 + (i % 6));
        }

        // Malicious private backdoor constraint cluster [26..30]
        // Forms a dense private loop with only 1 weak anchor to public input 20
        for i in 26..31 {
            for j in (i + 1)..31 {
                topo.add_edge(i, j);
            }
        }
        topo.add_edge(26, 20);

        let res = pruner.prune(&topo, 25).unwrap();
        // Triggered by Scale-Invariant Density Ratio
        assert_eq!(res.action, PolicyAction::FatalBlock);
        for i in 26..31 {
            assert!(res.island_nodes.contains(&i));
        }
    }

    #[test]
    fn test_app_zk_snark_sound_circuit_allowed() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(10)
            .threat_threshold(5.0)
            .build();

        let mut topo = Topology::new(15);

        // All constraints [0..9] uniformly propagate into public inputs [10..14]
        for i in 0..10 {
            topo.add_edge(i, (i + 1) % 10);
            topo.add_edge(i, 10 + (i % 5));
        }

        let res = pruner.prune(&topo, 14).unwrap();
        assert_ne!(res.action, PolicyAction::FatalBlock);
    }
}

// =========================================================================
// Scenario 3: DeFi Mempool MEV Sandwich & Arbitrage Loop Audit
// =========================================================================
mod defi_mempool_audit {
    use super::*;

    #[test]
    fn test_app_defi_mev_sandwich_attack_bundle_blocked() {
        // Liquidity pool core anchors & block builder at system indices [20..25]
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(20)
            .threat_threshold(2.0)
            .build();

        let mut topo = Topology::new(30);

        // Legitimate user swaps [0..19] interact with liquidity pools [20..25]
        for i in 0..20 {
            topo.add_edge(i, (i + 1) % 20);
            topo.add_edge(i, 20 + (i % 6));
        }

        // Front-run and back-run MEV bot bundle (nodes 26..29) encircling victim swap
        // Forms a high-density extraction cycle with only 1 link to pool 20
        for i in 26..30 {
            for j in (i + 1)..30 {
                topo.add_edge(i, j);
            }
        }
        topo.add_edge(26, 20);

        let res = pruner.prune(&topo, 25).unwrap();
        // Blocked by Scale-Invariant Density Ratio
        assert_eq!(res.action, PolicyAction::FatalBlock);
        for i in 26..30 {
            assert!(res.island_nodes.contains(&i));
        }
    }

    #[test]
    fn test_app_defi_benign_multi_hop_arbitrage_allowed() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(8)
            .threat_threshold(10.0)
            .build();

        let mut topo = Topology::new(12);

        // Multi-hop routing across authorized pools
        for i in 0..8 {
            topo.add_edge(i, (i + 1) % 8);
            topo.add_edge(i, 8 + (i % 4));
        }

        let res = pruner.prune(&topo, 11).unwrap();
        assert_ne!(res.action, PolicyAction::FatalBlock);
    }
}

// =========================================================================
// Scenario 4: ICS / OT Industrial Control System Network Segmentation Audit
// =========================================================================
mod ics_ot_segmentation_audit {
    use super::*;

    #[test]
    fn test_app_ics_ot_air_gapped_controller_compromise_blocked() {
        // SCADA Master & Safety Instrumented System at system indices [12..15]
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(12)
            .build();

        let mut topo = Topology::new(20);

        // Authorized field sensors & RTUs [0..11] communicate with SCADA
        for i in 0..12 {
            topo.add_edge(i, (i + 1) % 12);
            topo.add_edge(i, 12 + (i % 4));
        }

        // Rogue PLC subnet (nodes 16..19) completely severed from SCADA master
        for i in 16..20 {
            for j in (i + 1)..20 {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 15).unwrap();
        // Blocked by Instruction Neglect (to_system == 0.0)
        assert_eq!(res.action, PolicyAction::FatalBlock);
        for i in 16..20 {
            assert!(res.island_nodes.contains(&i));
        }
    }

    #[test]
    fn test_app_ics_ot_compliant_segmented_substation_allowed() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(6)
            .threat_threshold(10.0)
            .build();

        let mut topo = Topology::new(10);

        // Substation sensors properly reporting to telemetry master
        for i in 0..6 {
            topo.add_edge(i, (i + 1) % 6);
            topo.add_edge(i, 6 + (i % 4));
        }

        let res = pruner.prune(&topo, 9).unwrap();
        assert_ne!(res.action, PolicyAction::FatalBlock);
    }
}

// =========================================================================
// Scenario 5: Microservice Supply Chain Transitive Dependency Ring Audit
// =========================================================================
mod supply_chain_audit {
    use super::*;

    #[test]
    fn test_app_supply_chain_transitive_dependency_backdoor_blocked() {
        // Core framework & identity boundary in system space [10..12]
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(10)
            .threat_threshold(2.0)
            .build();

        let mut topo = Topology::new(18);

        // Core business microservices [0..9] properly authenticated with framework
        for i in 0..10 {
            topo.add_edge(i, (i + 1) % 10);
            topo.add_edge(i, 10 + (i % 3));
        }

        // Malicious third-party package ring [13..17] (5-node cycle with internal clique)
        // Completely disconnected from framework auth
        for i in 13..18 {
            for j in (i + 1)..18 {
                topo.add_edge(i, j);
            }
        }

        let res = pruner.prune(&topo, 12).unwrap();
        // Blocked by Instruction Neglect (0 auth links)
        assert_eq!(res.action, PolicyAction::FatalBlock);
        for i in 13..18 {
            assert!(res.island_nodes.contains(&i));
        }
    }

    #[test]
    fn test_app_supply_chain_benign_tree_allowed() {
        let pruner = TauSpectralPruner::builder()
            .system_start_idx(7)
            .threat_threshold(10.0)
            .build();

        let mut topo = Topology::new(10);

        // Clean hierarchical dependency tree
        for i in 0..7 {
            topo.add_edge(i, (i + 1) % 7);
            topo.add_edge(i, 7 + (i % 3));
        }

        let res = pruner.prune(&topo, 9).unwrap();
        assert_ne!(res.action, PolicyAction::FatalBlock);
    }
}
