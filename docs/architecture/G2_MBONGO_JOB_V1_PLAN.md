# G2 — `mbongo-job:v1` Implementation Plan

| | |
|---|---|
| **Status** | **PLAN, for approval.** No G2 product code exists, and none may be written until the maintainer gives a separate implementation approval. |
| **Governing scope** | Infrastructure first: decision D15, [`GENERAL_PURPOSE_COMPUTE_PROPOSAL.md`](GENERAL_PURPOSE_COMPUTE_PROPOSAL.md) §1.4. Where this plan and the proposal's §4.11 or §7 G2 differ, this plan wins. |
| **Authority** | RFC 0005, the v0.4 protocol lock and every NORMATIVE document win on their subjects. |
| **Kind** | Non-consensus. No protocol, consensus, RPC, SDK or released-MVP change. |

## 1. Scope

G2 delivers one reusable thing: **a generic way to describe a compute job, validate it, and hand it to a provider's runtime, through the existing task, receipt and dispatch primitives.**

| Owner | Responsible for |
|---|---|
| **MbongoChain** | The `mbongo-job:v1` format; validation; the provider runtime interface; dispatch; failure codes; receipts and technical verification; conformance tests |
| **Clients** | What the job is: their program or model, prompts, schemas, evaluation rules, data, and whether a result is right for their purpose |
| **Providers** | Where it runs: hardware, runtime installation, artifact provisioning, operating the worker, implementing the runtime interface |

**G2 does not include:**
- any business workload: no credit scoring, CV extraction, logistics or optimisation logic;
- customer-specific prompts, schemas or evaluation rules;
- any model integration, ML or GPU execution;
- moving Kaayu. `mbongo-infer:v1` and `kaayu-cv-extract` v1–v3 stay exactly as released;
- the MVP front, CLI, `mvp_service`, SDK or chain;
- capability discovery, T2 reports, registries, open assignment, pricing, settlement;
- benchmarking campaigns. G2 acceptance is correctness only.

## 2. What changed from the previous revision of this plan

| Previous revision | Now | Why |
|---|---|---|
| `mbongopay-scorecard:v1` as a workload, with a synthetic generator and at least 200 cases | Two test-only **conformance fixtures** (§3.5) and a small committed vector set | Fixtures test the infrastructure; they are not workloads |
| `WorkloadContract`, a Rust trait implemented per workload | **The contract is data in the job spec** (§3.1) | A compiled trait would need MbongoChain code for every client workload |
| `mbongo-native` adapter in the worker | No production adapter. A **mock worker** in tests exercises the interface | MbongoChain does not ship a workload runtime without a generic need |
| `json-schema` and `numeric-record` outputs | `bytes` and `canonical-json` only | The other types wait for a demonstrated need |
| Five slices | Slice 1, then Slice 2 (§5) | Less speculative work |

## 3. Design

### 3.1 The contract is data

A job is fully described by a canonical job spec. The worker needs no knowledge of the workload. `workload` is an opaque client label that MbongoChain never interprets.

| Field | Purpose |
|---|---|
| `workload` | Opaque client label |
| `runtime` | `{family, revision, binary_sha256?, accelerator_class}`: selects and pins the provider's adapter |
| `artifacts[]` | `{role, sha256, size_bytes}`: programs, weights, tables. Verified before work is accepted |
| `input`, `output` | `{type, max_bytes}` |
| `limits` | `max_wall_ms`, `max_memory_mib` |
| `determinism` | `BIT_EXACT`, `BIT_EXACT_PINNED`, `TOLERANCE`, `NON_DETERMINISTIC` |
| `confidentiality` | The classes the job may run under |
| `params` | Opaque to MbongoChain; passed to the adapter |

