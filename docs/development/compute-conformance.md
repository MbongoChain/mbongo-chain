# Mbongo Compute Conformance

> **Status: CURRENT.** Version `compute-conformance-v1`. This suite tests
> implementations against the architecture contracts; it is not protocol
> authority and adds no requirement of its own. The contracts it tests are
> [`compute-privacy-data-plane.md`](../architecture/compute-privacy-data-plane.md)
> (P1–P18),
> [`compute-private-data-plane-interface.md`](../architecture/compute-private-data-plane-interface.md)
> (F1–F24) and
> [`compute-control-plane-worker-interface.md`](../architecture/compute-control-plane-worker-interface.md)
> (E1–E30), under [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md).
> Where they disagree with this page, they win.

## What it is

A named, executable contract: every implementation of the compute control
plane, private data plane and worker — the reference one today, a CPU, GPU
or inference worker, a persistent data plane or a distributed control plane
tomorrow — runs the same 38 cases and gets the same PASS/FAIL. The suite
lives in `crates/mbongo-compute/src/conformance/`; the reference
implementation's adapter is `conformance::reference::ReferenceSubject`; the
CI gate is the step **Mbongo Compute Conformance**, which runs
`cargo run -p mbongo-compute --bin compute_conformance` and fails on any
case that fails or cannot be driven.

It tests **behaviour**, through one adapter, in the contracts' vocabulary:
actors (the named executor, another executor, the client, a stranger),
tasks, leases, grants, presentations, outcomes. It never reaches into an
implementation's types. Nothing in it depends on the reverse-bytes profile,
on in-memory storage, on a single process, on a GPU vendor, on confidential
hardware or on a cloud.

```
CPU worker ─┐
GPU worker ─┼─► implements `Subject` ─► run_all ─► PASS / FAIL per case, per group
AI worker  ─┘
```

## What a pass means, and does not

A green run means the subject **behaves** as E and F require in every case
below. It does not mean the subject is confidential — an ordinary provider
sees plaintext during execution (P8), and `compute-conformance-v1` cannot
evaluate a confidentiality claim, so it **fails** a subject that makes one
(C37). It does not mean the subject's outputs are correct — an anchored
receipt is a bound claim, not a proof (P16), and the suite proves that
boundary by anchoring a receipt for an output nothing computed (C32). It
does not mean exactly-once — the suite fails a subject that claims it (C25).

Two facts the privacy cases state precisely. C28/C29 prove that the
**conformant flow** put neither the private input nor the private output on
the chain. They do not prove that an application cannot intentionally
publish plaintext in a public field: `execution_spec` may legally carry any
public bytes within the protocol bound (RFC 0005 §2.1), and C33 commits such
a task to show that **protocol permission** is not the same thing as the
**conformant private workflow**, which never uses that field as input
transport (P5) and never sources input from any chain payload (P15, C35).

## Groups and cases

Groups are engineering organisation, not certification tiers.

| Group | Cases | What they establish |
|---|---|---|
| CORE | C01, C03, C25, C31, C32, C37 | the named executor completes the flow; nothing off-chain can replace it, forge its signature, or claim more than the chain proves |
| PRIVACY | C28, C29, C33, C34, C35 | no private bytes on-chain; public fields are not transports; the worker never reads input from the payload |
| AUTHORIZATION | C02, C04–C17, C26, C27 | grants are scoped, proven, single-use, expirable, revocable and unreplayable; a lease is not a grant and a grant is not a lease; the result is released only to its owner |
| LIFECYCLE | C18, C19, C20, C21, C36 | lease → grant → fetch → verify → execute → persist → receipt, and nothing out of order |
| FAILURE_RECOVERY | C22, C23, C24, C38 | crash after fetch, stale attempt, duplicate attempt, ambiguous submission |
| OBSERVABILITY | C30 | diagnostics captured at runtime carry identifiers, never payloads or secrets |

