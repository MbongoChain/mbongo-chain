# MVP CPU Inference Worker — `mbongo-infer:v1` on llama.cpp

**Status:** MVP v0.1 IMPLEMENTATION — NON-CONSENSUS. Gate MVP-B of [`MVP_v0.1.md`](MVP_v0.1.md).
**Implements:** [`MVP_INFERENCE_CONTRACT_v1.md`](MVP_INFERENCE_CONTRACT_v1.md) (frozen; never reinterpreted here). **Code:** `crates/mbongo-compute/src/cpu_inference/`, harness `src/bin/mvp_inference_harness.rs`, setup `scripts/setup-mvp-inference-runtime.sh`. **CI group:** `cpu_inference_worker` (step "Mbongo MVP CPU Inference Worker").
**Baseline:** `20d46774bdb1890b3f510e0ed8e31552dd41c96a`, measured 2026-10-17.

> **Gate result: blocked on the frozen contract, not on the worker.** The worker executes the contract on the pinned runtime and model and fails closed exactly as specified. On the real ten-case synthetic corpus, **8 of 10** produced schema-valid canonical JSON; the other two were cut off by the contract's own `max_tokens = 400` before the JSON closed (`finish_reason: "length"`), so the worker correctly refused them (`OUTPUT_NOT_JSON`, no result, no receipt). The frozen output schema permits far more than 400 tokens of JSON. Resolving that is a change to `kaayu-cv-extract:v1`, which needs the change control of `MVP_v0.1.md` §20 (§16.4).

## 1. Purpose

Turn the reference worker into a real CPU inference worker for exactly one frozen profile: a valid `mbongo-infer:v1` task is resolved to its committed job spec, the runtime and the exact GGUF are verified, the private input is fetched and verified, the frozen prompt is built, llama-server runs on CPU under the frozen output schema, the output is validated and canonicalized, persisted, and the bound receipt is anchored — with no protocol change.

## 2. Scope

One runtime (llama.cpp `llama-server`), one model (the pinned Qwen3-4B-Instruct-2507 Q4_K_M GGUF), one CPU worker, one profile (`kaayu-cv-extract:v1`), one inference path. Not: GPU, vLLM, routing, agents, ODS, marketplace, MBO, trusted metering, pricing, attestation. The reference profile `mbongo-ref:reverse-bytes:v1` stays and keeps passing every existing test.

## 3. Runtime and model acquisition

| Artifact | Source | Identity checked |
|---|---|---|
| llama.cpp CPU build | GitHub release `b11669` of `ggml-org/llama.cpp` — the build tag whose commit `620d2225cc436ab0b001e3200e590e3b286ef9ab` is the commit of release `v0.7.0` (the `v0.7.0` release carries only a pointer file, `nightly-tag.txt` = `b11669`) | archive SHA-256 as published by GitHub; `llama-server` executable SHA-256; `--version` |
| model | `unsloth/Qwen3-4B-Instruct-2507-GGUF` @ `a06e946bb6b655725eafa393f4a9745d460374c9`, file `Qwen3-4B-Instruct-2507-Q4_K_M.gguf` | size 2,497,281,120 B and SHA-256 `3605803b…e597` over every byte |

| Platform | Archive | Archive SHA-256 | `llama-server` SHA-256 |
|---|---|---|---|
| Windows x86-64 CPU | `llama-b11669-bin-win-cpu-x64.zip` | `37b3efcf962847ca869745fd4ccf7f5e7e8dacfeff9fa3523b4ca8cdc66d632b` | `6a4635aeb35656153d4bfeeddf761d24ed84aada9c9108d580737cbed465141b` |
| Linux x86-64 CPU | `llama-b11669-bin-ubuntu-x64.tar.gz` | `cf07c1c112d709bd67e522017471de57ed0ec8bea031d732c5422351c07f65ac` | `3f2b53013c2eb596c8aca87e4740240a41f95bcebf72e61084d6d5ba0db761cc` |

