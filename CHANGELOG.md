# Changelog

## Unreleased

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

This work adds public fields to `Topology` and `PrunerResolution`; callers that
construct either type with a struct literal will need to update. Constructor and
builder-based callers remain source-compatible. A semantic-version decision is
required before release.

