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

## Remediation plan

Each step is its own pull request, and removes its entries from the register. Nothing here accepts a finding.

### Priority 1: `rustls` and `rustls-webpki`

`rustls` 0.23.35 to 0.23.46, and `rustls-webpki` 0.103.8 to 0.103.16. Both keep the 0.23 and 0.103 lines and declare Rust 1.71, below the workspace MSRV of 1.75. The change is two versions in `Cargo.lock`.

| Advisory | Package | Reachability here |
|---|---|---|
| RUSTSEC-2026-0285 | `rustls` | On the path used: TLS 1.3 client |
| RUSTSEC-2026-0349 | `rustls` | On the path used: key exchange on later connections. Not assessed further |
| RUSTSEC-2026-0346 | `rustls` | Needs a session value printed with `Debug`. None found in `provider/` |
| RUSTSEC-2026-0348 | `rustls` | Not reachable: needs `brotli`, which is not in `Cargo.lock` |
| RUSTSEC-2026-0382, 0098, 0099 | `rustls-webpki` | Certificate verification. Narrowed, not removed, by the operator's own roots |
| RUSTSEC-2026-0049, 0104 | `rustls-webpki` | Not reached by the provider client: it configures no CRL |

**Trust model:** unchanged. The provider client keeps TLS 1.3 only, its operator-supplied roots through `with_root_certificates`, and no client authentication. No source file changes. `provider_transport`, the provider harness and the containment job are the regression evidence, on Windows and on Linux.

`rustls-webpki` 0.101.7 is `lock-only`: nothing compiles it.

### Priority 2: `wasmtime`

`mbongo-runtime` declares `wasmtime` and never uses it: its source has no `wasmtime` item, and `pub mod wasm` is commented out. Removing the declaration from two manifests drops 72 packages from `Cargo.lock`, adds none, and clears 37 findings, 25 of them vulnerabilities: `wasmtime`, `wasmtime-jit-debug`, `zstd-safe`, `zstd-sys`, `bincode`, `fxhash`, `mach`.

### Priority 3: the rest

| Group | Packages | Option |
|---|---|---|
| Patch available in the same line | `bytes`, `crossbeam-epoch`, `h2` 0.4, `libz-sys`, `webpki-roots` | `cargo update --precise`, one pull request, after checking each patched version stays compatible |
| Test or build only | `time`, `ring` 0.16, `zip`, `webpki-roots` | Through `rcgen` and build scripts. Lower priority, still open |
| Held by `libp2p` 0.53 | `hickory-proto`, `core2`, `instant`, `lru`, and others | Needs a `libp2p` upgrade, which is planned work. `hickory-proto` needs mDNS: `--no-mdns` narrows it |
| No fixed version | `rsa` | Timing side channel. Whether private-key operations occur here is not assessed |
| Not compiled | `quinn-proto`, `rustls-webpki` 0.101.7, `h2` 0.3 | Nothing to do unless a feature enables them |
| Unmaintained or unsound, no action yet | `anyhow`, `paste`, `pin-utils`, `proc-macro-error`, `rand`, `tokio` | Assess each advisory's condition against how the code uses it |

These groups are a first reading from dependency paths, not assessments. Each finding stays `open` until one is recorded.

## Baseline

Recorded on 2026-11-06, from `dev` `4502d0c2`, with `cargo-audit` 0.22.2: 617 packages, 59
vulnerabilities, 15 unsound, 9 unmaintained, 1 yanked. All are `open`.

These figures are not current state: the register and the latest run are. No dependency was
changed to produce them.
