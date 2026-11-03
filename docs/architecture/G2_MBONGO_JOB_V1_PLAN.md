# G2 — `mbongo-job:v1` Implementation Plan

| | |
|---|---|
| **Status** | **PLAN, for approval.** No G2 product code exists, and none may be written until the maintainer gives a separate implementation approval. |
| **Authority** | Direction, boundaries and gate rules: [`GENERAL_PURPOSE_COMPUTE_PROPOSAL.md`](GENERAL_PURPOSE_COMPUTE_PROPOSAL.md) §1.3, §4.11, §7 (decisions D3, D10, D13). RFC 0005, the v0.4 protocol lock and every NORMATIVE document win on their subjects. |
| **Kind** | Non-consensus, worker-side and client-side. No protocol, consensus, RPC, SDK or released-MVP change. |
| **Date** | 2026-11-03 |

## 1. Goal

Prove that a second, different workload can run through the existing task, receipt and dispatch primitives, using generic interfaces that a future ML or GPU runtime can implement without a protocol change.

**In scope:**
- the `mbongo-job:v1` contract document, job spec and vectors;
- the `RuntimeAdapter` and `WorkloadContract` interfaces;
- the `mbongo-native` adapter (deterministic Rust, integer arithmetic);
- the `mbongopay-scorecard:v1` contract and scorecard artifact;
- a seeded synthetic generator and an independent Node reference;
- a test-only fake runtime that exercises the interfaces for future runtimes;
- a conformance-suite extension.

**Out of scope:**
- GPU or ML execution, an ONNX or llama.cpp adapter;
- moving Kaayu. `mbongo-infer:v1` and `kaayu-cv-extract` v1–v3 stay exactly as released;
- the MVP front, the MVP CLI, `mvp_service`, the SDK, the chain;
- T2 re-execution reports (G3), signed registries, open assignment, pricing, settlement;
- real financial data, automated lending decisions, or any accept or decline output.

## 2. Design corrections to the §4.11 sketch

Reading the code for this plan produced four refinements. None changes the direction.

| Finding | Consequence for G2 |
|---|---|
| `inference::ContractError` lives on a protected path, and its variants are inference-specific | A new error type, `JobError`, in the new module (§6). `ContractError` is not edited |
| The canonical JSON model (`inference/canonical.rs`) has **only non-negative integers**: no booleans, `null`, negatives or fractions | The scorecard fits as it is. It is reused unchanged. Signed points use an offset or a `{sign, magnitude}` form, fixed in the contract. A future `TOLERANCE` contract must carry numbers as fixed-point integers or as strings; that is a contract rule, not a change to canonical JSON |
| `ExecutionError` has only `UnsupportedSpec` and `Failed(String)` | `ExecutionProfile` is unchanged. `JobError` renders to a **content-free code** inside `Failed`, so the worker and control plane need no change |
| `ProfileDispatch` lives in `cpu_inference/profile.rs`, a protected path | It is used as it is. Only its constructor's argument list differs at the test call site |

## 3. Code layout

**New files only**, plus one line in `crates/mbongo-compute/src/lib.rs` (`pub mod job;`). No new crate dependency.

```
crates/mbongo-compute/src/job/
  mod.rs        public surface
  spec.rs       job spec: parse, validate, canonicalize, digest; execution_spec tag
  runtime.rs    RuntimeAdapter, RuntimeIdentity, RunOutput, UntrustedMetering, Limits
  contract.rs   WorkloadContract, DeterminismClass, OutputType, OutputCheck
  error.rs      JobError, failure classes, codes
  profile.rs    JobProfile: the ExecutionProfile implementation
  metering.rs   MeteringSink (private side)
  native.rs     mbongo-native adapter
  mbongopay.rs  mbongopay-scorecard:v1
crates/mbongo-compute/tests/job_contract.rs   the §7 acceptance tests
test-vectors/job/                             job-spec, canonical I/O, scorecard, synthetic set
scripts/mbongopay-gen.mjs                     seeded synthetic generator
scripts/mbongopay-ref.mjs                     independent reference scorer
docs/architecture/compute-job-contract-v1.md  the normative contract (slice 1)
```

**Digest:** `BLAKE3(b"mbongo:job-spec:v1" || canonical_bytes)`, beside `mbongo:infer-job-spec:v1`.

**`execution_spec`:** `b"mbongo-job:v1"` ‖ the 32-byte digest, 45 bytes in total.

## 4. Pipeline

`JobProfile::execute_task` runs these steps in this order. The first failure stops the task, with its named code.