- **`execution_spec`:** `b"mbongo-job:v1"` ‖ the 32-byte digest, 45 bytes in total.
- **Digest:** `BLAKE3(b"mbongo:job-spec:v1" || canonical_bytes)`.
- **Canonical form:** the existing rules in `inference/canonical.rs`, reused unchanged. They allow only non-negative integers, so a future float output needs fixed-point integers or strings.

### 3.2 Provider runtime interface

```rust
/// Implemented by providers. MbongoChain ships no production implementation in G2.
pub trait RuntimeAdapter: Send + Sync {
    /// Family, revision, binary hash (if any), accelerator class.
    fn identity(&self) -> RuntimeIdentity;
    /// Runs one job on verified artifacts. Errors are content-free codes.
    fn run(&self, job: &ResolvedJob, input: &[u8]) -> Result<RunOutput, JobError>;
}

pub struct RunOutput {
    pub raw: Vec<u8>,
    /// Wall time, CPU time, peak memory. Untrusted; stays private (D14).
    pub metering: UntrustedMetering,
}
```

The worker, not the adapter, verifies artifacts, so a provider cannot skip the check by mistake.

`JobProfile` implements the existing `ExecutionProfile` trait and sits beside the existing profiles in `ProfileDispatch`. Neither is modified. `JobError` is new, because `inference::ContractError` is on a protected path; it renders to a content-free code inside `ExecutionError::Failed`, so the worker and control plane do not change.

### 3.3 Pipeline

The first failure stops the task, with its named code.

1. Parse `execution_spec`: tag and length.
2. Resolve the job spec by digest and recompute the digest.
3. Validate the spec.
4. Select the adapter by `runtime`.
5. Verify artifacts: size, then hash.
6. Check the input commitment and size.
7. `adapter.run`, under the limits.
8. Canonicalise and check the output by its declared type and size (T1).
9. Hand metering to the private side and return the bytes.

### 3.4 Failure classes

One code per failure; every message is content-free. A failed task has no receipt, as today.

| Class | Codes | Retry | Blame |
|---|---|---|---|
| `SPEC` | `JOB_SPEC_MALFORMED_EXECUTION_SPEC`, `JOB_SPEC_NOT_FOUND`, `JOB_SPEC_DIGEST_MISMATCH`, `JOB_SPEC_INVALID` | No | Client |
| `RUNTIME_SELECTION` | `JOB_RUNTIME_UNKNOWN`, `JOB_RUNTIME_MISMATCH` | No on this worker | Provider or client |
| `ARTIFACT` | `JOB_ARTIFACT_NOT_FOUND`, `JOB_ARTIFACT_DIGEST_MISMATCH`, `JOB_ARTIFACT_SIZE_MISMATCH` | No | Provider |
| `INPUT` | `JOB_INPUT_COMMITMENT_MISMATCH`, `JOB_INPUT_TOO_LARGE` | No | Client |
| `LIMIT` | `JOB_LIMIT_WALL_TIME`, `JOB_LIMIT_OUTPUT_SIZE` | No | Client or provider |
| `EXECUTION` | `JOB_RUNTIME_FAILED` | Yes, within the existing attempts | Provider |
| `OUTPUT` | `JOB_OUTPUT_NOT_CANONICAL`, `JOB_OUTPUT_TYPE_VIOLATION` | No | Provider |

### 3.5 Conformance fixtures

Fixtures exist only under `tests/` and `test-vectors/job/`. **No shipped binary contains them, and they never gain business logic.** Each is a few lines, checkable by hand and by a short Node script.

| Fixture | Does | Exercises |
|---|---|---|
| `fixture-byte-table` | Maps each input byte through a 256-byte table artifact | Artifact integrity, `bytes` output, `BIT_EXACT` |
| `fixture-sum` | Reads a canonical JSON list of integers; returns `{"count", "sum"}` | `canonical-json` output, replay |
| `fixture-future-runtime` | A mock adapter declaring `accelerator_class = "gpu:test"` and `TOLERANCE`. It computes nothing real | That a differently shaped runtime fits with no change to `job/` |

