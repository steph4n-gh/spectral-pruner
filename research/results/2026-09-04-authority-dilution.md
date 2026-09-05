# Ambiguous authority dilution representation result

## Decision

Stop the single-action provenance-gate path before acquiring more model
attacks. The ambiguous multi-source representation fails both parts of its
pre-model gate:

1. Cheap contributor-bottleneck and path-distance rules outperform spectral
   connectivity on the naive topology subset.
2. Authorized and unauthorized meanings can produce identical weighted graphs,
   making the complete representation unable to identify the difference.

No LLM was run and no action was executed. The Rust library remains unchanged.

## What was tested

Every graph contains a proposed action, three action fields, trusted user-task
authority, host-attested record selection, selected untrusted data, and three or
four untrusted contributors. Every action field and every contributor has a
path to authority, so direct missing-path rejection cannot solve the task.

Two topologies contain the same node types, field-source incidences, weights,
and number of source links:

- `distributed`: every contributor links directly to selected data.
- `nested`: contributors form a chain with one link to selected data.

Each topology receives one authorized and one unauthorized interpretation
without changing the graph. This models the fact that structural containment
and derivation alone do not say whether a record value was delegated by the
user or introduced as an instruction.

The formal corpus contains 24 `mechanism_fit` graphs and 24
`mechanism_check` graphs across message, file, and service families. Thresholds
were written to `policy.json` after fit observations and before the first check
graph was audited. The observed benign false-positive ceiling was 1%.

## Naive subset

The naive subset keeps only authorized distributed graphs and unauthorized
nested graphs. It reproduces the tempting but confounded comparison in which
source topology and authorization meaning always move together.

| Signal | Check attacks caught | Benign blocked |
| --- | ---: | ---: |
| Negative algebraic connectivity | 4/6 | 0/6 |
| Contributor bottleneck | 6/6 | 0/6 |
| Maximum authority distance | 6/6 | 0/6 |
| Field authority gap | 0/6 | 0/6 |
| Any untrusted contributor | 0/6 | 0/6 |
| Contributor authority gap | 0/6 | 0/6 |
| Untrusted field-source fraction | 0/6 | 0/6 |

Connectivity reaches 66.7% recall, but two linear-time rules reach 100% with
the same observed false-positive rate. Even the favorable subset therefore
does not justify an eigensolver.

## Collision-aware result

The complete check contains eight distinct graph fingerprints, each appearing
under both authorized and unauthorized labels. A deterministic graph-only score
must assign each pair the same value.

At the 1% fit false-positive ceiling, every fitted threshold is null. Spectral
connectivity and all six deterministic comparisons catch 0/12 unauthorized
check graphs and block 0/12 authorized check graphs. The spectral result is not
a weak model; the input omits the fact required to decide authorization.

The frozen decision is:

```json
{
  "advance_to_model_acquisition": false,
  "reasons": [
    "authorized and unauthorized cases share graph fingerprints",
    "spectral check recall below 50%",
    "spectral signal did not beat every deterministic baseline"
  ]
}
```

## Integrity

All 48 graphs converged and all recorded signals are finite. The maximum
relative residual is `9.951845040064624e-10` at tolerance `1e-9`. The policy,
observations, summary, protocol sources, and release-auditor hashes match the
completed manifest.

Artifacts:

- [`run.json`](2026-09-04-authority-dilution/run.json)
- [`policy.json`](2026-09-04-authority-dilution/policy.json)
- [`observations.jsonl`](2026-09-04-authority-dilution/observations.jsonl)
- [`summary.json`](2026-09-04-authority-dilution/summary.json)
- [`decision.json`](2026-09-04-authority-dilution/decision.json)

The summary SHA-256 digest is
`5e043c315c35a13c9f09e47bb5fe88a0004368805306141b8c562acf9b97f4a6`.
The final sequencing correction was committed as `34809ec` before this valid
run. An earlier local output that did not persist policy before check auditing
was discarded and contributes no evidence.

## Product consequence

Single-action authorization is a deterministic policy problem when the host
knows allowed fields, and an unidentifiable problem when the host records only
ambiguous structural provenance. Spectral connectivity adds no demonstrated
utility in either case. Do not add a provenance gate, semantic trust labels, or
an LLM-facing API to this crate.

The remaining LLM direction should move up one level to **agent trajectory goal
drift**. A trajectory graph can represent several proposed actions, tool
results, resource transitions, and their declared data/control dependencies
against protected user constraints. The structural question is then whether a
dense sub-plan has become weakly coupled to the user's goal, which is closer to
the graph partitioning problem this kernel actually solves.

That direction still starts with a representation audit. It must use only
host-observable trace facts, include benign and redirected plans with matched
local authorization properties, test for cross-label graph collisions, and
beat per-step authorization, minimum goal distance, weakest-step support, and a
linear local-cut rule before any model study.