`scripts/setup-mvp-inference-runtime.sh` downloads, verifies and caches both, fails closed (a wrong digest moves the file to `quarantine/` and exits non-zero), never writes inside the repository, and is idempotent. Licenses: llama.cpp MIT ("Copyright (c) 2023-2026 The ggml authors", read at the pinned commit); model Apache-2.0 (base model card and GGUF repository card). Neither the binary nor the weights are committed.

**Recorded on the reference host:** the archive's SHA-256 matched the published digest; the downloaded model's SHA-256 was `3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597`, size 2,497,281,120 B, in 34 s — **exactly the MVP-A pin**. The setup script re-verified the cache (`MVP_INFERENCE_SETUP=PASS`) and, given a corrupted model in a scratch cache, quarantined it and exited 1.

## 4. Worker configuration

Operator configuration only; a task can name none of it. Cache root `$MBONGO_INFER_HOME` (default `~/.cache/mbongo/inference`) with `runtime/b11669-<platform>/`, `models/`, `logs/`, `quarantine/`.

| Setting | Default | Env |
|---|---|---|
| runtime path | `<home>/runtime/b11669-<platform>/llama-server[.exe]` | `MBONGO_INFER_RUNTIME` |
| runtime SHA-256 | the platform pin above | — |
| model path | `<home>/models/<artifact_name>` | `MBONGO_INFER_MODEL` |
| bind | `127.0.0.1` (not configurable) | — |
| port | 9931 | `MBONGO_INFER_PORT` |
| threads | half the logical processors | `MBONGO_INFER_THREADS` |
| context | 8,192 tokens | — |
| start-up timeout | 120 s | — |
| inference timeout | 180 s | `MBONGO_INFER_TIMEOUT_SECS` |
| response cap | 256 KiB | — |

Only those variables are read; the environment is never logged. No secret is needed. **The control-plane lease must outlive the inference timeout** (the live run uses a 600 s lease); otherwise a long inference finds its lease expired at persistence.

## 5. Start-up verification

In order, each failing closed with no process left running: (0) the compiled profile is the frozen one — prompt-template and output-schema SHA-256 and the job-spec digest `be9ae960…fe0e` equal pinned constants, so a rebuilt worker with an edited profile file refuses to start; (1) the executable exists and has the pinned SHA-256; (2) `llama-server --version` reports release core `0.7.0`, build 11669 and a commit prefix of the job spec's commit (the upstream CI stamps every build `-dev`; the commit is authoritative); (3) the model file has the job spec's size and SHA-256; (4) nothing already answers on the port; (5) the server is spawned with structured arguments and reports `/health` 200; (6) `/props` reports `build_info` `b11669-620d2225c` and the configured model file. Readiness is logged as runtime, build, commit, model digest prefix, threads and port.

Server arguments: `--model <path> --host 127.0.0.1 --port <p> --n-gpu-layers 0 --threads <t> --threads-batch <t> --ctx-size 8192 --parallel 1 --no-webui --no-slots --no-cache-prompt --log-verbosity 2`. GPU layers default to "auto" at this revision, so `0` is explicit; the build itself is CPU-only. The slots endpoint (which can expose prompts) and the web UI are off.

## 6. Profile dispatch

`ExecutionProfile` gained three default methods — `spec_tags`, `accepts`, `execute_task` — whose defaults reproduce the reference behaviour exactly; the worker calls them instead of comparing against `spec_tag` and calling `execute`. `ProfileDispatch` holds `[ReverseBytesProfile, InferenceProfile]`: the first profile that accepts the task's exact `execution_spec` runs it; none → `UNSUPPORTED_EXECUTION_PROFILE` (`ExecutionError::UnsupportedSpec`). There is no fallback: a failed inference never becomes a reverse-bytes result. The inference profile accepts exactly the 47-byte spec of the frozen job spec, so the control plane, which offers only tasks whose spec a session advertises, never leases an incompatible task — and the worker never fetches its private input.

## 7. Job spec validation

Every task goes through `inference::preflight` (contract §17 steps 1–8). The job spec is resolved from the repository-defined profile set (`RepositoryProfiles`), re-hashed and compared with the committed digest; there is no chain storage of job specs. Steps 5–6 compare the job spec with the identity the backend proved at start-up; a backend whose runtime or model differs cannot even be bound to the profile.

