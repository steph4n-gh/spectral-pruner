# E2E Test Infra: spectral-pruner

## Test Philosophy
- **Opaque-box & Requirement-Driven**: Tests exercise the public API (`TauSpectralPruner`, `Topology`, `PrunerBuilder`, `PrunerWorkspace`, `PrunerResolution`) strictly against mathematical invariants and security directives from `ORIGINAL_REQUEST.md` and `AGENTS.md`.
- **Zero External Test Dependencies**: Test harness uses pure standard-library Rust and deterministic pseudo-random generators (e.g. LCG / Xorshift in <10 lines of pure Rust) with 0 external crates.
- **Methodology**: Systematic 4-tier testing hierarchy + property-based testing and fuzzing:
  - **Tier 1 (Feature Coverage)**: >=5 test cases per feature covering happy paths and core mechanics in isolation.
  - **Tier 2 (Boundary & Corner Cases)**: >=5 test cases per feature covering boundary conditions (empty graphs, disconnected nodes, large stars, dense cliques, symmetric topologies, edge limits).
  - **Tier 3 (Cross-Feature Combinations)**: Pairwise combinatorial testing of feature interactions (sinks + isolated nodes, custom tau + single-token tripwire, scale-invariant density on large vs small systems).
  - **Tier 4 (Real-World Application Scenarios)**: High-level realistic workloads (LLM attention jailbreak defense, ZK constraint audits, DeFi mempool sandwich attack clustering, OT network segregation).
  - **Adversarial Fuzzing**: Property-based invariant verification across 10,000+ random graph configurations.

---

## Feature Inventory & Test Mapping
| # | Feature | Requirement Source | Tier 1 (Count) | Tier 2 (Count) | Tier 3 | Tier 4 |
|---|---------|-------------------|:--------------:|:--------------:|:------:|:------:|
| 1 | `Topology` & Sink Filtering | `AGENTS.md:50`, `src/engine.rs` | 5 | 5 | ✓ | ✓ |
| 2 | Contiguous `CsrGraph` Compilation | 2026 Research | 5 | 5 | ✓ | ✓ |
| 3 | Fast `BitSet` Node Masking | 2026 Research | 5 | 5 | ✓ | ✓ |
| 4 | Edge-Case Fast Paths ($N<3, \max(d)=0$) | `AGENTS.md:47-53` | 5 | 5 | ✓ | ✓ |
| 5 | Arrington Zero-Degree Clamping | `AGENTS.md:25-28` | 5 | 5 | ✓ | ✓ |
| 6 | Shifted Laplacian SpMV & Operator $M$ | `src/engine.rs:173-276` | 5 | 5 | ✓ | ✓ |
| 7 | Null-Space Projection & Momentum | 2026 Research | 5 | 5 | ✓ | ✓ |
| 8 | Rayleigh Quotient Connectivity ($\lambda_2$) | `src/engine.rs:256-267` | 5 | 5 | ✓ | ✓ |
| 9 | Reusable `PrunerWorkspace` Zero-Alloc | 2026 Research | 5 | 5 | ✓ | ✓ |
| 10 | Injected $\tau$-Boundary Tie-Breaking | `AGENTS.md:21-23` | 5 | 5 | ✓ | ✓ |
| 11 | Scale-Invariant Density Ratio | `AGENTS.md:29-33` | 5 | 5 | ✓ | ✓ |
| 12 | Instruction Neglect Thresholding | `AGENTS.md:34-38` | 5 | 5 | ✓ | ✓ |
| 13 | Micro-Steering Single-Token Tripwire | `AGENTS.md:39-42` | 5 | 5 | ✓ | ✓ |
| 14 | Policy Verdict Evaluation | `src/engine.rs:368-377` | 5 | 5 | ✓ | ✓ |
| 15 | Telemetry vs Output Separation | `AGENTS.md:50` | 5 | 5 | ✓ | ✓ |
| 16 | Builder & Config Validation | `src/error.rs` | 5 | 5 | ✓ | ✓ |

---

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | Streaming LLM Attention Steering Guard | F1, F5, F10, F13, F14, F15 | High |
| 2 | ZK-SNARK R1CS Constraint Backdoor Audit | F1, F2, F9, F11, F12, F14 | High |
| 3 | DeFi Mempool Sandwich & MEV Loop Audit | F1, F2, F8, F11, F14 | High |
| 4 | OT / Industrial Control System Segregation | F1, F10, F12, F14, F15 | Medium |
| 5 | Software Supply Chain Transitive Ring Audit | F1, F2, F11, F13, F14 | Medium |

---

## Test Runner & Execution
- **Run All Tests**: `cargo test --all-targets`
- **Run Integration Suites**:
  - `cargo test --test e2e_tier1_features`
  - `cargo test --test e2e_tier2_boundaries`
  - `cargo test --test e2e_tier3_combinatorial`
  - `cargo test --test e2e_tier4_applications`
  - `cargo test --test fuzz_adversarial`
- **Run Benchmarks**: `cargo run --release --example benchmark_suite`