## 4. Protected surfaces

**G2 adds new files only**, plus one line in `crates/mbongo-compute/src/lib.rs` (`pub mod job;`). No new crate dependency.

```
crates/mbongo-compute/src/job/            spec, runtime, error, output, profile
crates/mbongo-compute/tests/job_contract.rs
test-vectors/job/
scripts/job-vectors.test.mjs
docs/architecture/compute-job-contract-v1.md
```

**Proved unchanged with Git objects at merge:**
- `crates/mbongo-core/**`, `crates/mbongo-node/**`, `crates/mbongo-network/**`, `crates/mbongo-storage/**`;
- `crates/mbongo-compute/src/{inference,profiles,cpu_inference,mvp}/**`, `execution.rs`;
- `sdk/typescript/**`, `docs/specs/**`, `docs/rfcs/**`;
- `test-vectors/inference/**`, `test-vectors/mvp/**`;
- the `mvp-v0.1.0` tag.

**Rollback.** Stop before merge if a protected path differs, an existing vector changes, or the work needs business logic. After merge, revert the squash commit if an MVP regression is traced to G2; G2 is additive, so that removes only new files.

## 5. Implementation tasks

Each task states the five items of D15.

### Slice 1

**Task 1 — Job contract and validation**

| | |
|---|---|
| Capability | Any client can describe any job in one validated, digest-bound format |
| MbongoChain owns | `compute-job-contract-v1.md`, `job/spec.rs`, `job/error.rs`, vectors |
| Clients or providers own | The job's content; `workload` and `params` stay opaque |
| Minimal tests | JS-1 to JS-5, FC-1, FC-2 |
| Excluded | Schema languages, a contract registry, SDK helpers |

**Task 2 — Provider runtime interface and dispatch**

| | |
|---|---|
| Capability | Any provider runtime plugs in behind one interface, with artifact checks and limits applied by the worker |
| MbongoChain owns | `job/runtime.rs`, `job/profile.rs`, `job/output.rs` |
| Clients or providers own | Implementing the adapter, installing the runtime, supplying artifacts |
| Minimal tests | DS-1 to DS-4, AI-1 to AI-3, CO-1, CO-2, RL-1 to RL-4 |
| Excluded | Any production adapter; capability discovery; a process boundary (decision 1, §7) |

**Task 3 — Mock worker and fixtures**

| | |
|---|---|
| Capability | The whole path is exercised without any real workload |
| MbongoChain owns | Test-only mock worker and the §3.5 fixtures |
| Clients or providers own | Nothing: fixtures are not offered to anyone |
| Minimal tests | DR-1, DR-2, TV-1, TV-2, PE-1, PE-2 |
| Excluded | Business logic, benchmarks, large synthetic sets |

**Task 4 — Compatibility tests**

| | |
|---|---|
| Capability | Proof that G2 changes nothing already released, and that a non-Rust client can compute the same digests |
| MbongoChain owns | `job_contract.rs` isolation tests, `job-vectors.test.mjs` |
| Clients or providers own | Nothing |
| Minimal tests | XI-1 to XI-4, JS-1 in Node |
| Excluded | SDK changes |

### Slice 2 (separate approval)

**Task 5 — Provider conformance and end-to-end receipts**

| | |
|---|---|
| Capability | A provider can test its worker against a named suite, and a job reaches an anchored receipt |
| MbongoChain owns | An extension of `compute-conformance-v1` through its existing `Subject` adapter; one devnet test |
| Clients or providers own | Running the suite against their worker |
| Minimal tests | TV-3, PE-3, FC-3 |
| Excluded | Certifying providers; capability registration (M4) |

## 6. Minimal acceptance tests

Each test states its expected count and asserts it first. A negative test passes only by reaching its **named code**.

