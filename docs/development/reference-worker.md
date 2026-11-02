# The reference compute worker

**Status:** CURRENT — describes what `crates/mbongo-compute` does today.
**Authority it implements:** [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md) (consensus, unchanged), [compute-control-plane-worker-interface.md](../architecture/compute-control-plane-worker-interface.md) (E) and [compute-private-data-plane-interface.md](../architecture/compute-private-data-plane-interface.md) (F). Nothing in this document or in the crate is protocol authority; where the crate chose something E or F left open, it is an implementation policy and says so.

## What it proves

A client can submit a `ComputeTask` whose private input stays off-chain; the executor the client named can obtain that input through a scoped, single-use, off-chain authorization, verify its commitment, run a deterministic transform, persist the private result, and anchor an RFC 0005-bound `Receipt` — without the chain acting as scheduler or as private-data transport.

The crate is a **reference implementation**: one execution profile, one in-memory data plane, one in-process control plane. It is not a product, a marketplace, a scheduler, or confidential compute.

## Running it

The live harness spawns a producer node and plays every role in process — client, control plane, worker, client again — against the real chain:

```bash
cargo build -p mbongo-node
cargo run -p mbongo-compute --bin compute_harness
```

It prints one line per phase and exits 0 on `REFERENCE COMPUTE FLOW: PASS`. CI runs it on every pull request after the replay harness. It never prints private bytes or keys.

The operational vertical — a fresh three-node devnet, the task submitted and the receipt read back through the public SDK, the negative cases, restarts and privacy scans — is the **Mbongo Compute v0.4 Vertical Harness**, `cargo run -p mbongo-node --bin compute_vertical`, described in [`DEVNET_V0.4_OPERATIONS.md`](../runbooks/DEVNET_V0.4_OPERATIONS.md).

The deterministic suite, which drives every fault the contracts name against an in-memory chain double:

```bash
cargo test -p mbongo-compute
```

## The four planes

| Component | Module | Authority for | Never |
|---|---|---|---|
| chain | `chain.rs` | task existence, `task.executor`, `input_commitment`; whether a receipt is anchored; account nonces | scheduling; private data |
| control plane | `control_plane.rs` | sessions, leases, attempts — coordination | the executor key; choosing the executor |
| private data plane | `data_plane.rs` | objects, capabilities, consumption; whether a result exists | on-chain state; deciding validity |
| worker | `worker.rs`, `execution.rs` | its own execution; the executor key it holds | reading input from the chain; claiming correctness |

## The lifecycle, as implemented (E §7)

```
client:   store private input in the data plane  →  register task executor
          →  commit ComputeTask (RFC 0005)        →  register input reference with the control plane
control:  observe the task in a block (get_block_by_height; confirmation depth 1)
worker:   open session (proof of possession over a challenge)
control:  offer → lease { lease_id, task_id, executor, worker_instance, attempt_id, not_after, issuer sig }
control:  authorize_fetch → single-use capability { task, executor, FetchInput, object, expiry, issuer sig }
worker:   present capability with proof of possession over a fresh challenge → data plane consumes it
worker:   verify input against input_commitment → refuse on mismatch (no execution, no result, no receipt)
worker:   execute the reference profile
control:  authorize_put → single-use capability
worker:   persist result → data plane confirms durability → ONLY THEN build the bound receipt
worker:   sign receipt with the executor key; sign and submit AnchorReceipt as the executor
chain:    rules (a)–(j), (q)–(s)
client:   retrieve the private result under a get-result grant; verify against output_commitment
```

The order **lease → capability → fetch → start** is enforced: the control plane issues capabilities only under a live lease, and the worker verifies before it starts.

## REFERENCE / TEST EXECUTION PROFILE

`ReverseBytesProfile` reverses the input bytes. It is selected by an `execution_spec` equal to the ASCII tag `mbongo-ref:reverse-bytes:v1`. This is a **reference-worker implementation convention**: the chain treats `execution_spec` as opaque bytes and enforces nothing about it. The profile exists to prove the lifecycle; it is not the production compute model.

Commitments follow the RFC 0005 §2.4 interoperability convention, `BLAKE3("mbongo:compute-input:v1" || bytes)` and `BLAKE3("mbongo:compute-output:v1" || bytes)`, over the raw bytes. The chain compares commitments for equality and never learns the derivation; a client that blinds its commitment is equally valid on-chain and agrees a different convention with its worker. RFC 0005 defines no application canonicalization, and neither does this crate.

## What is public and what is private

| On-chain (public, permanent) | Off-chain (private, deletable) |
|---|---|
| `ComputeTask`: submitter, executor, salt, `input_commitment`, `execution_spec` (the profile tag) | the input bytes |
| `Receipt`: `task_id`, `input_commitment`, `output_commitment`, executor, empty metadata, signature | the result bytes |
| — | capabilities, challenges, proofs, leases, attempts, sessions |

The harness proves the input and result bytes appear in no block. `execution_spec` and `metadata` carry no payload in the reference flow; if an application chooses to put bytes there, they are public chain data.