1. Parse `execution_spec`: tag and length.
2. Resolve the job spec by digest and recompute the digest.
3. Validate the spec, including the field rules in §7.1.
4. Look up the `WorkloadContract` by `contract`. Check that `determinism` and `output.type` equal the contract's own.
5. Look up the `RuntimeAdapter` by `runtime`. Check it against `contract.runtime_requirement()`.
6. Verify artifacts: hash and size.
7. Check the input commitment and the input-size limit.
8. `contract.validate_input`.
9. `adapter.run`, under the limits.
10. `contract.canonical_output`, then the output-size limit.
11. `contract.check_output` (T1). A worker never returns a result that fails its own contract.
12. Hand metering to the sink and return the bytes.

## 5. MbongoPay v1

| Aspect | Rule |
|---|---|
| Label | Every output carries `"purpose": "synthetic risk score for infrastructure testing - not a credit decision"` |
| Input | `numeric-record`: 8–12 integer features in a fixed order, each with a declared range. No identity field and no protected attribute |
| Model | An integer scorecard (bins to points), an artifact identified by SHA-256 |
| Output | `score` (integer, fixed range), `risk_band` (an enum, not a decision), up to 3 `reason_codes` in a fixed tie-break order |
| Forbidden | Any accept, decline, approve or limit field; floats; real data |
| Determinism | `BIT_EXACT` |
| Synthetic set | At least 200 cases from a committed seed, including every bin edge. Expected outputs come from the Node reference, which is written from the contract document and not from the Rust code |

## 6. Failure classification

`JobError` has one code per failure. Each code belongs to a class, and every message is content-free: a code and, at most, a field name or a count.

| Class | Codes (initial set) | Retry? | Blame |
|---|---|---|---|
| `SPEC` | `JOB_SPEC_MALFORMED_EXECUTION_SPEC`, `JOB_SPEC_NOT_FOUND`, `JOB_SPEC_DIGEST_MISMATCH`, `JOB_SPEC_INVALID` | No | Client |
| `CONTRACT` | `JOB_CONTRACT_UNKNOWN`, `JOB_CONTRACT_DETERMINISM_MISMATCH`, `JOB_CONTRACT_OUTPUT_TYPE_MISMATCH` | No | Client |
| `RUNTIME_SELECTION` | `JOB_RUNTIME_UNKNOWN`, `JOB_RUNTIME_REQUIREMENT_UNMET` | No on this worker | Provider or client |
| `ARTIFACT` | `JOB_ARTIFACT_NOT_FOUND`, `JOB_ARTIFACT_DIGEST_MISMATCH`, `JOB_ARTIFACT_SIZE_MISMATCH` | No | Provider |
| `INPUT` | `JOB_INPUT_COMMITMENT_MISMATCH`, `JOB_INPUT_TOO_LARGE`, `JOB_INPUT_INVALID` | No | Client |
| `LIMIT` | `JOB_LIMIT_WALL_TIME`, `JOB_LIMIT_OUTPUT_SIZE` | No, for a deterministic job | Client or provider |
| `EXECUTION` | `JOB_RUNTIME_FAILED` | Yes, within the existing attempts | Provider |
| `OUTPUT` | `JOB_OUTPUT_NOT_CANONICAL`, `JOB_OUTPUT_CONTRACT_VIOLATION` | No | Provider |

A failed task has no receipt, as today. The classes are off-chain reporting, and no failure receipt is proposed.

## 7. Acceptance tests

Each test states its expected population and asserts it before comparing. A negative test passes only by reaching its **named code**, never merely by failing.

### 7.1 Generic job specification validation (JS)

| ID | Test | Expected |
|---|---|---|
| JS-1 | Spec vector: canonical bytes and digest reproduce in Rust and in Node | Equal, on every vector |
| JS-2 | `execution_spec` length and tag | Exactly 45 bytes; anything else → `JOB_SPEC_MALFORMED_EXECUTION_SPEC` |
| JS-3 | Non-canonical spec (key order, whitespace, duplicate key, float, boolean, `null`) | Rejected; never normalised silently |
| JS-4 | Each required field removed, one at a time | `JOB_SPEC_INVALID` naming the field; the count of cases equals the count of required fields |
| JS-5 | Unknown field at each level | `JOB_SPEC_INVALID`; only `params` is contract-defined |
| JS-6 | Spec bytes that do not hash to the requested digest | `JOB_SPEC_DIGEST_MISMATCH` |
| JS-7 | `mbongo-job` and `mbongo-infer` domains over identical bytes | Different digests |
| JS-8 | `confidentiality` lists a class the contract does not support | `JOB_SPEC_INVALID` |

