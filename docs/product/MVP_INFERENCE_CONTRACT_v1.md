# MVP Inference Execution Contract v1 — `mbongo-infer:v1`

**Status:** NORMATIVE APPLICATION CONTRACT — NON-CONSENSUS — MVP v0.1.
**Decided:** 2026-10-17 on baseline `38445ef278894ec31ac30b12660c6afe5d738f62` (protocol v0.4, frozen).
**Implements:** [`MVP_v0.1.md`](MVP_v0.1.md) §11 (gate MVP-A). **Implementation:** `crates/mbongo-compute/src/inference/`. **Vectors:** [`test-vectors/inference/`](../../test-vectors/inference/README.md). **Test group:** `inference_contract` (CI step "Mbongo MVP Inference Contract").

> **One sentence.** A `ComputeTask` whose `execution_spec` is the 47 bytes `"mbongo-infer:v1" ‖ BLAKE3("mbongo:infer-job-spec:v1" ‖ canonical job spec)` means: run exactly the runtime, model artifact, prompt template, output schema and generation parameters that job spec names, on the private input `input_commitment` commits to, and persist the canonical JSON result `output_commitment` commits to — with nothing added to the v0.4 protocol.

---

## 1. Purpose

MVP-B will build the inference worker. This document fixes every application-level decision that worker would otherwise have to invent: which model, which exact artifact, which runtime at which revision, what the input is, what output is required, which generation parameters are allowed, how the job spec is identified and committed, how the worker verifies it holds the right job spec, and how the client verifies the result corresponds to that contract. Same `execution_spec` → same committed job spec → same intended model, runtime and profile.

## 2. Scope

In scope: the `execution_spec` layout for inference; the job-spec document, its canonical form, digest and validation; runtime and model identity rules; the MVP profile `kaayu-cv-extract:v1` (input contract, prompt template, output schema, generation parameters, limits); result canonicalization; the worker's pre- and post-execution validation order; error taxonomy; retry and drift rules; privacy, logging and observability fields; what the receipt proves.

Out of scope: running llama.cpp (MVP-B), any client transport or CLI (MVP-C), GPU, agents, pricing, metering as a payment basis, MBO, a network profile registry, output-correctness verification.

## 3. Authority boundary

| Requirement | Existing authority | Current meaning | Plane | Testable observable |
|---|---|---|---|---|
| `execution_spec` | [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md) §2.1, §2.10 (≤ 1,024 bytes), §2.12 (opaque; application-versioned by convention) | committed, stored, hashed into `task_id`; never interpreted by consensus; public | APPLICATION | A03, A19: 47 bytes, inside the bound, task envelope unchanged |
| `task_id` | RFC 0005 §2.2 | `BLAKE3(DOMAIN_TASK ‖ SCALE(task))`; a different `execution_spec` is a different task | chain | A14: a changed job spec changes `execution_spec` and therefore `task_id` |
| `input_commitment` | RFC 0005 §2.4; reference convention `BLAKE3("mbongo:compute-input:v1" ‖ bytes)` ([reference-worker.md](../development/reference-worker.md)) | commitment to the private input object; derivation unknown to consensus | chain ↔ PRIVATE_DATA_PLANE | A15, A21, A22: input commitment is over the input object only |
| `task.executor` | RFC 0005 §2.5, rules (q)–(s) | the one address that may answer | chain | unchanged; not touched by this contract |
| private input | [F](../architecture/compute-private-data-plane-interface.md) §4, §6–§7 | an off-chain object fetched under a single-use capability and verified against `input_commitment` before execution | PRIVATE_DATA_PLANE / WORKER | A21: commitment mismatch → no execution |
| result persistence | F §10, [E](../architecture/compute-control-plane-worker-interface.md) §11 | result persisted and confirmed durable before any receipt | WORKER / PRIVATE_DATA_PLANE | unchanged lifecycle; A13 fixes which bytes are persisted |
| receipt binding | RFC 0005 rules (q)–(s); [RFC 0002](../rfcs/0002-receipt-anchoring-v0.3.md); [RECEIPT_SPEC](../specs/RECEIPT_SPEC_v0.1.md) | receipt carries `task_id`, `input_commitment`, `output_commitment`, executor, empty metadata, signature | chain | A18: receipt wire unchanged |
| execution representation | E §5–§6; [J](../architecture/compute-provider-capability-policy.md) §5.3 `execution.profile_tags` | the session advertises the profile tags it can run; eligibility requires the task's tag | CONTROL_PLANE | the inference worker advertises `mbongo-infer:v1` (§25) |
| worker profile | [reference-worker.md](../development/reference-worker.md) "REFERENCE / TEST EXECUTION PROFILE"; `execution::ExecutionProfile` | a profile answers to one `execution_spec` tag; the worker refuses any other | WORKER | A05: the reference tag is not an inference spec and vice versa |