## 8. Input validation

The worker lifecycle verifies the fetched bytes against `task.input_commitment` before any profile runs (F §7); `preflight` re-checks it, then parses `kaayu-cv-extract-input:v1` and enforces the 10,000-byte canonical bound. Token counts never decide admissibility.

## 9. Prompt construction

Exactly two messages: `system` = the frozen template bytes, `user` = `cv_text`. Nothing else — no timestamp, no worker identity, no preamble.

## 10. llama-server invocation

`POST /v1/chat/completions` on loopback through a bounded HTTP/1.1 client (one request per connection, total deadline, byte cap). Body: the two messages; `temperature` 0.0, `top_p` 1.0, `seed` 42, `max_tokens` 400 — each from the job spec, nothing added; `stream: false`; `cache_prompt: false` (the server documents that KV reuse can change logits); `response_format: {"type": "json_schema", "json_schema": {"schema": <the pinned schema, unmodified>}}`. At this revision the server converts that schema into a sampling grammar (`tools/server/server-common.cpp` at `620d2225`).

## 11. Output validation and canonicalization

`inference::finish`: raw output ≤ 8,000 B, strict parse (no prose stripping, no brace search, no repair), schema validation, canonical bytes, `output_commitment = BLAKE3("mbongo:compute-output:v1" ‖ canonical)`. An oversized output maps to the existing `OUTPUT_SCHEMA_INVALID` ("output exceeds max_output_bytes"); the contract defines no separate code and none is invented. A server body that is not a chat completion is `INFERENCE_FAILED`; model content that is not JSON is `OUTPUT_NOT_JSON`.

## 12. Persistence and receipt

Unchanged lifecycle: the canonical bytes are persisted under a put capability, durability is confirmed, then the bound receipt is signed and anchored. No model, runtime or job-spec field is added to the receipt; the task already commits the job spec through `execution_spec`.

## 13. Failure and retry

| Event | Code | Effect |
|---|---|---|
| server unreachable, died, I/O failure | `INFERENCE_FAILED` | attempt fails; server killed; restarted before the next request |
| request exceeds the inference timeout | `INFERENCE_FAILED` | same (a hung server is not reused) |
| response over the byte cap | `INFERENCE_FAILED` | attempt fails |
| non-200 status | `INFERENCE_FAILED` | attempt fails |
| not JSON / schema-invalid | `OUTPUT_NOT_JSON` / `OUTPUT_SCHEMA_INVALID` | no result, no receipt |
| runtime or model differs | `RUNTIME_REVISION_MISMATCH` / `MODEL_DIGEST_MISMATCH` | worker never ready |
| persistence fails | `RESULT_PERSIST_FAILED` (`FailureClass::Persistence`) | no receipt |

Failed attempts release the lease (E); a retry re-runs the same task with the same job spec, model, runtime, input and parameters — the request is byte-identical (B17). Execution is at-least-once under failure; the first anchored receipt wins; exactly-once is not claimed.

## 14. Crash recovery

| Crash point | Proven by | Result |
|---|---|---|
| before fetch / after fetch | `crash_before_inference_then_one_receipt` | no inference before the crash; retry infers once; one receipt |
| while the request is in flight | real harness, corpus and live chain | `INFERENCE_FAILED`, nothing persisted, no receipt; server restarted (`restarts=1`); retry succeeds; one receipt |
| after inference, before persist | `crash_after_inference_before_persist_reinfers_once_then_one_receipt` | inference repeated once (at-least-once); one receipt |
| after persist, before receipt | `crash_after_persist_reuses_the_result_without_reinference` | durable result reused; no second inference; one receipt |
| after receipt submission | `crash_after_receipt_submit_never_signs_a_second_receipt` | lookup-first; one receipt |

## 15. Observability and privacy

Per attempt the profile logs and records `input_commitment` prefix (public; correlates with `task_id` on-chain), `job_spec_digest` prefix, `runtime_id`, `model_id` (`name@sha256[:12]`), start and completion times, duration, `input_tokens`, `output_tokens` and the outcome code; the worker logs `task_id`. Token counts are **untrusted observational metering**, never a settlement basis. Never logged: CV text, prompt, raw output, canonical result; `ChatRequest` and `Completion` print sizes only in `Debug`; contract errors carry field names, never content. The server runs at verbosity 2 with slots off.