| Category | ID | Test | Expected |
|---|---|---|---|
| **Job specification** | JS-1 | Vectors: canonical bytes and digest, in Rust and Node | Equal |
| | JS-2 | Bad tag or length | `JOB_SPEC_MALFORMED_EXECUTION_SPEC` |
| | JS-3 | Non-canonical spec; each required field removed; unknown field | `JOB_SPEC_INVALID`; never normalised silently |
| | JS-4 | Bytes that do not match the digest | `JOB_SPEC_DIGEST_MISMATCH` |
| | JS-5 | `mbongo-job` and `mbongo-infer` domains over identical bytes | Different digests |
| **Dispatch** | DS-1 | `ProfileDispatch` with both profiles: each spec reaches its own | Proven by a spy |
| | DS-2 | Unknown tag | `ExecutionError::UnsupportedSpec`, as today |
| | DS-3 | Unknown runtime; wrong revision or accelerator | `JOB_RUNTIME_UNKNOWN`; `JOB_RUNTIME_MISMATCH`, adapter not called |
| | DS-4 | `fixture-future-runtime` through the same pipeline | Works with no edit to `job/` production code |
| **Artifact integrity** | AI-1 | One byte changed, same size | `JOB_ARTIFACT_DIGEST_MISMATCH` |
| | AI-2 | Wrong size; missing | `JOB_ARTIFACT_SIZE_MISMATCH`; `JOB_ARTIFACT_NOT_FOUND` |
| | AI-3 | Adapter never reached on an artifact failure; artifact replaced between tasks | Spy count 0; second task fails |
| **Canonical outputs** | CO-1 | `canonical-json` fixture output, in Rust and Node | Equal bytes; idempotent |
| | CO-2 | Adapter returns non-canonical or wrong-type bytes | `JOB_OUTPUT_NOT_CANONICAL`; `JOB_OUTPUT_TYPE_VIOLATION` |
| **Deterministic replay** | DR-1 | Fixture vectors run twice, in different orders | Identical |
| | DR-2 | The same vectors on Linux CI and Windows | Same digest |
| **Technical verification** | TV-1 | Tampered output | T0 fails at the commitment check |
| | TV-2 | Output breaking its declared type | T1 fails |
| | TV-3 | Evidence states T0 or T1 at most, with `BUSINESS_CORRECTNESS = NOT_ASSESSED`; job anchored on a devnet | No higher claim |
| **Resource limits** | RL-1 | Input one byte over | `JOB_INPUT_TOO_LARGE`, before `run` |
| | RL-2 | Output one byte over | `JOB_LIMIT_OUTPUT_SIZE`, nothing stored |
| | RL-3 | Adapter past `max_wall_ms` | `JOB_LIMIT_WALL_TIME`; worker usable afterwards |
| | RL-4 | `max_memory_mib` | **Declared, not enforced in G2.** Reports state `MEMORY_LIMIT_ENFORCED = NO` |
| **Failure classification** | FC-1 | One test per code | Code count equals test count |
| | FC-2 | Each code has one class, retry rule and blame | Table test against §3.4 |
| | FC-3 | Control-plane attempts; adapter panic | `EXECUTION` retries, others do not; panic becomes `JOB_RUNTIME_FAILED` |
| **Cross-workload isolation** | XI-1 | A Kaayu spec to `JobProfile`, a job spec to the inference profile | Both refuse |
| | XI-2 | Interleaved jobs on one worker | Same outputs as alone; a failure does not affect the next job |
| | XI-3 | Kaayu v1–v3, `mvp-http-cli-v1`, MVP-D and MVP-E vectors | Pass; files unchanged at Git-object level |
| | XI-4 | Kaayu-specific code is unreachable from `job/` | Static check |
| **Privacy-safe evidence** | PE-1 | Canary values in input and artifact; scan errors, logs, metering | 0 occurrences |
| | PE-2 | Every `JobError` rendering | Code and field name or count only |
| | PE-3 | Metering reaches only the private side | Absent from receipts, chain and logs (D14) |