Nothing in this document is consensus. Where it and RFC 0005 could be read differently, RFC 0005 wins.

## 4. Terminology

- **job spec** — the off-chain document saying *how* to execute (`InferenceJobSpecV1`).
- **job_spec_digest** — its 32-byte BLAKE3 commitment (§8).
- **profile** — an application layer over the generic contract: input contract, prompt template, output schema, parameters. The MVP has exactly one, `kaayu-cv-extract:v1`.
- **input object** — the private bytes `input_commitment` commits to: a canonical `kaayu-cv-extract-input:v1` object (§11).
- **result** — the canonical `kaayu-cv-profile:v1` bytes `output_commitment` commits to (§15).
- **canonical** — the one byte form defined in §7.

## 5. `execution_spec` format

Fixed length, 47 bytes:

| Offset | Length | Content |
|---|---|---|
| 0 | 15 | ASCII `mbongo-infer:v1` (the version lives in the tag) |
| 15 | 32 | `job_spec_digest` |

Decoding order: the family prefix `mbongo-infer:` must match (else `MALFORMED_EXECUTION_SPEC`: not an inference spec at all); then the full tag `mbongo-infer:v1` (else `UNSUPPORTED_INFERENCE_SPEC_VERSION`); then the exact length (else `MALFORMED_EXECUTION_SPEC`). No length prefix is needed because every field is fixed-size; no separator because the tag is a fixed prefix. 47 of the 1,024 permitted bytes are used.

The `execution_spec` carries nothing else: no CV text, no prompt, no output, no schema, no model secret, no key (A16, A17).

## 6. Job spec schema (`InferenceJobSpecV1`)

A flat tree of objects whose leaves are ASCII strings or non-negative integers. The field set is exact: unknown or missing fields are `JOB_SPEC_INVALID`.

| Field | Type | Rule |
|---|---|---|
| `contract` | string | exactly `mbongo-infer:v1` (else `UNSUPPORTED_INFERENCE_SPEC_VERSION`) |
| `runtime.family` | string | `llama.cpp` (else `UNSUPPORTED_RUNTIME`) |
| `runtime.mode` | string | `llama-server` |
| `runtime.revision` | string | exact release tag `vX.Y.Z`; never `latest`, `current`, `stable`, `main`, `master`, `HEAD` or empty |
| `runtime.commit` | string | 40 lowercase hex, the commit the tag resolves to |
| `runtime.license` | string | SPDX id |
| `model.family`, `model.name` | string | human names; **not** authoritative |
| `model.source` | string | URL of the artifact repository |
| `model.source_revision` | string | 40 lowercase hex repository revision |
| `model.artifact_name` | string | file name |
| `model.artifact_sha256` | string | 64 lowercase hex — **authoritative** model identity |
| `model.artifact_size_bytes` | integer | > 0 |
| `model.format` | string | `gguf` |
| `model.quantization` | string | e.g. `Q4_K_M` |
| `model.license` | string | SPDX id |
| `profile.id` | string | e.g. `kaayu-cv-extract` |
| `profile.version` | integer | ≥ 1 |
| `profile.input_contract` | string | e.g. `kaayu-cv-extract-input:v1` |
| `profile.output_contract` | string | e.g. `kaayu-cv-profile:v1` |
| `profile.prompt_template_sha256` | string | 64 lowercase hex over the template bytes |
| `profile.output_schema_sha256` | string | 64 lowercase hex over the schema bytes |
| `generation.temperature_milli` | integer | temperature × 1000, ∈ [0, 200] |
| `generation.top_p_milli` | integer | top-p × 1000, ∈ [500, 1000] |
| `generation.seed` | integer | ≤ 2³²−1 |
| `generation.max_tokens` | integer | ∈ [1, 400] for `profile.version` 1; ∈ [1, 600] for version ≥ 2 (amendment CC-1, §14) |
| `limits.max_input_bytes` | integer | ∈ [1, 10000] |
| `limits.max_output_bytes` | integer | ∈ [1, 8000] |

