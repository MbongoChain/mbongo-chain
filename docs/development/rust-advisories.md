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
- Dependabot does not honour `rust-version`. A reviewer checks what an update requires, against
  [Rust version](#rust-version).
- `Cargo.lock` changes are code changes, and go through the normal review.

Dependabot security updates are enabled in the repository settings, and are reviewed the same way.

## Rust version

| Source | Version |
|---|---|
| `rust-version` in the workspace `Cargo.toml` | 1.85 |
| Checked by CI, job `Rust 1.85 (minimum supported version)` | 1.85.0, Linux x86_64, `cargo check` of all targets, with default and with all features |
| Tested by CI | 1.94.0 |

Until 2026-11-07 the workspace declared 1.75 while nothing built on it: 1.75.0 and 1.84.0 failed,
1.85.0 passed, because of the versions in `Cargo.lock`. Raising the declaration also turned on
Clippy lints that were held back for 1.75, and their fixes are behaviour-neutral.

**What 1.85 means here:** the workspace compiles on it, on Linux. Tests, Windows and other
platforms are proven on 1.94.0 only. A dependency update that needs more than 1.85 now fails CI,
and moving the minimum is a maintainer decision.

## Coverage

Two sources cover Rust dependencies, and they differ.

- `cargo-audit` reads the RustSec database, against every package in `Cargo.lock`.
- Dependabot alerts read GitHub's advisory database. They began to cover Rust once `.github/dependabot.yml` configured Cargo, although `Cargo.lock` is still not listed among the dependency graph's manifests.

Neither is a subset of the other: GitHub reports advisories on `openssl` that RustSec does not list, and RustSec lists advisories GitHub does not report here. The register follows RustSec. Dependabot alerts are read in the repository's Security tab.

## Scope

`cargo-audit` reads `Cargo.lock`, which also lists optional dependencies that nothing compiles. Each entry's `scope` says what `cargo tree` found, across all targets:

| Scope | Meaning |
|---|---|
| `compiled-runtime` | Built into a workspace crate through normal dependencies |
| `compiled-build-or-test` | Built only for build scripts or tests |
| `feature-only` | Built only with a feature that is off by default |
| `lock-only` | In `Cargo.lock`, built by nothing |

`compiled-runtime` says a package is built, not that a defect in it can be reached. `lock-only` findings stay in the register: enabling a feature would make them real.

Entries recorded from 2026-11-22 also carry four informational fields, which the check does not read:

| Field | Content |
|---|---|
| `patched` | The patched version ranges of the RustSec advisory, as published. Empty when none is published |
| `unaffected` | Its unaffected ranges |
| `aliases` | Its CVE and GHSA identifiers |
| `path` | How `cargo tree` reaches the package from the workspace, or what lists it when nothing compiles it |

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

`mbongo-runtime` declares `wasmtime` and never uses it: its source has no `wasmtime` item, and `pub mod wasm` is commented out. Removing the declaration from two manifests drops 72 packages from `Cargo.lock`, adds none, and clears 36 findings, 24 of them vulnerabilities: `wasmtime`, `wasmtime-jit-debug`, `zstd-safe`, `bincode`, `fxhash`, `mach`. `zstd-sys` stays, through `rocksdb`.

### Priority 3: the rest

| Group | Packages | Option |
|---|---|---|
| Patch available in the same line | `openssl` 0.10.75, 8 GitHub alerts, none in RustSec, fixed by 0.10.80, `bytes`, `crossbeam-epoch`, `h2` 0.4, `libz-sys`, `webpki-roots` | `cargo update --precise`, one pull request, after checking each patched version stays compatible |
| Test or build only | `time`, `ring` 0.16, `zip`, `webpki-roots` | Through `rcgen` and build scripts. Lower priority, still open |
| Held by `libp2p` 0.53 | `hickory-proto`, `core2`, `instant`, `lru`, and others | Needs a `libp2p` upgrade, which is planned work. `hickory-proto` needs mDNS: `--no-mdns` narrows it |
| No fixed version | `rsa` | Timing side channel. Whether private-key operations occur here is not assessed |
| Not compiled | `quinn-proto`, `rustls-webpki` 0.101.7, `h2` 0.3 | Nothing to do unless a feature enables them |
| Unmaintained or unsound, no action yet | `anyhow`, `paste`, `pin-utils`, `proc-macro-error`, `rand`, `tokio` | Assess each advisory's condition against how the code uses it |

These groups are a first reading from dependency paths, not assessments. Each finding stays `open` until one is recorded.

## Recorded 2026-11-22

The scheduled run of 2026-11-16 on `dev` (`90f3350b`) failed with 15 findings missing from the register. The run of 2026-11-22 on pull request #274 failed with the same 15 and 5 more. All 20 were published to the RustSec database after the register was last updated. The lock was not changed to produce them: `dev` `85818434` and that pull request hold the same 542 packages at the same versions. The 20 were recorded from database commit `8b75c83c`, with `cargo-audit` 0.22.2. All are `open`, and none is accepted.

| Package | Advisories | Scope | Patched |
|---|---|---|---|
| `hickory-proto` 0.24.4 | RUSTSEC-2026-0487, 0489, 0490, 0491 | `compiled-runtime`, through mDNS | `>= 0.26.2` |
| `hickory-proto` 0.24.4 | RUSTSEC-2026-0505, 0511, 0512, 0530, 0531, 0550, 0551 (DNSSEC validation) | `compiled-runtime`, through mDNS | none published; unaffected from `0.26.0-beta.1` |
| `url` 2.5.7 | RUSTSEC-2026-0592 | `compiled-runtime` | `>= 2.5.9` |
| `rustix` 1.1.2 | RUSTSEC-2026-0486 (unsound) | `compiled-runtime`, Unix targets | `>= 1.1.6` |
| `zip` 1.1.4 | RUSTSEC-2026-0545, 0548 | `compiled-build-or-test`, the build script of `utoipa-swagger-ui` | `>= 9.0.0`, `>= 9.0.5` |
| `hickory-resolver` 0.24.4 | RUSTSEC-2026-0514 | `lock-only` | `>= 0.26.2` |
| `libp2p-quic` 0.10.3 | RUSTSEC-2026-0525 | `lock-only` | `>= 0.13.1` |
| `quinn-proto` 0.11.13 | RUSTSEC-2026-0523, 0524 | `lock-only` | `>= 0.11.19` |
| `quinn-udp` 0.5.14 | RUSTSEC-2026-0538 | `lock-only` | `>= 0.5.16, < 0.6.0`, `>= 0.6.3` |

The `hickory-proto` entries join RUSTSEC-2026-0119 under "Held by `libp2p` 0.53" in the remediation plan. `url` and `rustix` have patches in the same line. No reachability is assessed here.

## Baseline

Recorded on 2026-11-06, from `dev` `4502d0c2`, with `cargo-audit` 0.22.2: 617 packages, 59
vulnerabilities, 15 unsound, 9 unmaintained, 1 yanked. All are `open`.

These figures are not current state: the register and the latest run are. No dependency was
changed to produce them.