## Identities and grants

- **Executor identity** — the `Address` a task names. Its Ed25519 key is held by the worker process as a 32-byte seed supplied at start (`ExecutorKey`). It signs receipts, anchoring transactions and possession proofs. It is never serialized, logged or handed to the control plane, which can relay a signature and cannot produce one.
- **Worker instance, session, attempt, lease, capability** — opaque 32-byte identifiers, off-chain only, never in a task or receipt.
- **Execution lease** — coordination only; grants no bytes. Short (60 s in the harness), renewed by heartbeat, expired at `not_after`, revoked with its capabilities when superseded.
- **Data-plane capability** — a signed grant binding task, presenter, one operation, one object, a window and a use count; presented with proof of possession over a data-plane challenge; single-use for fetch and put; refused when expired, revoked, consumed, mis-scoped, wrongly presented or wrongly issued. A consumed grant is never reopened.

A `task_id` is an identifier. No method takes a bare `task_id` as authorization.

## Trust assumptions

**Worker / provider.** Ordinary execution: the process holds input and output plaintext in memory, overwritten on drop as best effort (`Plaintext`). No provider confidentiality is claimed. A worker under the correct executor key can see plaintext, lie about progress, fabricate output, or retain data; the chain and this crate do not detect that. An anchored receipt is a **bound claim** — this executor, this task, this committed input, this output commitment — not a verified result.

**Control plane.** Trusted for coordination, not for authority. It cannot change `task.executor` (it has no such field, and the data plane checks the client-registered executor independently), cannot authorise another executor (the data plane refuses), cannot forge a possession proof or a receipt (no executor key), and cannot anchor as the executor. It can deny, delay or withhold work.

**Data plane.** Trusted to enforce grants and to remember consumption and results. In memory: **restart is not survivable** and is out of scope for the first worker. A durable backend must keep the same invariants.

**Client.** Registers its task's executor and delegates issuance to the control plane's service key. It could register wrongly; the chain-committed executor is the authority a production data plane should prefer (F §5.1).

## Crash and retry

| Crash | What survives | Next attempt |
|---|---|---|
| before fetch | nothing consumed | lease expires; capability revoked; new lease, new capability |
| after fetch | capability consumed | new lease, **fresh** capability; the old is never reopened |
| during execution | capability consumed | as above; the same executor retries — no reassignment |
| after result persisted | result durable in the data plane | new lease; result **reused, not recomputed**; the receipt is byte-identical |
| after receipt submitted | transaction pending or in a block | **lookup first** (scan for the receipt); resubmit the same signed bytes only if absent; never a second receipt |
| control-plane restart | tasks and attempt counters (`DurableState`) | leases forgotten → expired; sessions re-authenticate; data plane remembers consumption |

Duplicate execution: **effectively-once under healthy coordination, at-least-once under failure**. One live lease per task; the data plane keeps one result per (task, executor); the chain settles a second anchor by first-anchored-wins. No exactly-once claim is made. An instance that still holds a live lease is handed that lease again, not a new attempt; every pass begins with the lookup.

## Confirmation depth

The control plane offers a task once one block exists above the one carrying it (`confirmation_depth: 1`). This is an implementation policy for the current single-producer devnet, not protocol finality.

## Logging

Logs carry identifiers and outcomes: task ids, executor addresses, instance, session, attempt, lease and capability ids, state transitions, failure classes. They never carry input, output, keys, capability grants or proofs; `Debug` on the relevant types redacts them, and a test asserts it.

## Confidential-compute extension point

`execution::ConfidentialExtension` names the one step a confidential profile changes — the content key released only to an attested environment, the output encrypted inside it. Workstream K implements that step beside the worker, in `confidential/` ([`compute-confidential-attestation-key-release.md`](../architecture/compute-confidential-attestation-key-release.md)): a client seals the input under a content key and registers the object as confidential; the control plane's release authority issues a challenge, verifies attestation evidence, runs the provider-policy evaluator and mints a single-use release; only then does `authorize_fetch_confidential` issue the fetch capability and the key service release the content key wrapped to the environment key the evidence bound. The ordinary `authorize_fetch` refuses a confidential object. The attestation path is **REFERENCE_ATTESTED — test only, not hardware attestation**; the worker's own `run_once` path is the ordinary profile, unchanged, and TEE is not required.

## Conformance

This implementation passes [Mbongo Compute Conformance](compute-conformance.md) (`compute-conformance-v1`, 38 cases) through its adapter `conformance::reference::ReferenceSubject`; CI runs it as the step *Mbongo Compute Conformance*. The suite, not this document, is the reusable statement of what a future implementation must do.

## Limitations

- In-memory data plane and in-process control plane; no persistence across process restart except the control plane's `DurableState` snapshot.
- One execution profile; one client, one executor per harness run.
- No vendor, cloud, KMS or attestation dependency, by design.
- The live harness is Rust end to end; the TypeScript SDK's client role was proven separately in Workstream D against the same node.