Every string is non-empty, ≤ 256 bytes, printable ASCII (0x20–0x7E) without `"` or `\`, so canonical job-spec bytes contain no escape sequence. No floats exist anywhere (fractions are integer thousandths). No resource hints: provider count is one and the backend is CPU (MVP_v0.1 §6–§7); none are needed, and none are authoritative if a later profile adds them. No price: price does not affect execution identity (§23, A25).

## 7. Job spec canonicalization (constrained canonical JSON)

Not RFC 8785; a subset whose every rule is local and tested (`inference::canonical`):

| Construct | Rule |
|---|---|
| object | members sorted by key, byte-wise ascending over the key's UTF-8; duplicate keys rejected |
| array | order preserved |
| string | UTF-8; `"` → `\"`, `\` → `\\`, U+0000–U+001F → `\u00xx` lowercase hex; every other scalar raw (including U+007F and non-BMP) |
| integer | non-negative decimal, no leading zeros, ≤ 2⁶⁴−1; the only number form |
| whitespace | none |
| `true`, `false`, `null`, negative or fractional numbers, exponents | rejected on parse; never produced |
| nesting | ≤ 16 levels |

The parser accepts any RFC 8259 document inside those constraints (whitespace, escapes and surrogate pairs included); `canonicalize` is idempotent. The same canonical form serves the job spec (ASCII-only by §6), the input object and the result (full UTF-8).

## 8. Job spec digest

```
job_spec_digest = BLAKE3( "mbongo:infer-job-spec:v1" ‖ canonical_job_spec_bytes )
```

Domain `mbongo:infer-job-spec:v1` is distinct from `mbongo:compute-task:v1`, `mbongo:compute-input:v1`, `mbongo:compute-output:v1` and every receipt domain (A02). BLAKE3 is the repository's commitment convention (RFC 0005 §2.2, §2.4). This is an application-level commitment, not a consensus hash: no validator computes it.

## 9. Runtime identity

`runtime = { family: "llama.cpp", mode: "llama-server", revision: "v0.7.0", commit: "620d2225cc436ab0b001e3200e590e3b286ef9ab", license: "MIT" }`.

Evidence: GitHub release `v0.7.0` of `ggml-org/llama.cpp`, published 2026-10-15T17:13:19Z, tag object `757cad02…` → commit `620d2225cc436ab0b001e3200e590e3b286ef9ab`; `LICENSE` at that commit is the MIT License, "Copyright (c) 2023-2026 The ggml authors". The project also publishes rolling `bNNNNN` build tags; those are not release tags and the validator refuses them as revisions (`runtime_build_number_not_release_tag`). A worker verifies at start-up that the server it runs reports exactly this tag and commit (§17 step 5, §18). Nothing of the runtime is vendored here.

## 10. Model identity

