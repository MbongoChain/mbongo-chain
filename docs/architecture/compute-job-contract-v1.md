# Compute Job Contract v1 (`mbongo-job:v1`)

| | |
|---|---|
| **Status** | CANDIDATE. Becomes NORMATIVE when G2 Slice 1 is merged |
| **Kind** | Architecture, non-consensus. RFC 0005 wins on protocol: the chain sees only the opaque `execution_spec` bytes |
| **Code** | `crates/mbongo-compute/src/job/`. **Vectors:** `test-vectors/job/job-contract-v1.json`, computed by `scripts/job-vectors.test.mjs` independently of the Rust code |

A job spec fully describes a job. The contract is data: a worker needs no knowledge of the workload, and no client workload is compiled into MbongoChain.

## `execution_spec`

`b"mbongo-job:v1"` ‖ the 32-byte digest, 45 bytes in total. The digest is `BLAKE3(b"mbongo:job-spec:v1" || canonical_bytes)`. Canonical form: the rules of `inference/canonical.rs` (sorted keys, no whitespace, non-negative integers only). Bytes that are not already canonical are refused, never normalised.

## Job spec

Every field is required, and any unknown field is refused.

| Field | Rule |
|---|---|
| `workload` | Client label, 1–128 bytes. **Opaque** |
| `runtime` | `family`, `revision`, `accelerator_class`, and optional `binary_sha256` |
| `artifacts[]` | `role` (unique), `sha256` (lowercase hex), `size_bytes` |
| `input`, `output` | `type` (`bytes` or `canonical-json`), `max_bytes` |
| `limits` | `max_wall_ms`, enforced. `max_memory_mib`, **declared, not enforced** in v1 Under Linux containment a provider enforces it, capped by its own ceiling ([`PROVIDER_CONTAINMENT_LINUX.md`](../runbooks/PROVIDER_CONTAINMENT_LINUX.md)). |
| `determinism` | `BIT_EXACT`, `BIT_EXACT_PINNED`, `TOLERANCE`, `NON_DETERMINISTIC` |
| `confidentiality` | One or more of `PUBLIC`, `VERIFIED`, `CONFIDENTIAL`, `SOVEREIGN` |
| `params` | An object. **Opaque**; passed to the adapter |

A number outside the canonical model, such as a float, must be carried as a fixed-point integer or a string.

## Validation order

The first failure stops the task, and a failed task has no receipt.

1. Tag and length of `execution_spec`.
2. Resolve the job spec by digest; recompute the digest.
3. Validate the job spec.
4. Select the adapter: family, then revision, accelerator class and pinned build.
5. Verify each artifact, on every task: presence, size, SHA-256.
6. Input commitment (`mbongo:compute-input:v1`), size and type.
7. Run the adapter under `max_wall_ms`.
8. Output size and type (T1), without normalising.
9. Hand metering to the private side; return the bytes.

## Failure codes

Messages are content-free: a code and, for `JOB_SPEC_INVALID`, a field name.

| Class | Codes | Retry | Blame |
|---|---|---|---|
| Spec | `JOB_SPEC_MALFORMED_EXECUTION_SPEC`, `JOB_SPEC_NOT_FOUND`, `JOB_SPEC_DIGEST_MISMATCH`, `JOB_SPEC_INVALID` | No | Client |
| Runtime selection | `JOB_RUNTIME_UNKNOWN`, `JOB_RUNTIME_MISMATCH` | No | Client or provider |
| Artifact | `JOB_ARTIFACT_NOT_FOUND`, `JOB_ARTIFACT_DIGEST_MISMATCH`, `JOB_ARTIFACT_SIZE_MISMATCH` | No | Provider |
| Input | `JOB_INPUT_COMMITMENT_MISMATCH`, `JOB_INPUT_TOO_LARGE`, `JOB_INPUT_TYPE_VIOLATION` | No | Client |
| Limit | `JOB_LIMIT_WALL_TIME`, `JOB_LIMIT_OUTPUT_SIZE` | No | Client or provider |
| Execution | `JOB_RUNTIME_FAILED` | Yes | Provider |
| Output | `JOB_OUTPUT_NOT_CANONICAL`, `JOB_OUTPUT_TYPE_VIOLATION`, `JOB_OUTPUT_COMMITMENT_MISMATCH` | No | Provider |

## What a result proves

At most **T1**: the bytes match the output commitment (`mbongo:compute-output:v1`) and conform to the declared type and size. It says nothing about whether the output is right for the client's purpose, or that the declared runtime ran. Reported metering is untrusted and stays private.

## Provider boundary

`RuntimeAdapter` is an in-process Rust trait: temporary scaffolding. A process or network boundary for independently operated providers is designed at Slice 2. A provider must never need to recompile MbongoChain for a client's workload.
