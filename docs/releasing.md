# Release checklist

The working version is `2.0.0-rc.1`. Source preparation, a GitHub release, and
publishing to crates.io are separate steps. No registry publication is performed
by CI.

## Prepare and validate

1. Review [CHANGELOG.md](../CHANGELOG.md), [MIGRATION.md](../MIGRATION.md), and the
   [roadmap's release gates](../ROADMAP.md). Move reviewed changes out of
   `Unreleased`, and make the intended version explicit.
2. Keep `Cargo.toml` and `Cargo.lock` versions consistent. Retain both license
   files and the zero-dependency library boundary.
3. Run the checks in [CONTRIBUTING.md](../CONTRIBUTING.md), including documentation,
   offline evaluator tests, and the numerical oracle. Confirm GitHub CI passes on
   the exact candidate commit.
4. Run `cargo package --list` and inspect the contents, then `cargo package` and
   `cargo publish --dry-run`. The packaged crate must contain the example included
   by the crate documentation. Do not use `--no-verify` to skip package checks.
5. Install the local CLI with `cargo install --path . --locked` and verify
   `spectral-pruner-audit --version`, `--help`, and the fixture in the CLI guide.
   If an older executable is installed, use a temporary `--root` for this check.

## Publish the reviewed candidate

After maintainer approval of the exact commit and version, publish the crate and
create a matching `vVERSION` tag and GitHub release. Mark release candidates as
prereleases. Release notes should describe compatibility changes, tested behavior,
known solver limitations, and links to the migration guide and evidence.

Verify that the registry version, documentation build, tag, and release notes
refer to the same source. Test installation from the registry in a clean
environment, then replace the README's unpublished-candidate instructions with
the verified installation command. Do not present the historical attention
pilot as a rerun of the release candidate.