`model = { family: "Qwen3", name: "Qwen3-4B-Instruct-2507", source: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF", source_revision: "a06e946bb6b655725eafa393f4a9745d460374c9", artifact_name: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf", artifact_sha256: "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597", artifact_size_bytes: 2497281120, format: "gguf", quantization: "Q4_K_M", license: "Apache-2.0" }`.

**Provenance and license.** The base model `Qwen/Qwen3-4B-Instruct-2507` is published under Apache-2.0 (model card: `license: apache-2.0`; non-thinking instruct model; llama.cpp listed among supported runtimes). No official Qwen GGUF repository for this model was reachable at decision time (the `Qwen/Qwen3-4B-Instruct-2507-GGUF` name answers 401 from the public API); the artifact is the Q4_K_M quantization published in the `unsloth` repository above (card license `apache-2.0`, `base_model: Qwen/Qwen3-4B-Instruct-2507`, most-downloaded community GGUF of this model at decision time). The digest is the Hugging Face LFS SHA-256 recorded for that file at that repository revision, obtained from the repository metadata **without downloading the artifact**; MVP-B's set-up downloads the file, recomputes SHA-256 over its bytes, and refuses to start on any difference (`MODEL_DIGEST_MISMATCH`). No model content is copied into this repository.

**The digest is authoritative.** Two artifacts with the same human name and different bytes are different models (A08, A14); the names exist for humans and logs.

**Tokenizer.** `TOKENIZER_SEPARATE_ID_REQUIRED = NO`: a GGUF file embeds its tokenizer (vocabulary, merges, pre-tokenizer type, special tokens) and the chat template as metadata, so the artifact digest already fixes them. A separate tokenizer identity would be redundant.

## 11. Input contract (`kaayu-cv-extract-input:v1`)

The private input object is the canonical form of

```
{"contract":"kaayu-cv-extract-input:v1","cv_text":"<CV as UTF-8 text>"}
```

Rules: exactly those two fields; `cv_text` non-empty after trimming; the only control characters allowed are `\n`, `\r`, `\t`; canonical bytes ≤ `limits.max_input_bytes` (10,000 — aligned with the ≤ 2,500-token product bound at a conservative four bytes per token; token counts are never a contract rule, bytes are). Text extraction from PDFs stays on the client side.

`input_commitment = BLAKE3("mbongo:compute-input:v1" ‖ canonical input bytes)` — the existing reference convention, unchanged. The job spec is **not** part of the input object and the input is **not** part of the job spec: two commitments, two authorities (A15).

## 12. Prompt and profile identity

Prompt construction is **worker-owned and pinned**: the chat request is exactly two messages — `system` = the prompt template bytes verbatim (`crates/mbongo-compute/profiles/kaayu-cv-extract-v1/prompt-template.txt`, SHA-256 `6c950b4863bc339af94d904dbb609b3dd9098259740f01a6539406e641276d63`), `user` = `cv_text`. No substitution, no additional message, no tool, no retrieval (A23). The model's own chat template is embedded in the GGUF and therefore covered by the artifact digest.

`profile = { id: "kaayu-cv-extract", version: 1, input_contract: "kaayu-cv-extract-input:v1", output_contract: "kaayu-cv-profile:v1", prompt_template_sha256, output_schema_sha256 }`. A worker cannot change the instructions silently: the template digest is inside the job spec, so a changed template is a changed digest, a changed `execution_spec` and a different task.

## 13. Output schema (`kaayu-cv-profile:v1`)

`crates/mbongo-compute/profiles/kaayu-cv-extract-v1/output-schema.json` (JSON Schema 2020-12, SHA-256 `81521447e7b7b45c670ff1717d608fc2304765ab530587aba2370ba0a9191af5`). One object, `additionalProperties: false`, all fields required, **no numbers, booleans or nulls anywhere** — only strings, arrays and objects, so the canonical form of §7 is total over valid outputs:

| Field | Type | Bound |
|---|---|---|
| `summary` | string | ≤ 600 chars |
| `seniority` | enum | `unknown` `junior` `mid` `senior` `lead` `executive` |
| `location` | string | ≤ 120 chars; city, region or country only |
| `skills` | string[] | ≤ 40 × ≤ 60 chars |
| `experience` | object[] `{title ≤120, organization ≤120, start ≤10, end ≤10, description ≤300}` | ≤ 15 |
| `education` | object[] `{degree ≤120, institution ≤120, year ≤10}` | ≤ 10 |
| `languages` | object[] `{language ≤40, level ∈ unknown basic intermediate fluent native}` | ≤ 10 |
| `certifications` | string[] | ≤ 15 × ≤ 100 chars |

The profile is **identity-free by design**: no name, contact, address, birth date or identifier field exists, and an output that adds one is schema-invalid (`identity_field_added`). The schema uses only keywords llama.cpp's schema-to-grammar converter supports (`type`, `required`, `properties`, `additionalProperties:false`, `items`, `maxItems`, `maxLength`, `enum`); MVP-B passes it as `response_format` and measures that the grammar is honoured. Bounds are character counts, matching JSON Schema `maxLength`. The typed Rust validator `CandidateProfileV1` is the executable form of the schema; A24 asserts the field set, bounds and enumerations agree.

## 14. Generation parameters

`generation = { temperature_milli: 0, top_p_milli: 1000, seed: 42, max_tokens: 400 }`; allowed ranges in §6. Temperature 0 with a fixed seed is supported by llama-server (greedy decoding) and makes output variance low; it **does not** make inference bit-reproducible across CPU builds, thread counts or hardware, and this contract claims no such thing (§21). A job spec outside the ranges is `INVALID_GENERATION_PARAMETERS` before any inference (A11).

**Amendment CC-1 (MVP-B.1).** The `max_tokens` range is keyed by `profile.version`: version 1 keeps `[1, 400]` exactly, so every version-1 job spec, vector and test of this contract has the outcome it had; version 2 and later may declare up to 600 tokens, the hard ceiling of change control CC-1 ([`MVP_v0.1.md`](MVP_v0.1.md) §20.1). Nothing else in `mbongo-infer:v1` changes: not the `execution_spec` framing, not the job-spec field set, canonicalization or digest, not the validation order or the error taxonomy. The reason is a profile defect, not a contract defect: `kaayu-cv-extract:v1` allowed more output than 400 tokens can hold; its successor [`kaayu-cv-extract:v2`](KAAYU_CV_EXTRACT_PROFILE_v2.md) narrows the output and needs a moderately larger budget.

## 15. Result canonicalization

The worker never persists raw model bytes. Steps: parse the raw output with the §7 parser (`OUTPUT_NOT_JSON` on failure — including `null`, numbers, prose before the JSON, truncation); validate against §13 (`OUTPUT_SCHEMA_INVALID`); serialize canonically; persist exactly those bytes; `output_commitment = BLAKE3("mbongo:compute-output:v1" ‖ canonical bytes)`. Whitespace and key order therefore never change a commitment; content does (A13). The raw output is bounded by `limits.max_output_bytes` (8,000) before parsing.

## 16. Error taxonomy

Application-level, never consensus. Codes are the stable names; variants never carry input, prompt or output content.