### 7.2 Runtime and workload-contract dispatch (DS)

| ID | Test | Expected |
|---|---|---|
| DS-1 | `ProfileDispatch` holding `mbongo-infer:v1` and `mbongo-job:v1`: each spec reaches its own profile | Proven by a spy, not by the output alone |
| DS-2 | Unknown tag | `ExecutionError::UnsupportedSpec`, as today |
| DS-3 | Unknown contract; unknown runtime | `JOB_CONTRACT_UNKNOWN`; `JOB_RUNTIME_UNKNOWN` |
| DS-4 | A runtime that fails `runtime_requirement` (family, revision, accelerator) | `JOB_RUNTIME_REQUIREMENT_UNMET`, and the adapter's `run` is never called |
| DS-5 | **Future-runtime interface test.** A test-only fake adapter (`family = "test-onnx"`, `accelerator_class = "gpu:test"`) and a test-only `TOLERANCE` contract with fixed-point output run through the same pipeline | Dispatches and returns canonical bytes **with no edit to `job/` production code**. It computes nothing real: no ML or GPU execution |
| DS-6 | A `NON_DETERMINISTIC` test contract | Accepted by dispatch, and its declared class is visible to the verifier |

### 7.3 Artifact integrity (AI)

| ID | Test | Expected |
|---|---|---|
| AI-1 | Correct scorecard | Accepted |
| AI-2 | One byte changed, same size | `JOB_ARTIFACT_DIGEST_MISMATCH` |
| AI-3 | Size differs | `JOB_ARTIFACT_SIZE_MISMATCH`, checked before hashing |
| AI-4 | Artifact missing | `JOB_ARTIFACT_NOT_FOUND` |
| AI-5 | The artifact is replaced between two tasks | The second task fails: no verified-once cache keyed by name |
| AI-6 | The adapter's `run` is never reached on any AI-2 to AI-4 failure | Spy count 0 |

### 7.4 Canonical outputs (CO)

| ID | Test | Expected |
|---|---|---|
| CO-1 | Output vectors reproduce byte for byte in Rust and in Node | Equal |
| CO-2 | `canonicalize(canonicalize(x)) == canonicalize(x)` over the whole synthetic set | Equal |
| CO-3 | `output_commitment` uses the `mbongo:compute-output:v1` domain over the canonical bytes | Equals the vector |
| CO-4 | Reason-code ties | Fixed order, on cases built to tie |
| CO-5 | An adapter that returns non-canonical bytes | `JOB_OUTPUT_NOT_CANONICAL` |

### 7.5 Deterministic replay (DR)

| ID | Test | Expected |
|---|---|---|
| DR-1 | At least 200 cases: Rust against the Node reference | **100 % byte equality**, count asserted |
| DR-2 | The same set run twice, in different orders, in one process | Identical: no hidden state |
| DR-3 | The same set on Linux CI and on the Windows reference host | The same digest over all outputs |
| DR-4 | Static check: no float type, clock, random source, environment read or hash-map iteration in `native.rs` or `mbongopay.rs` | None found |
| DR-5 | Replay from the committed seed regenerates the synthetic set | Same digest |

### 7.6 Technical verification (TV)

G2 covers T0 and T1 only. T2 is G3.

| ID | Test | Expected |
|---|---|---|
| TV-1 | One `mbongo-infer:v1` and one `mbongo-job:v1` task through one worker, receipts anchored on a devnet | T0 passes for both |
| TV-2 | Tampered output bytes | T0 fails at the commitment check |
| TV-3 | `check_output` on an out-of-range score, an unknown band and an extra field | T1 fails with `JOB_OUTPUT_CONTRACT_VIOLATION` |
| TV-4 | Every evidence record states T0 or T1 as the highest level, with `BUSINESS_CORRECTNESS = NOT_ASSESSED` | No T2 and no semantic claim |

### 7.7 Resource limits (RL)

| ID | Test | Expected |
|---|---|---|
| RL-1 | Input one byte over the limit | `JOB_INPUT_TOO_LARGE`, before `run` |
| RL-2 | Output one byte over the limit | `JOB_LIMIT_OUTPUT_SIZE`, and no result is stored |
| RL-3 | A test adapter that sleeps past `max_wall_ms` | `JOB_LIMIT_WALL_TIME`; the worker stays usable for the next task |
| RL-4 | A limit above the worker's ceiling | `JOB_SPEC_INVALID` |
| RL-5 | `max_memory_mib` | **Declared and recorded, not enforced in G2.** An in-process adapter cannot be limited reliably. Every report states `MEMORY_LIMIT_ENFORCED = NO`; enforcement belongs to adapters that run in a separate process |

