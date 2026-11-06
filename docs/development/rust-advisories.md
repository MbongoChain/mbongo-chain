# Rust Dependency Advisories

How the committed `Cargo.lock` is audited, and what is known about it. The register is
[`rust-advisories.json`](rust-advisories.json).

## Policy

- `cargo-audit`, at a pinned version, audits `Cargo.lock` in `.github/workflows/rust-audit.yml`:
  on dependency changes, on `dev`, and every Monday.
- **Nothing is suppressed.** No advisory is ignored in `cargo audit`. Every run prints every
  finding, known or not.
- The job fails on a finding missing from the register, on a register entry no longer
  reported, and on a report that does not cover every package in `Cargo.lock`.
- The advisory database is deliberately the latest one, so this workflow can fail with no
  change here. That is its purpose. It is separate from `ci.yml` so that it does not block
  unrelated pull requests.
- A register entry records a finding. **It does not accept the risk.** Accepting one is a
  maintainer decision, recorded in that entry's `decision`.

## When the job fails

1. Read the finding, and find what pulls the package in.
2. Add it to the register, or remove the entry if it is no longer reported, in a pull request
   of its own.
3. `status` is `open` until someone has assessed it. `decision` says what was decided, by
   whom, and when.

## Dependency updates

Dependabot proposes Cargo updates weekly, limited to patch and minor versions, and never merges
anything: there is no auto-merge in this repository, and none is to be added.

- `rocksdb` is excluded: it is held by the CI runner pin.
- Dependabot does not honour `rust-version`. A reviewer checks that an update keeps the
  workspace MSRV.
- `Cargo.lock` changes are code changes, and go through the normal review.

Dependabot security updates are enabled in the repository settings, and are reviewed the same way.

## Coverage

GitHub's dependency graph does not list `Cargo.lock` for this repository, so Dependabot alerts do
not cover Rust dependencies today. **This workflow is the only coverage of them.**

## Scope

`cargo-audit` reads `Cargo.lock`, which also lists optional dependencies that nothing compiles. Each entry's `scope` says what `cargo tree` found, across all targets:

| Scope | Meaning |
|---|---|
| `compiled-runtime` | Built into a workspace crate through normal dependencies |
| `compiled-build-or-test` | Built only for build scripts or tests |
| `feature-only` | Built only with a feature that is off by default |
| `lock-only` | In `Cargo.lock`, built by nothing |

`compiled-runtime` says a package is built, not that a defect in it can be reached. `lock-only` findings stay in the register: enabling a feature would make them real.

## Baseline

Recorded on 2026-11-06, from `dev` `4502d0c2`, with `cargo-audit` 0.22.2: 617 packages, 59
vulnerabilities, 15 unsound, 9 unmaintained, 1 yanked. All are `open`.

These figures are not current state: the register and the latest run are. No dependency was
changed to produce them.