| Code | Raised when |
|---|---|
| `UNSUPPORTED_INFERENCE_SPEC_VERSION` | `mbongo-infer:` prefix with another version; or `contract` field not `mbongo-infer:v1` |
| `MALFORMED_EXECUTION_SPEC` | not an inference spec, or wrong length |
| `JOB_SPEC_NOT_FOUND` | no job spec with the committed digest is known |
| `JOB_SPEC_DIGEST_MISMATCH` | the resolved spec does not hash to the committed digest |
| `JOB_SPEC_INVALID` | schema, identity or limit rule violated |
| `UNSUPPORTED_RUNTIME` | runtime family or mode not runnable |
| `RUNTIME_REVISION_MISMATCH` | worker's tag or commit differs |
| `MODEL_NOT_FOUND` | no model loaded |
| `MODEL_DIGEST_MISMATCH` | loaded model's SHA-256 differs |
| `INVALID_INPUT_FORMAT` | input object malformed or wrong contract |
| `INPUT_TOO_LARGE` | input over the limit |
| `INPUT_COMMITMENT_MISMATCH` | fetched bytes do not hash to `task.input_commitment` |
| `INVALID_GENERATION_PARAMETERS` | parameters outside §6 ranges |
| `INFERENCE_FAILED` | the runtime produced no output |
| `OUTPUT_NOT_JSON` | raw output outside the §7 model |
| `OUTPUT_SCHEMA_INVALID` | JSON but not §13 |
| `RESULT_PERSIST_FAILED` | data plane did not confirm durability |

MVP-B maps each to E's failure classes (`Input` for the input-side codes, `Execution` for the rest) when it reports to the control plane.

## 17. Worker validation order

`inference::preflight` — any failure means **no execution, no result, no receipt**:

1. parse `execution_spec` (§5)
2. resolve the job spec by digest from the repository-defined profiles
3. verify the resolved spec's digest equals the committed digest (a registry that answers wrongly is caught: A06)
4. validate the job spec: field set, identity rules, generation and limit bounds (§6, §14)
5. runtime family, mode, revision **and commit** equal the worker's (§9)
6. model artifact SHA-256 equals the loaded model's (§10)
7. fetched input hashes to `task.input_commitment` (the worker lifecycle already enforces this before any profile runs, F §7; the contract re-checks so the profile is safe on its own)
8. input object valid for the profile and within `limits.max_input_bytes` (§11)

then execution, then `inference::finish`:

9. raw output ≤ `limits.max_output_bytes`; parse (§7); validate (§13); canonicalize (§15)
10. persist the canonical bytes; only after the data plane confirms durability, build the receipt with `output_commitment` over those bytes (F §10, E §11)

A failure after inference (steps 9–10) yields no receipt of any kind: the receipt model has no failure receipt and this contract invents none.

## 18. Retry and drift semantics

- **Retry** happens under E's attempt semantics for the same task, the same job spec and the same input. The job spec is immutable by construction: it is identified by the digest in the task. A different model, runtime, template, schema or parameter is a different job spec, a different `execution_spec`, a different task (A14, A24).
- **Drift** fails closed. At start-up the worker records its runtime tag and commit and the SHA-256 of the model file it loaded; steps 5–6 compare them on every task. A worker running another model or revision never silently accepts the task.
- **Versioning.** `mbongo-infer:v1` changes only on a breaking change of this contract's semantics (layout, canonicalization, digest domain, job-spec schema). `kaayu-cv-extract` increments `profile.version` whenever any field that alters execution identity changes (prompt, schema, model, runtime, parameters, limits); every such change already changes the digest, and the version is the human-readable marker of it. A change of a field that does not alter execution identity has no place in the job spec.

## 19. Privacy and logging

| Public (on-chain, permanent) | Private (off-chain, deletable) |
|---|---|
| `ComputeTask`: submitter, executor, salt, `input_commitment`, the 47-byte `execution_spec` | CV text; the input object; the constructed prompt; the raw model response; the canonical result |
| `Receipt`: `task_id`, commitments, executor, empty metadata, signature | capabilities, leases, sessions |
| — | the job spec itself is **PUBLIC_METADATA** in classification (it names model and runtime, never CV data) but is not published on-chain; only its digest is. In the MVP it is a repository-defined document |

Logs **may** carry: `task_id`, `job_spec_digest`, model and runtime identity, timings, token counts, error code. Logs **must not** carry, by default: CV text, the prompt, the model output, the canonical result (F §13 extended to this profile). Error variants are built so they cannot.

## 20. Observability