## 7. Decisions needing approval

1. **Provider boundary.** G2's `RuntimeAdapter` is an in-process Rust trait. Independent providers will need a process or network boundary, so that they do not have to link their runtime into MbongoChain. **Recommendation:** approve Slice 1 with the trait, and decide the boundary at Slice 2, when the conformance suite shows what it must carry.
2. **Start of Slice 1.** It needs a separate implementation approval.

## 8. Risks

| Risk | Control |
|---|---|
| Interfaces shaped only by mocks | DS-4 uses a differently shaped mock. Slice 2's boundary decision is the first real test |
| A fixture growing into a workload | §3.5 rule: test-only, no business logic |
| Evidence read as more than it is | TV-3: never above T1, never business correctness |
| A runaway in-process adapter outliving its deadline | RL-3 proves the worker recovers. A hard kill needs the process boundary of decision 1 |

## 9. Slice 2 design (PROPOSED, for approval)

**No Slice 2 code exists, and none may be written until this design is approved.** It replaces Task 5 of §5 and settles §7 decision 1.

### 9.1 Shape

A provider operates **one service bundle**: front, control plane, data plane and worker in one process, as `mvp_service` does today. Independence comes from each provider running its own bundle with its own executor key. The E and F contracts stay in-process, so **no new network protocol is needed**.

| Boundary | Between | Mechanism | New? |
|---|---|---|---|
| Chain | Client and provider | `ComputeTask`, `AnchorReceipt`, `rpc_v0.3` | No |
| Client to provider | Client and bundle | Signed HTTP, the same routes as the MVP, plus two | Same pattern |
| Inside the bundle | Control plane, data plane, worker | In-process calls | No |
| **Worker to runtime** | Worker and the provider's runtime | **OS process: stdin, stdout, exit code** | **Yes: the only new contract** |

### 9.2 Flow

1. `GET /v1/capabilities`: the provider's declaration.
2. `PUT /v1/job-specs`: the client sends its canonical job spec. The provider validates it, selects a runtime and verifies artifacts, and refuses **before anything reaches the chain**. It answers with the `execution_spec`.
3. `PUT /v1/inputs`, then the client commits a `ComputeTask` naming the provider's executor, then `POST /v1/tasks/{id}/register`.
4. The unchanged `Worker` takes the lease, fetches, runs `JobProfile`, persists, and anchors the receipt.
5. `GET /v1/tasks/{id}`, then `GET /v1/tasks/{id}/result`.
6. The client finds the receipt on-chain (`find_receipt`), checks its signature (`verify_receipt_signature`), and runs `verify_result`.

### 9.3 Reused and new

| Need | Reused as it is | New |
|---|---|---|
| Job submission | `ComputeTask`, `rpc_v0.3`, `JobSpec`, `JobProfile` | `provider/` module with the routes above |
| Capability declaration | `capability-advertisement-v1` claims, `Session.spec_tags` | One read-only route. Claims are **declared**, and `eligible()` stays for M4 |
| Runtime invocation | `RuntimeAdapter`, `ResolvedJob` | `SubprocessRuntime` adapter (§9.4) |
| Result commitments | `reference_output_commitment`, private data plane | — |
| Execution receipts | `Worker::bound_receipt`, `anchor_transaction`, Receipt v1 | — |
| Technical verification | `find_receipt`, `verify_receipt_signature`, `verify_result` | Small generic client path. `mvp/verify.rs` is Kaayu-specific and is not used |
| Failure handling | `JobError`, `AttemptEvent::Failed`, leases | Task loop that parks by failure class (§9.5) |
| Request auth | `verify_signature`, `ReplayGuard`, the header names | A signing message with an audience (§9.8, decision 2) |

`mvp/front.rs` cannot be reused directly: it hard-codes the frozen task, the Kaayu input and one executor, and it is protected. Its task loop is about 130 lines; re-writing them in `provider/` is cheaper than risking the released path.

