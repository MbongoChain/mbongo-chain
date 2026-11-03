# G3 Prerequisites — Host-Class Identity and LLM Re-Execution

| | |
|---|---|
| **Status** | **REQUIREMENTS, for approval.** G3 is not started and needs G2 and its own approval. |
| **Authority** | [`GENERAL_PURPOSE_COMPUTE_PROPOSAL.md`](GENERAL_PURPOSE_COMPUTE_PROPOSAL.md) §4.2, §4.7, §7 G3, decision D14. RFC 0005 wins on protocol. |
| **Kind** | Non-consensus, client-side verification. No receipt, protocol or SDK change. |
| **Date** | 2026-11-03 |

## 1. The problem

T2 means that an independent re-execution reproduced the output. For a `BIT_EXACT` contract such as MbongoPay, any conforming host can do that.

An LLM is different. Its output depends on the runtime build, the model bytes, the thread layout and the CPU's instruction paths, so Kaayu is declared `BIT_EXACT_PINNED`: bit-exact only within a **host class**. G3 must define that class before it can report T2 for Kaayu, and it must never let a report imply more.

## 2. What is measured today

| Evidence | Result | What it shows |
|---|---|---|
| #178 client-path runs, 2026-11-03 | 25 of 25 result files byte-identical across two separate devnet runs | **Same host, same build of the runtime, same model, same threads, 6.** Repeatability on one machine |
| V4-G1 determinism check (#179) | 9 of 9 repeats identical, for one lab profile | The same, on one machine |

**Not measured:** any second machine, any other CPU model, any other thread count, Linux against Windows. **No cross-host claim can be made today.** The reference host is one Intel i7-8850H laptop.

## 3. Host-class identity

A host class is the tuple below. Two hosts are in the same class only if every field is equal.

| Field | Source | Why it matters |
|---|---|---|
| `runtime_family`, `runtime_revision` | Existing `BackendIdentity` | Different code |
| `runtime_binary_sha256` | Pinned and verified at start-up | Different build flags change the kernels |
| `model_sha256` | Job spec | Different weights |
| `cpu_vendor`, `cpu_family`, `cpu_model` | CPUID | Different micro-architecture |
| `isa_features` | The ordered, canonical list of instruction-set features the **runtime actually dispatches on** (for example AVX2, FMA, AVX-512 subsets) | Different kernels and rounding |
| `threads` | Worker configuration | Changes reduction order |
| `gpu_offload_layers` | Fixed at 0 on CPU | Any offload is a different class |
| `os_family`, `arch` | Build target | Different libm and allocator paths |
| `generation_params_digest` | Job spec: temperature, seed, token limit, grammar | Different request |

- **`host_class_id`** = `BLAKE3(b"mbongo:host-class:v1" || canonical JSON of the tuple)`, using the existing canonical rules.
- **Trust.** The tuple is **self-declared and untrusted**. A report shows it as `declared`, and an attacker can claim any class. Only T4 (attestation) can bind a class to real hardware.
- **Privacy.** No hostname, serial number, MAC address, user name or path. The tuple describes a class, not a machine.

## 4. Reproducibility requirements for LLM T2

G3 may report T2 for a `BIT_EXACT_PINNED` contract only if **all** of these hold:

| # | Requirement |
|---|---|
| R1 | The contract declares `BIT_EXACT_PINNED` and has an approved reproducibility statement |
| R2 | The original result's `host_class_id` is known, and the verifier's is computed locally |
| R3 | The two ids are **equal**. Otherwise the report is `T2_NOT_APPLICABLE_HOST_CLASS`, never a failure and never a pass |
| R4 | The re-execution uses the exact runtime binary, model, input and generation parameters, each verified by hash first |
| R5 | Comparison is **byte equality** of canonical output. No similarity score and no tolerance |
| R6 | A mismatch within a class is reported as `T2_MISMATCH`. It proves a difference; it does not by itself prove fraud, since the class is only declared |
| R7 | The verifier is a separate process from the worker under test. The report names who ran it. Re-execution by the same operator is reported as such, and is weaker than independent re-execution |
| R8 | The report states the highest level reached and keeps `BUSINESS_CORRECTNESS = NOT_ASSESSED`. T2 says "reproduced", never "correct" or "uninfluenced": a re-execution faithfully reproduces a prompt-injected output too |
| R9 | Private inputs: re-execution needs plaintext, so it is available only to a party already allowed to see the input. T2 grants no new access |

**Required qualification before any Kaayu T2 claim (a G3 acceptance test):**

| # | Test | Expected |
|---|---|---|
| Q1 | At least 50 cases × 3 repeats, same host, after restarts | 100 % byte equality |
| Q2 | Same host, thread count changed | Measured and recorded. The class rule is confirmed or corrected from the result |
| Q3 | A second host with the same tuple | Measured. **If it differs, the tuple is insufficient, and Kaayu T2 is withheld until it is corrected** |
| Q4 | A host with a different tuple | The report must say `T2_NOT_APPLICABLE_HOST_CLASS` |
| Q5 | Load: concurrent requests on the same server, if ever enabled | Measured. Batching can change results. The MVP path is one job at a time |

Q3 needs a second machine, which is not available today. **If none is available, G3 ships T2 for MbongoPay only**, and Kaayu stays at T1.

## 5. Metering

Metering stays in the **private data plane** initially (D14). It is not placed in receipts or on chain, and it is labelled untrusted. G3 reads no metering for verification.

## 6. Deferred

These are out of G3, each to its own gate or RFC (D14):
- signed, content-addressed contract registries;
- open assignment (RFC);
- settlement (RFC);
- T3 redundancy and T4 attestation.

## 7. Open points

| # | Question | Needed before |
|---|---|---|
| H1 | How the original result's `host_class_id` reaches the verifier. Recommendation: the private data plane, beside the result, as untrusted metadata. Receipt metadata would publish hardware details | G3 design |
| H2 | Whether `isa_features` can be read from the runtime, or must be derived from CPUID | G3 design |
| H3 | Access to a second host for Q3 | Kaayu T2 only |