| Id | Name | Invariants |
|---|---|---|
| C01 | `core_correct_executor_completes_flow` | E3, F6, G1, G48 |
| C02 | `auth_wrong_executor_cannot_execute` | E1, E3, F4, G1 |
| C03 | `core_control_plane_cannot_replace_executor` | E1, E2, E17, F20, G2 |
| C04 | `auth_task_id_is_not_a_capability` | F3, E24, G7 |
| C05 | `auth_locator_is_not_a_capability` | F9, G7 |
| C06 | `auth_capability_is_task_scoped` | F5, G8 |
| C07 | `auth_capability_is_resource_scoped` | F5, G9 |
| C08 | `auth_capability_is_executor_scoped` | F4, G10 |
| C09 | `auth_capability_requires_proof_of_possession` | F4, E3 |
| C10 | `auth_input_capability_is_single_use` | G11 |
| C11 | `auth_consumed_capability_replay_fails` | E10, G12, G14 |
| C12 | `auth_expired_capability_fails` | F10, G13 |
| C13 | `auth_revoked_capability_fails` | F10, G13 |
| C14 | `auth_cross_task_replay_fails` | F21, G14 |
| C15 | `auth_cross_executor_replay_fails` | F22, G14 |
| C16 | `auth_lease_alone_grants_no_data` | E6, G6 |
| C17 | `auth_capability_alone_grants_no_execution` | E6, E7, E8 |
| C18 | `lifecycle_commitment_verified_before_execution` | F6, F7, G15 |
| C19 | `lifecycle_corrupted_input_blocks_execution` | F7, G16 |
| C20 | `lifecycle_result_persisted_before_receipt` | E13, F23, G21 |
| C21 | `lifecycle_result_persistence_failure_blocks_receipt` | E13, F23, G22 |
| C22 | `recovery_crash_after_fetch_requires_fresh_capability` | E10, E11, G33 |
| C23 | `recovery_stale_attempt_is_fenced` | E8, G34 |
| C24 | `recovery_duplicate_execution_does_not_overwrite_result` | E12, G32 |
| C25 | `core_exactly_once_is_not_claimed` | E12, G32 |
| C26 | `auth_task_id_alone_cannot_retrieve_result` | F15, G24 |
| C27 | `auth_unauthorized_result_retrieval_fails` | F15, G23 |
| C28 | `privacy_raw_input_absent_from_chain` | P1, P2, P4, P5, F1, G17 |
| C29 | `privacy_raw_output_absent_from_chain` | P1, P2, P4, F2, G18 |
| C30 | `observability_diagnostics_omit_payloads_and_secrets` | F19, E23, G37 |
| C31 | `core_control_plane_cannot_forge_executor_signature` | E17, G28, G29, G30 |
| C32 | `core_anchored_receipt_is_not_output_correctness` | P16, F16, E19, G47 |
| C33 | `privacy_execution_spec_is_not_input_transport` | P5, P12 |
| C34 | `privacy_receipt_metadata_is_not_output_transport` | P4, P12 |
| C35 | `privacy_worker_never_sources_input_from_chain_payload` | P15, P5 |
| C36 | `lifecycle_lease_precedes_capability` | E6, E7, G6 |
| C37 | `core_no_confidentiality_claim_without_attestation` | P8, P9, F12, F13, E20, E21, G19, G20 |
| C38 | `recovery_ambiguous_submission_yields_one_receipt` | E14, G31 |

G-numbers are the reference-worker invariants recorded on Epic #126 for
Workstream G; P, F and E numbers are the architecture documents' own.

## Workstream I traceability

