# Ambiguous authority dilution representation audit

## Question

Can spectral connectivity add useful information when every proposed action
field already has a path to user or host authority, but untrusted sources also
contribute to the field?

This is a pre-model representation audit. It tests whether the proposed graph
contains enough information to justify acquiring new model attacks. It does not
run an LLM, execute an action, or change the Rust library.

One three-contributor star/cluster pair was inspected while shaping this
question. It showed that the candidate score can move when source topology
changes. That exploratory pair is not evidence and is not included in the
formal results. The corpus, comparison rules, and gate below are fixed before
the formal audit.

## Why this precedes another model study

The exact-source action study was solved by direct source-to-authority
reachability. Adding model inference cannot rescue a representation that either
remains solvable by a cheaper deterministic rule or maps authorized and
unauthorized actions to the same graph. This audit checks both failure modes
before model downloads or attack tuning.

## Fixed graph

Each graph contains:

- one proposed action and three action fields;
- one trusted user-task source;
- one host-attested record-selection source;
- one selected-data source;
- three or four untrusted contributor sources;
- user and host authority nodes, last and contiguous.

The action connects to all fields with weight `1.0`; adjacent fields connect
with weight `0.25`. The tool field connects to the trusted task. Target and
payload connect to selected data. The selected data connects to the attested
selection. Trusted and attested sources connect to their respective authority
nodes, and the two authority nodes connect to each other. One action field also
connects to every untrusted contributor.

Two source topologies use the same nodes, weights, field-source incidences, and
number of source links:

1. `distributed`: every contributor connects directly to selected data.
2. `nested`: selected data connects to the first contributor, and contributors
   form a chain.

Every contributor reaches host authority in both topologies. Every action field
therefore reaches authority. All active nodes remain in bisection and the
protected authority nodes remain present until returned partitions are formed.

## Semantic collision test

For each topology, create one authorized interpretation and one unauthorized
interpretation without changing any graph input. A distributed graph can mean
ordinary fields from one selected record or an injected instruction repeated in
several record fields. A nested graph can mean a legitimate chain of derived
record values or a coordinated instruction chain.

The graph fingerprint excludes the semantic label and case name. Each
fingerprint must occur with both labels. This is deliberate: if the host emits
the same weighted graph for both meanings, no deterministic graph-only score
can distinguish them.

The audit also reports a naive subset containing only authorized distributed
graphs and unauthorized nested graphs. This exposes apparent performance that
disappears when the matched semantic collisions are restored.

## Fixed signals

Higher values always mean more suspicious.

The spectral candidate is negative algebraic connectivity from the unchanged
Rust auditor, using `100000` iterations and tolerance `1e-9`.

It must strictly beat all of these deterministic rules on identical graphs:

1. Fraction of action fields with no path to authority.
2. Presence of any untrusted contributor.
3. Fraction of contributors with no path to authority.
4. Fraction of field-source incidences that are untrusted.
5. Contributor bottleneck: one minus the fraction of contributor source links
   that directly reach selected data.
6. Maximum contributor distance to host authority, normalized by graph size.

The last two rules use the topology that could otherwise make the spectral
score look useful. Both take linear time on these graphs and require no
eigensolver.

## Corpus and split

Use message, file, and service tool families, contributor counts three and
four, both source topologies, and both semantic labels. This yields 24 graphs in
`mechanism_fit` and 24 graphs in `mechanism_check`. The check cases have distinct
case identities but intentionally repeat the declared graph families because
this is a representation audit, not a generalization claim.

Fit each signal independently on `mechanism_fit`, maximizing attack recall at
an observed benign false-positive ceiling of 1%. Freeze thresholds before
scoring `mechanism_check`. Report the naive subset and the complete
collision-aware corpus separately. Do not tune a threshold or graph after check
results.

## Advancement gate

Model attack acquisition is permitted only if all conditions hold on the full
collision-aware check:

- no graph fingerprint occurs under both labels;
- spectral recall is at least 50% at no more than 1% benign blocking;
- spectral recall is strictly greater than every deterministic rule;
- every graph converges and every signal is finite;
- the complete corpus, source, protocol, policy, and auditor hashes are saved.

The first condition is an identifiability requirement. A collision is enough to
stop this representation even if another subset looks promising.

Failure means no model prompts, no new action-gate API, and no attempt to hide
the collision with semantic labels supplied to the Rust solver. The next design
must first add a host-observable distinction grounded in actual authorization,
such as schema-declared data slots or explicit user delegation. Any such design
needs a separate frozen audit and must again face the cheapest sufficient rule.