The MVP is **PUBLIC** (J): the Mbongo-operated worker sees the CV plaintext. Input and result stay off-chain. No provider-resistant confidentiality is claimed; K and K-HW are untouched and not on this path.

| Scan | Result |
|---|---|
| CI B19/B20: every log line of the test binary vs CV lines, template, canonical result | 0 hits |
| real corpus run: harness stdout, worker `RUST_LOG=info` log, llama-server log vs 121 private fragments | 0 hits |
| real live-chain run: same three logs vs 32 fragments | 0 hits |
| real live-chain run: 81 blocks vs CV lines, input hex, result text and hex, summary | 0 hits |

## 16. Performance (reference host)

Intel Core i7-8850H (6 cores / 12 threads), 31.3 GB RAM, Windows 11 Pro 10.0.26200; llama.cpp 0.7.0-dev build 11669 commit `620d2225c`, CPU build, 6 threads, ctx 8,192, GPU layers 0.

| Measure | Value |
|---|---|
| model SHA-256 verification at start-up | 10.3–10.5 s |
| cold start (spawn → healthy) | 3.5–3.8 s |
| warm durations of the 8 completed cases (ms) | 51,173 · 27,955 · 55,388 · 54,385 · 30,481 · 50,228 · 48,623 · 31,581 |
| warm P50 / P95 / max | 48.6 s / 55.4 s / 55.4 s |
| R01: prompt / output tokens | 573 / 342 |
| generation rate | ≈ 8.8 tokens/s; prompt processing ≈ 60 tokens/s |
| peak llama-server working set / private memory | 6,262 MB / 3,967 MB (≈ 4.8 GB working set warm) |
| live chain: attempt after restart | 52.5 s |

**MVP_LATENCY_TARGET (P95 ≤ 120 s) is plausible** for outputs that fit 400 tokens on this host: 55 s observed. It is **not** plausible for the long CV at a budget large enough to finish it: R08 needed 1,331 tokens and 223 s. One host; not a claim about other hardware.

### 16.4 The blocking finding

| Case | Budget | `finish_reason` | completion tokens | duration | parses |
|---|---|---|---|---|---|
| R08 long (12 positions) | 400 | `length` | 400 | 75 s | no |
| R08 | 1,500 (diagnostic) | `stop` | 1,331 | 223 s | yes |
| R10 executive (3 positions) | 400 | `length` | 400 | 56 s | no |
| R10 | 1,500 (diagnostic) | `stop` | 463 | 63 s | yes |

Grammar enforcement works — every response that was allowed to finish was valid, schema-shaped JSON. What fails is the frozen contract's arithmetic: `kaayu-cv-profile:v1` allows up to 15 positions with 300-character descriptions, 40 skills and a 600-character summary, roughly 2,000 tokens of JSON at its maximum, while `generation.max_tokens` is 400. Any realistic CV with more than about three described positions can be truncated. The worker refuses those correctly, but the closing bar of this gate (10/10 structurally valid) cannot be met without changing the frozen profile, which this gate is not allowed to do. Resolving it is a `kaayu-cv-extract` version 2 under `MVP_v0.1.md` §20, choosing between: **(a)** tighter output bounds so the schema's worst case fits the budget (keeps the latency target; drops older positions and long descriptions), **(b)** a larger `max_tokens` with the same schema (completes every case; breaks P95 ≤ 120 s on this host for long CVs), or **(c)** both moderately. Recommendation: (a) with an explicit worst-case serialized size ≤ the budget, verified by a test that serializes a maximal valid profile and counts its tokens.

## 17. Real-model proof

