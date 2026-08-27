# Project Plan: Spectral-Pruner Library Uplift

## Objective
Uplift the `spectral-pruner` Rust zero-dependency crate with:
1. August 2026 spectral graph theory advancements, algorithmic optimizations (e.g. accelerated eigensolvers, zero-alloc sparse power iteration, SIMD vectorization, cache-friendly CSR/CSC representations without external crates).
2. Enhanced security analysis (topological anomaly detection, structural decoupling, adversarial graph resistance).
3. Preserving strict mathematical invariants from `AGENTS.md` (tau-boundary, Arrington clamping, Scale-Invariant Semantic Density Ratio, Instruction Neglect, Arrington Single-Token Tripwire).
4. Comprehensive test verification, benchmarks, and fuzzing harness showing performance improvements without modifying existing tests.
5. Zero dependency additions (`cargo tree` verification) and zero compiler warnings/errors.

## Phases
- **Phase 0: Survey & Research**
  - Explorer 1: Codebase architecture, existing tests, invariants, data structures, and current performance hotspots.
  - Explorer 2 (Spec Miner): AGENTS.md invariant requirements, mathematical specifications, constraints, API surface.
  - Explorer 3: 2026 state-of-the-art spectral graph theory research, zero-dependency eigensolver acceleration techniques (e.g., Chebyshev acceleration, conjugate gradient on Laplacian, deflation, shift-and-invert, memory-aligned sparse matrix operations).
- **Phase 1: Architecture & Milestone Decomposition**
  - Create `PROJECT.md` with Feature Inventory, Architecture, Interface Contracts, and Milestones.
  - Create `TEST_INFRA.md` for dual-track testing and fuzzing/benchmarks.
- **Phase 2: Implementation & Iteration Loop**
  - Milestone execution with Workers, Reviewers, Challengers, and Forensic Auditors.
- **Phase 3: Fuzzing, Benchmarks & E2E Validation**
  - Run comprehensive benchmarks and fuzzing suite.
  - Verify zero-dependency footprint and 100% test pass.
- **Phase 4: Final Forensic Audit & Deliverables Synthesis**
  - Full codebase integrity audit.
  - Final report synthesis to parent/user.
