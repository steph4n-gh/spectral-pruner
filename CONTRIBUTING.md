# Contributing

Useful contributions improve a concrete graph workflow, reproduce a numerical
failure, clarify a public contract, or add evidence for a claimed application.
Keep changes small enough to review and keep the Rust crate dependency-free.

## Get started

```sh
git clone https://github.com/steph4n-gh/spectral-pruner.git
cd spectral-pruner
cargo run --example quick_start
cargo test --all-targets
cargo test --doc
```

The Rust library, CLI, and examples use the standard library only. Offline research
tests also need Python 3.10+ and a built release CLI. The numerical oracle requires
NumPy; model extraction additionally requires the packages in
`research/requirements.txt`.

For the offline checks, install only NumPy and build the CLI:

```sh
python3 -m pip install numpy==2.2.6
cargo build --release --bin spectral-pruner-audit
python3 -m unittest discover -s research -p 'test_*.py' -v
python3 research/numerical_oracle.py
```

## Propose a change

For a bug, include the crate version or commit, a minimal graph, builder/CLI
settings, expected result, and actual result. Synthetic edge lists are ideal
when the original data cannot be shared. Report a numerical discrepancy with its
reference value and convergence diagnostics.

For a feature, describe a real caller and the current limitation before proposing
an interface. Reuse existing code and avoid abstractions for hypothetical users.
The [roadmap](ROADMAP.md) lists the current priorities.

## Pull requests

Use a focused branch and explain the behavior change plus validation. Add tests
that reproduce meaningful failures or protect a public contract. Do not change
an expected verdict merely to make a test pass; explain why the old expectation
conflicted with the contract.

Run the checks in [DEVELOPMENT.md](DEVELOPMENT.md#verification). CI also verifies
the public documentation and quick start. Measure performance using release
benchmarks, recording the host, settings, and convergence; avoid absolute timing
assertions on shared CI machines.

Preserve the injected tau boundary, isolated-node initialization and classification,
protected-node participation, signature density ratio, instruction neglect, and
exact single-token tripwire. `AGENTS.md` and `DEVELOPMENT.md` describe these
constraints. Any intentional public API change needs migration notes and an
appropriate version change.

## Research contributions

Pin model and dataset revisions. Keep calibration and evaluation splits separate,
report false positives alongside detection, and compare against simple baselines.
Synthetic scenarios demonstrate code paths; they do not validate production
security. For prompt-injection research, distinguish attack-labeled text from an
attack that actually changes a model's behavior.

Please make reviews specific, constructive, and respectful. Contributions are
provided under the repository's MIT OR Apache-2.0 license unless explicitly agreed
otherwise before submission.