| Class | What ran | Result |
|---|---|---|
| MOCK_WORKER_TESTS | `cpu_inference_worker`, 31 tests (B01–B25, crash matrix, runtime negatives) | 31/31 |
| REAL_LLAMA_CPP_TEST | pinned binary verified (digest, `--version`, `/props`), started, health, restart after kill | PASS |
| REAL_MODEL_TEST | 10 synthetic CVs (R01–R10) through the real profile | 8/10 schema-valid; 2 truncated by the frozen budget (§16.4); 3/3 invalid inputs refused before the model (0 reached it); in-flight kill → `INFERENCE_FAILED` then recovery |
| REAL_WORKER_E2E | live producer node; task accepted; first attempt killed mid-inference; retry; receipt anchored and verified by the client; result retrieved and commitment matched; chain and log scans | PASS |

Qualitative sanity: R01 and its repeat R06 produced **byte-identical** canonical results on the same host (not a cross-platform claim); the prompt-injection CV produced no `@` and no out-of-enum seniority; sparse, French, Unicode and multilingual CVs produced schema-valid profiles with non-ASCII strings canonicalized stably. Extraction quality is not graded; `OUTPUT_CORRECTNESS_PROVEN = NO`.

## 18. CI strategy

CI never downloads the model. The `cpu_inference_worker` group runs the real worker lifecycle, real profile, real preflight and finish, with a scripted backend and an in-process fake HTTP server for the transport negatives. The confidential suite is asserted by running its own unchanged binary in a child process (its K20 case needs the process-wide logger the B19/B20 test already owns). Real runs are `mvp_inference_harness corpus|e2e` on a host that ran the setup script; a cached self-hosted runner can be added later.

## 19. Security limitations

The worker trusts its own host: post-setup tampering with the runtime's shared libraries is outside the threat model (the archive digest covers them at setup; the executable digest at every start). Model and runtime execution are operationally trusted; the receipt proves attribution and commitments, not that this model produced the output. Fixed seed and temperature 0 gave repeatable bytes on one host; `BIT_EXACT_CROSS_PLATFORM_DETERMINISM = NO`. llama-server is never exposed beyond loopback and has no API key; MVP-C must not expose it.

## 20. Non-goals

GPU, vLLM, routing, a second model or profile, agents, ODS, remote model APIs (none is called — inference is local and offline after setup by construction; not separately tested with the network disabled), marketplace, MBO, trusted metering, pricing, attestation, a process-supervision framework.

## 21. MVP-C handoff (once §16.4 is resolved)

MVP-C builds a worker as `Worker::new(ids, executor, Box::new(ProfileDispatch::new(vec![Box::new(ReverseBytesProfile), Box::new(InferenceProfile::new(Arc::new(LlamaServer::start(RuntimeConfig::from_env(..)?, &kaayu::job_spec())?.0))?)])))` and never touches llama.cpp. The client computes `execution_spec` and `input_commitment` per the contract (helpers must reproduce `test-vectors/inference/kaayu-cv-extract-v1.json`), and exposes the observation fields of §15. The lease must exceed the inference timeout.

## Invariants

B1 inference stays application-level · B2 `ComputeTask` wire unchanged · B3 `Receipt` wire unchanged · B4 exact runtime revision required (digest, build, commit, `/props`) · B5 exact model digest required · B6 a wrong model never substitutes (cannot bind) · B7 a wrong runtime never substitutes · B8 exact job-spec digest required · B9 input commitment verified before inference · B10 private input off-chain · B11 private output off-chain · B12 input absent from default logs · B13 output absent from default logs · B14 prompt-template digest pinned and checked at start-up · B15 output-schema digest pinned and checked at start-up · B16 generation parameters come only from the job spec · B17 malformed JSON cannot succeed · B18 schema-invalid output cannot succeed · B19 canonical bytes are the persisted bytes · B20 receipt only after persistence · B21 a retry cannot change the contract · B22 a server failure cannot create a success receipt · B23 PUBLIC: the provider sees plaintext · B24 K and K-HW unchanged · B25 no GPU · B26 no remote model API · B27 no agents · B28 no marketplace · B29 no MBO · B30 token counts observational only · B31 output correctness not proven · B32 fixed seed is not cross-platform bit determinism · B33 runtime and model not in git · B34 offline after setup · B35 MVP-C needs no llama.cpp knowledge.
