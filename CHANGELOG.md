# Changelog

## Unreleased

- Add a tested quick start, full CLI reference, mathematical contract, contributor
  guide, release checklist, and an actionable roadmap. Clarify source-candidate
  installation, verdict handling, and the historical scope of attention results.
- Refresh package discovery metadata and add CLI `--version` / `-V` output.
- Label domain examples as simulations and describe policy recommendations
  without claiming to modify external systems or certify their safety.
- Report convergence beside benchmark latency; expose iteration and tolerance
  settings on the weighted-TSV benchmark, defaulting to 10000 and 1e-9.
- Omit token strings from newly written evaluation predictions while preserving
  provenance and numerical signals. Existing artifacts are not rewritten.
- Verify the quick start and public documentation in CI, and add regression
  coverage for evaluation artifact contents and CLI help/version output.
- Update checkout, cache, and Python setup actions to the Node 24 runtime.

## 2.0.0-rc.1 — release candidate

### Stabilization

- Reject non-finite tau and tolerance at construction. Reject accumulated
  weight overflow and invalid numerical results before emitting a verdict.
- Fail closed for invalid protected intervals on every resolution path,
  including small graphs, edgeless graphs, and empty islands.
- Scale the shifted operator by maximum degree so uniformly tiny weights do
  not prematurely stop iteration; compute the Rayleigh quotient using edge energy.
- Expose convergence, iteration count, normalized eigenpair residual, and the
  numerical-failure policy trigger. Configured connectivity policies fail closed
  on an unconverged estimate when system policy is enabled.
- Add CLI iteration/tolerance controls and preserve tiny finite values in JSON.
- Add deterministic numerical stress cases and offline research/CLI tests to CI.
- Validate evaluation resume settings and saved row identities; refuse to score
  unconverged graphs. Older evaluation manifests require a fresh output directory.
- Fix current-stable Clippy's slice-clearing warning and include both license texts.
- Replace hardware-dependent debug timing assertions with repeated analytical
  connectivity checks; keep latency measurements in the release benchmarks.

See [MIGRATION.md](MIGRATION.md) for the changes from the published 1.0.0 API.

### Added

- Positive finite weighted edges and weighted CSR processing.
- Auditable partition, density, boundary, conductance, and trigger diagnostics.
- Optional calibrated algebraic-connectivity and configurable instruction
  thresholds, with mechanism-disable controls for ablation.
- Weighted TSV audit CLI with stdin streaming and versioned JSON output.
- Real aggregate and layerwise Hugging Face attention extraction, public-dataset
  evaluation, calibration, BIPIA adaptation, numerical oracle, stress variants,
  and a real-graph latency benchmark.

### Corrected

- Count undirected internal edges once.
- Use actual in-range protected node count instead of the boundary endpoint.
- Keep protected system nodes active even if also submitted as sinks.
- Fail closed on a nonzero invalid protected interval.
- State allocation and LLM-validation claims at their measured scope.

### Compatibility

This release adds public fields to `Topology`, `PrunerResolution`, and
`PrunerDiagnostics`; callers using struct literals must update. The published
1.0.0 API therefore requires a major version change. Version 2.0.0-rc.1 is a
reviewable prerelease, not a stable release or a production LLM-defense claim.
