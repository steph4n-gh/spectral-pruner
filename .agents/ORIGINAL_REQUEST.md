# Original User Request

## 2026-08-27T22:20:34Z

# Teamwork Project Prompt — Draft

> Status: Launched
> Goal: Craft prompt → get user approval → delegate to teamwork_preview
> Requested team: Large-scale agent team

Use a very large team of agents. Research, optimize, and enhance the security mechanisms of the `spectral-pruner` Rust library based on the latest spectral graph theory advances as of August 2026, uplifting it into a production-ready critical component.

Working directory: `/Volumes/Storage/bigworkspace/spectral-pruner`
Integrity mode: development

## Requirements

### R1. Implement Modern Optimizations
Research advances in spectral graph theory (as of August 2026) and implement algorithmic optimizations and security enhancements directly into the Rust codebase.

### R2. Adhere to Hard System Constraints
Maintain the absolute zero-dependency footprint of the `spectral-pruner` crate. You must not introduce external linear algebra crates or async runtimes. 

### R3. Preserve Mathematical Invariants
Strictly preserve existing mathematical invariants documented in AGENTS.md, particularly $\tau$-Boundary Tie-Breaking, Arrington Clamping, the Scale-Invariant Semantic Density Ratio, and the Arrington Single-Token Tripwire.

## Acceptance Criteria

### Objective Verification
- [ ] `cargo tree` confirms zero new dependencies were introduced.
- [ ] `cargo test` passes all existing invariant tests without modifications to the existing tests themselves.
- [ ] New benchmarking or fuzzing tests are included that objectively demonstrate the performance or security improvements over the baseline.
- [ ] The codebase compiles without warnings or errors.