Operational fields MVP-B and MVP-C expose, none consensus: `task_id`, `status`, `runtime_id` (`family/mode/revision`), `model_id` (`name@sha256[:12]`), `job_spec_digest`, `submitted_at`, `started_at`, `completed_at`, `execution_duration_ms`, `input_tokens`, `output_tokens` (as reported by llama-server: **untrusted operational metering**, never a settlement basis), `failure_code` (§16), `receipt_tx`. The MVP's displayed reference price is configuration outside this contract (MVP_v0.1 §8).

## 21. Receipt semantics

The receipt proves: executor attribution (signed by the key of the executor the client named), task binding (`task_id`), input binding (`input_commitment`), output binding (`output_commitment` over the canonical result). It does **not** prove that the worker used the claimed model or runtime, that the JSON is semantically or factually correct, or that another run would produce the same bytes. In the MVP the single Mbongo-controlled worker's model and runtime execution is **operationally trusted** (MVP_v0.1 §7); the chain commitments and the receipt remain independently verifiable. Client-facing text may not call this path "verified inference" (`OUTPUT_CORRECTNESS_PROVEN = NO`).

## 22. Golden vector

`test-vectors/inference/kaayu-cv-extract-v1.json`, reproduced byte-exactly by test A20 and by the committed document `crates/mbongo-compute/profiles/kaayu-cv-extract-v1/job-spec.json`:

```
job_spec_canonical = {"contract":"mbongo-infer:v1","generation":{"max_tokens":400,"seed":42,"temperature_milli":0,"top_p_milli":1000},"limits":{"max_input_bytes":10000,"max_output_bytes":8000},"model":{"artifact_name":"Qwen3-4B-Instruct-2507-Q4_K_M.gguf","artifact_sha256":"3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597","artifact_size_bytes":2497281120,"family":"Qwen3","format":"gguf","license":"Apache-2.0","name":"Qwen3-4B-Instruct-2507","quantization":"Q4_K_M","source":"https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF","source_revision":"a06e946bb6b655725eafa393f4a9745d460374c9"},"profile":{"id":"kaayu-cv-extract","input_contract":"kaayu-cv-extract-input:v1","output_contract":"kaayu-cv-profile:v1","output_schema_sha256":"81521447e7b7b45c670ff1717d608fc2304765ab530587aba2370ba0a9191af5","prompt_template_sha256":"6c950b4863bc339af94d904dbb609b3dd9098259740f01a6539406e641276d63","version":1},"runtime":{"commit":"620d2225cc436ab0b001e3200e590e3b286ef9ab","family":"llama.cpp","license":"MIT","mode":"llama-server","revision":"v0.7.0"}}
job_spec_digest    = be9ae960334c3e917ad600e2b5220440c27dd79c5e586e998e60cbc148affe0e
execution_spec     = 6d626f6e676f2d696e6665723a7631 ‖ be9ae960334c3e917ad600e2b5220440c27dd79c5e586e998e60cbc148affe0e   (47 bytes)
```

Companion vectors: the synthetic input object and its `input_commitment` (`fca07b64…78e6`), and one schema-valid output in two non-canonical forms reducing to one canonical byte string with `output_commitment` `62f389ac…382e`.

## 23. Invariants