### 9.4 Runner contract (`mbongo-runner:v1`)

The provider's configuration maps `{family, revision, accelerator_class}` to a command and an optional binary hash, verified at start-up.

| Item | Rule |
|---|---|
| Input | Bytes on **stdin**. Never written to disk by the worker |
| Output | Bytes on **stdout**, read up to `output.max_bytes + 1` |
| Job | A fresh directory holding `job.json` (the canonical job spec) and `artifacts/<sha256>`, named by `MBONGO_JOB_DIR` |
| Environment | **Cleared**, except `MBONGO_JOB_DIR` and what the provider's configuration lists. The executor key is never passed |
| stderr | Read, counted and **dropped**: it may hold job content |
| Exit | `0` is success; anything else is `JOB_RUNTIME_FAILED` |
| After | The job directory is deleted |

A runner can be in any language, and can itself launch a container. **Providers never recompile MbongoChain for a workload.** The in-process trait stays as the seam, and `SubprocessRuntime` is one generic implementation of it.

### 9.5 Errors and retry

`ControlPlane` has no terminal failure: a failed task is simply offered again. The MVP front parks a task after 3 attempts. `provider/` does the same, by class:

| Failure | Behaviour |
|---|---|
| `JOB_RUNTIME_FAILED` | Retried, then parked after 3 attempts |
| Any other `JobError` | **Parked at once**; the runner is not called again |
| Lost lease, crash | Existing E rules: a new attempt, a fresh capability |

The code reaches the client through `GET /v1/tasks/{id}`, content-free. It is captured by a profile wrapper, the pattern the front already uses, so `Worker` does not change. A failed task has no receipt.

### 9.6 Timeouts and resource limits

| Limit | Enforced by | Strength |
|---|---|---|
| `max_wall_ms` | Worker kills the child | **Hard** for the direct child. Its own children may survive unless the provider's launcher contains them |
| `output.max_bytes` | Worker stops reading and kills | **Hard** |
| `input.max_bytes` | Before the runner starts | **Hard** |
| `max_memory_mib`, CPU, disk | Nothing | **Not enforced.** Provider responsibility |
| Lease | `max_wall_ms` above the provider's ceiling is refused at step 2 | The worker sends no heartbeat while running, so the ceiling stays below `lease_secs` |

### 9.7 Interoperability is not isolation

| | Interoperability (this design) | Isolation (not this design) |
|---|---|---|
| **Means** | Any runtime plugs in, with no rebuild and no customer code in the core | The runtime cannot reach what it should not |
| **Gives** | Language independence, crash containment, killable jobs, bounded output | Protection of the host, the executor key, other jobs and other clients' data |
| **Provided by** | MbongoChain | **The provider's OS**: a separate account, a container or a VM |

**A runner under the worker's account can read the executor key file and use the network.** Clearing the environment prevents accidents, not attacks. The conformance suite cannot prove isolation, and no report may claim it.

### 9.8 Trust assumptions and limits