### 7.8 Failure classification (FC)

| ID | Test | Expected |
|---|---|---|
| FC-1 | One test per `JobError` code | Each reached by its own branch; the code count equals the test count |
| FC-2 | Each code maps to exactly one class, retry rule and blame value | Table test against §6 |
| FC-3 | Retry behaviour through the existing control-plane attempts | `EXECUTION` retries; `SPEC`, `INPUT` and `ARTIFACT` do not re-run the adapter |
| FC-4 | A panic inside an adapter | Becomes `JOB_RUNTIME_FAILED`; the worker survives |

### 7.9 Cross-workload isolation (XI)

| ID | Test | Expected |
|---|---|---|
| XI-1 | A Kaayu spec given to `JobProfile`, and a job spec given to the inference profile | Both refuse; neither reaches a backend |
| XI-2 | The scorecard artifact named in a spec for another contract | Refused |
| XI-3 | Interleaved MbongoPay and Kaayu tasks on one worker, using the fake inference backend | Outputs equal the same tasks run alone: no shared mutable state |
| XI-4 | A failing job followed by a valid one, in both orders across workloads | The valid one is unaffected |
| XI-5 | **Kaayu regression.** The `kaayu-cv-extract` v1–v3 vectors, `mvp-http-cli-v1`, MVP-D and MVP-E vectors | Pass, and the files are unchanged at Git-object level |
| XI-6 | Kaayu-specific logic (the injection screen, CV field names) is unreachable from `job/` | Static check |

### 7.10 Privacy-safe evidence (PE)

| ID | Test | Expected |
|---|---|---|
| PE-1 | Canary values planted in the input and artifact, then every error, log line, metering record and receipt metadata field scanned | 0 occurrences |
| PE-2 | `JobError` rendering for every code | Code and field name or count only |
| PE-3 | Metering goes only to the private-side sink | Absent from receipts, chain and logs (D14); labelled untrusted |
| PE-4 | Committed test vectors | Synthetic only, from the seed. Guardrail test: no protected attribute, identity field or decision field in the input or output schema |
| PE-5 | Campaign evidence files | Contain codes, counts and digests; no inputs or outputs beyond the committed synthetic set |

## 8. Slices

Each slice is a commit in one PR set. The first failed criterion stops the work.

| # | Slice | Tests |
|---|---|---|
| 1 | Contract document, vectors, Node reference and generator; **no Rust** | JS-1 and CO-1 vectors exist; DR-5 |
| 2 | `spec.rs`, `error.rs` | JS, FC-1 and FC-2 for the codes so far |
| 3 | `runtime.rs`, `contract.rs`, `profile.rs`, test-only fake runtime | DS, AI, RL, PE-2 |
| 4 | `native.rs`, `mbongopay.rs` | CO, DR, TV-2, TV-3, PE-4 |
| 5 | Integration, isolation, conformance extension, devnet test | TV-1, TV-4, XI, FC-3, FC-4, PE-1, PE-3, PE-5 |

## 9. Protected surfaces and rollback

Protected paths and rollback conditions are those of [§7 G2](GENERAL_PURPOSE_COMPUTE_PROPOSAL.md) of the proposal, with this one addition: the only existing file G2 may edit is `crates/mbongo-compute/src/lib.rs` (one line), along with `docs/INDEX.md` and `typos.toml` if needed. The merge gate states this expected list before looking at the diff.

## 10. Risks

| Risk | Control |
|---|---|
| Interfaces shaped by one real runtime | DS-5 and DS-6 run a differently shaped fake runtime through unchanged code; review against the proposal's §4.11.4 table |
| Self-confirming scorer | The Node reference is written from the contract document, and DR-1 compares bytes |
| Scope creep toward real credit use | §5 forbidden list, PE-4 guardrail test, and the output label |
| A fake runtime mistaken for ML support | It is test-only (`cfg(test)` or under `tests/`), and no shipped binary contains it |
| Wall-time limit on an in-process adapter | A runaway adapter thread can outlive the deadline. RL-3 proves that the worker recovers; a hard kill needs a separate process, which later adapters will use |
| Evidence read as more than it is | TV-4: G2 evidence never exceeds T1 and never states business correctness |

## 11. Approval needed

**A separate maintainer approval is required to start slice 1.** Nothing in this plan authorises G2 code, and it does not depend on G1.