| # | Invariant | Where |
|---|---|---|
| A1 | the inference contract is non-consensus | §3; A18, A19 |
| A2 | `execution_spec` contains no private input | §5; A16 |
| A3 | `execution_spec` contains no private result | §5; A17 |
| A4 | `input_commitment` remains the input authority | §11; A15 |
| A5 | `job_spec_digest` remains the execution-contract authority | §8; A14 |
| A6 | `task.executor` remains the executor authority | §3 (untouched) |
| A7 | the job-spec digest is verified before execution | §17 step 3; A06 |
| A8 | the model artifact digest is verified before execution | step 6; A08 |
| A9 | the runtime revision (tag and commit) is verified before execution | step 5; A07 |
| A10 | model or runtime drift fails closed | §18; A07, A08, A21 |
| A11 | the input commitment is verified before execution | step 7; A21 |
| A12 | invalid input does not execute | step 8; A09, A12 |
| A13 | invalid generation parameters do not execute | step 4; A11 |
| A14 | output must be valid JSON | §15; A13 |
| A15 | output must satisfy the schema | §13; A13, A24 |
| A16 | persisted result bytes equal committed result bytes | §15, §17 step 10 |
| A17 | receipt wire unchanged | A18 |
| A18 | `ComputeTask` wire unchanged | A19 |
| A19 | inference output correctness is not proven | §21 |
| A20 | the provider may see plaintext (PUBLIC profile) | §19; MVP_v0.1 §7 |
| A21 | raw input stays off-chain | §19 |
| A22 | raw output stays off-chain | §19 |
| A23 | a retry cannot silently change the job spec | §18 |
| A24 | a different model or configuration requires a new job spec and task | §18; A14 |
| A25 | price is not part of execution identity | §6 (no price field) |
| A26 | token counts are observational only | §20 |
| A27 | the profile registry is repository-local in the MVP | `RepositoryProfiles`; §24 |
| A28 | no marketplace logic | §2 |
| A29 | no agent semantics | §2, §12 (two messages, no tools) |
| A30 | no MBO settlement | §2 |
| A31 | the same `execution_spec` always names the same job spec | §8 digest is content-derived; A20 |
| A32 | the output schema is identity-free | §13; `identity_field_added` |

## 24. Non-goals

A network or on-chain profile registry; a second profile, model or runtime; GPU; a plugin framework; prompt templating beyond the verbatim system message; any receipt or task field; token-economic semantics of token counts; a failure receipt; output-correctness verification (#52); hardware attestation as a requirement (K and K-HW are unchanged and remain available to a future CONFIDENTIAL profile).

## 25. MVP-B handoff

MVP-B implements an `ExecutionProfile` whose `spec_tag` is the 15-byte tag prefix check plus `preflight`, and must not reinterpret anything here:

| Item | Where |
|---|---|
| `execution_spec` parser/encoder | `inference::execution_spec` |
| job-spec schema, canonical form, digest, validation | `inference::job_spec`, `inference::canonical` |
| the frozen profile, prompt, schema, parameters, limits | `inference::kaayu` and `crates/mbongo-compute/profiles/kaayu-cv-extract-v1/` |
| golden vector and negative cases | `test-vectors/inference/` |
| runtime revision policy | §9; `check_runtime` |
| model digest policy and acquisition | §10; `check_model`; download → SHA-256 → refuse on mismatch |
| input and output types | `CvExtractionInputV1`, `CandidateProfileV1` |
| validation order | §17; `preflight`, `finish` |
| error taxonomy and E mapping | §16 |
| advertisement | the session advertises `execution.profile_tags ⊇ {"mbongo-infer:v1"}` under J; no new discovery protocol |
| what to measure | latency on the CPU reference host; that `response_format` with the §13 schema is honoured by llama-server v0.7.0 |

The SDK wire format is unchanged (`SDK_WIRE_CHANGE = NO`); application helpers (`createInferenceExecutionSpec`, job-spec digest) are deferred to MVP-C, where they must reproduce §22 byte-for-byte.

## See also

- [`MVP_v0.1.md`](MVP_v0.1.md) — the frozen product scope this contract serves
- [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md) §2.4, §2.10, §2.12 — commitments, the bound, opacity
- [reference-worker.md](../development/reference-worker.md) — the lifecycle MVP-B keeps
- [compute-privacy-data-plane.md](../architecture/compute-privacy-data-plane.md) §7, §13 — provider visibility, metadata privacy
- [compute-provider-capability-policy.md](../architecture/compute-provider-capability-policy.md) §5.3 — `execution.profile_tags`
- [`test-vectors/inference/README.md`](../../test-vectors/inference/README.md)