| Invariant | Exact authority | Cases |
|---|---|---|
| P4 | "`Receipt.metadata` is public and is not a private-data transport." — privacy architecture §16 | C34 (metadata carries neither input nor output), C28, C29 (metadata scanned as part of the chain) |
| P5 | "`ComputeTask.execution_spec` is public and is not a private-data transport." — privacy architecture §16 | C33 (the conformant task's spec carries no input; protocol permission distinguished from the workflow), C28, C35 |
| P15 | "Worker input is never sourced from public blockchain payload fields." — privacy architecture §16, restated §11: "A worker must not obtain customer input from blockchain payload fields" | C35 (a task whose only "input" is its `execution_spec` is never executed, produces no result and no receipt), C33 |
| F19 | "logs must not contain raw private payload or reusable secrets by default" — F §18, rules in F §13 | C30 (runtime-captured log output plus every `Debug` rendering the scenario touched: no input, no output, no private key, no grant signature, no possession proof, in raw or hex form; the task id is present) |

E23 (logs contain no payloads, keys or reusable capability secrets) is F19
inherited by the control plane and is tested by the same case.

## How a future implementation runs it

Implement `mbongo_compute::conformance::Subject` for your system and call
`run_all`:

```rust
use mbongo_compute::conformance::{run_all, Subject};

struct MyWorkerSubject { /* your control plane, data plane, worker, chain, clock */ }
impl Subject for MyWorkerSubject { /* ~45 thin methods in E/F vocabulary */ }

let report = run_all(MyWorkerSubject::new).await;
print!("{}", report.render());
assert!(report.passed());
```

The adapter contract, in E/F terms:

- **Scenario.** A fresh subject is one client, one private input stored in
  your data plane, and one task naming `Actor::Executor`, committed by
  `commit_task`. The suite makes a new subject per case. You must also be
  able to commit a second task whose `execution_spec` carries the input
  bytes with **no** private object behind it, without special-casing it
  (C33, C35).
- **Actors.** You hold every key. The suite names roles: the executor,
  another executor, the client, a stranger. "Present as X" means X proves
  possession with its own key, however your data plane defines proof.
- **Opaque handles.** `Grant`, `Lease`, `Locator` and `Instance` are your
  types. The suite only clones, compares and hands them back — plus
  `retarget`, which returns the same grant with one binding changed and
  nothing re-authorised, so the suite can present a forgery.
- **Chain.** Anything implementing `ChainClient`. The suite scans blocks
  itself (SCALE bytes) for private data and receipts; `produce_block` may
  mean "include pending" on a double or "wait for one" on a node.
- **Time.** `advance_time` plus `lease_ttl_secs` / `grant_ttl_secs`. The
  reference uses an injected manual clock; a real system may sleep. The
  suite advances only past the lifetime it means to expire.
- **Faults.** `inject` (six worker fault points), a result-store failure, a
  lost submission response. A subject that cannot inject a fault returns
  `false`; the case is reported **UNSUPPORTED**, which fails its group — a
  required invariant that cannot be driven is not passed.
- **Claims.** `info().claims`: `exactly_once` must be `false`;
  `confidential_execution` must be `false` in this suite version.
- **Diagnostics.** `diagnostics()` returns what an operator would see:
  captured log output and debug renderings; `secrets()` lists the byte
  strings that must never appear (private keys, grant signatures, proofs,
  content keys). The reference captures `log` output at runtime.

Nothing requires copying the reference worker, the reverse-bytes profile,
Rust for the implementation under test (a non-Rust system is driven through
an adapter that crosses the process or network boundary; the assertions do
not depend on Rust semantics), in-memory storage, or any vendor. GPU- or
AI-specific suites, and metering, are later additions alongside this one,
not changes to it.

## Versioning

`compute-conformance-v1` tests the authorities listed at the top, as
merged. A new version is needed when an authority document changes an
invariant the suite tests, when a mandatory case is added or its meaning
changes, or when the adapter contract changes incompatibly. Adding a case
that only tightens an existing invariant, or a subject, does not. The
version is implementation-level: it is not a protocol lock, an RFC or a
consensus surface.

## Running it here

```bash
cargo run -p mbongo-compute --bin compute_conformance
```

prints the report and exits non-zero on failure; `cargo test -p
mbongo-compute --test conformance` runs the same suite under the test
harness and checks the catalog, and `--test conformance_negative` proves the
suite is not vacuous: a deliberately misbehaving adapter fails exactly the
cases it should.
