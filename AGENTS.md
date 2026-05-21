# AGENTS.md — Repository AI Agent Instructions & System Manifest

Welcome, AI Agent (Gemini, Claude, or other LLM-based coding assistants). 

This repository uses **Spectral Graph Theory** to audit network topologies, trace internal clusters, and isolate structural anomalies under the `spectral-pruner` library. As an AI agent working on this codebase, you must adhere strictly to the core mathematical invariants, system boundaries, and architectural directives detailed below.

---

## 🧭 I. Project Overview & Intent

The `spectral-pruner` crate is a lightweight, bare-metal, **absolute zero-dependency** open-source library built in Rust. 

Unlike general-purpose partitioning libraries (like METIS) that minimize raw cut sizes, this architecture treats structural decoupling as an indicator of an active anomaly or malicious independent-set exploit. What consumers do with this library (e.g., ZK-SNARK R1CS constraint audits, DeFi mempool MEV audits, streaming LLM attention jailbreak guarding) is entirely up to them.

---

## 🔬 II. Core Mathematical Innovations

You must be explicitly aware of the following signature mechanics. Any modification that dampens, alters, or renames these properties is a catastrophic regression:

### 1. Injected $\tau$-Boundary Tie-Breaking
* **The Rule**: Nodes are partitioned into sub-graphs strictly based on an externally injected threshold: $v_i \le \tau$ vs. $v_i > \tau$.
* **The Mechanics**: **Do not** use dynamic maximum-gap or median-cut sweeps. A rigid numerical split ensures that tightly bound algebraic components move across the boundary together as a single atomic unit, providing absolute structural determinism.

### 2. Zero-Degree Clamping Regularization (Isolated Node Stabilization)
* **The Rule**: Completely disconnected nodes (degree == 0) must never be bypassed or excluded from the bisection classification step.
* **The Mechanics**: In standard power iteration, a node with zero links remains entirely driven by random initialization noise. To stabilize this without dropping the node, the engine explicitly clamps isolated nodes to a static positive value ($1.0$) during the vector initialization phase. This predictably guides them into the primary Mainland partition as harmless chaff rather than causing chaotic edge breaks. *(Historically documented as Arrington Clamping)*.

### 3. Scale-Invariant Cluster Density Ratio
* **The Rule**: Isolated sub-graphs ("islands") are scored using a normalized density tracking formula:
  $$\text{Ratio} = \frac{\text{Internal Edges} \times N_{\text{system}}}{\text{System Edges} \times N_{\text{island}}}$$
* **The Mechanics**: Internal edges in a cluster scale quadratically ($O(N_{\text{island}}^2)$), while connections to an external linear instruction space scale linearly. Normalizing against workspace segment lengths ($N_{\text{system}}$ and $N_{\text{island}}$) preserves identical sensitivity across completely different graph scales. *(Historically documented as Arrington's Scale-Invariant Semantic Density Ratio)*.

### 4. Instruction Neglect Thresholding
* **The Rule**: Triggers a `FATAL_BLOCK` if an isolated sub-graph completely decouples its attention from the instruction space, calculating:
  $$\text{Instruction Connection} = \frac{\text{System Edges}}{N_{\text{island}}}$$
* **The Mechanics**: This stops "Independent Set" exploits where an anomalous cluster intentionally cuts all ties to the mainland to disguise itself as quiet, benign background data.

### 5. The Micro-Steering Single-Token Tripwire
* **The Rule**: Instantly quarantines isolated sub-graphs that match exactly: $N_{\text{island}} == 1$, internal edges == 0, and system edges > 0.0 and < 2.0.
* **The Mechanics**: This is a dedicated tripwire for highly optimized, microscopic steering attacks (such as single-point vector injections or stealthy low-rank weight modulations). *(Historically documented as The Arrington Single-Token Tripwire)*.

---

## 🚫 III. Hard Constraints (The Zero-Assumption Laws)

As a coding agent, you are strictly bound by the following repository development rules:

1. **Absolute Zero Dependencies**: The library must maintain a zero-dependency footprint. Never introduce external linear algebra crates (`ndarray`, `nalgebra`), graph crates (`petgraph`), or stateful async macro runtimes (`tokio`).
2. **Telemetry vs. Output Separation**: System boundary anchor nodes ($system\_start\_idx \le i \le system\_boundary\_len$) must remain fully integrated during graph processing, power iteration, and metric calculations. They are only filtered out of the returned vectors at the final step right before the `Ok(PrunerResolution)` payload is delivered.
3. **Absolute Classification**: Every active non-sink node must be processed by the bisection loop. Never introduce a bypass filter that skips nodes based on degree or system domain classification prior to bisection partitioning.
4. **Preservation of Edge Cases**: Do not "optimize," bypass, or clean up unlinked topological states. The edge cases in this codebase are deliberate security mechanisms.