- **Provider sees plaintext** (PUBLIC). Provider-resistant confidentiality is not proven (#144).
- **A provider can lie.** It can anchor a wrong output, declare a runtime it does not run, or misreport metering. A receipt proves E only. Evidence stops at T1, and detection needs T2 or T3.
- **Capability claims are declared**, never verified.
- **Clients cannot upload code.** Artifacts are provisioned by the provider, so a client names only what the provider chose to hold. This removes remote code execution from Slice 2.
- **Request signatures have no audience today.** `mvp/auth.rs` signs method, path, timestamp, nonce and body, but not the recipient. That was harmless with one front. With independent providers, a malicious provider could replay a client's signed request to another provider within the 120 s window. `provider/` must bind the provider's executor address and use its own domain.
- **State is in memory** (#169): results are lost on a restart.
- **Transport:** loopback by default. Remote use needs the provider's own TLS boundary. Slice 2 is tested on loopback only and deploys nowhere.
- **No discovery:** a client learns a provider's URL out of band.

### 9.9 Task card

| | |
|---|---|
| Capability | A client gets a verifiable result from a runtime that an independent provider operates, with no MbongoChain rebuild |
| MbongoChain owns | `SubprocessRuntime`, `mbongo-runner:v1`, `provider/` (routes, audience-bound auth, task loop), a `provider_service` binary, the generic client path |
| Clients or providers own | Clients: job specs, inputs, whether a result is right. Providers: runtimes, artifacts, OS isolation, TLS, operating the bundle |
| Excluded | Discovery, registries, `eligible()` wiring, durable state, payment, model adapters, GPU or ML, a shared control plane, client-uploaded code, SDK changes, deployment |

### 9.10 Acceptance criteria

| ID | Criterion |
|---|---|
| S2-1 | A runner written in another language reproduces the `test-vectors/job` outputs byte for byte, with no rebuild of MbongoChain |
| S2-2 | **End to end, on a devnet:** separate client and provider processes, a job reaches an anchored receipt, and the client verifies T0 and T1 |
| S2-3 | `JOB_RUNTIME_FAILED` is retried then parked after 3 attempts; every other class parks at once, with a runner call count of 0 or 1 as expected; the code reaches the client |
| S2-4 | A runner ignoring its deadline is killed (`JOB_LIMIT_WALL_TIME`, process proven gone). A stdout flood is cut at `max_bytes + 1`. Both on Linux CI and Windows |
| S2-5 | A fixture runner reports its environment: no key, seed or unlisted variable. The input is not on disk. The job directory is gone afterwards |
| S2-6 | A job needing an absent runtime or artifact is refused at `PUT /v1/job-specs`, with no chain transaction |
| S2-7 | Unsigned, replayed, wrong-submitter and **wrong-audience** requests are refused, each by its named code |
| S2-8 | Canary values in input and runner stderr: 0 occurrences in logs, status, receipts, chain or metering (PE-1, PE-3) |
| S2-9 | Evidence states T1 at most, with `BUSINESS_CORRECTNESS = NOT_ASSESSED` and no isolation claim (TV-3) |
| S2-10 | `compute-conformance-v1` passes against the bundle through its `Subject` adapter. All existing MVP vectors and suites pass unchanged |

Fixtures stay the three of §3.5. No benchmarks.

### 9.11 Protected surfaces

The paths of §4, plus `worker.rs`, `control_plane.rs`, `data_plane.rs`, `policy.rs`, `mvp/**` and `job/profile.rs`, are proved unchanged with Git objects.

**Existing files touched:** `lib.rs` (one line), the crate's `Cargo.toml` (one `[[bin]]`), `ci.yml` (one step), and `job/mod.rs` (one line). **No protocol, consensus, RPC or SDK change, so no RFC is needed.**

**Rollback:** additive, so revert the squash commit. Stop before merge if a protected path differs, or if the work needs a new network protocol, a chain change or customer logic.

### 9.12 Decisions needed

| # | Decision | Recommendation |
|---|---|---|
| 1 | Provider topology: one bundle per provider, or a shared control plane with remote workers | **Bundle.** A shared control plane needs a new protocol and a central operator |
| 2 | Request auth: a new audience-bound signing message in `provider/`, or the existing one | **New message**, with domain `mbongo-provider-http:v1`. The MVP stays as it is |
| 3 | Artifacts: provider-provisioned only, or client-supplied | **Provider-provisioned.** Client-supplied code needs real isolation first |
| 4 | Isolation: a documented provider duty, or a startup refusal when the runner shares the worker's account | **Documented duty** in Slice 2. A refusal is not portable and proves little |
| 5 | Runner contract: stdin, stdout and exit code, as in §9.4 | **Approve**, as `mbongo-runner:v1` |
