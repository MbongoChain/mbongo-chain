# MVP-D — 20-Job Kaayu Acceptance Run

**Status:** MVP v0.1 ACCEPTANCE EVIDENCE — NON-CONSENSUS. Gate MVP-D of [`MVP_v0.1.md`](MVP_v0.1.md); issue [#160](https://github.com/MbongoChain/mbongo-chain/issues/160).
**Measures:** the frozen MVP path of [`MVP_HTTP_CLI.md`](MVP_HTTP_CLI.md) (MVP-C), [`MVP_CPU_INFERENCE_WORKER.md`](MVP_CPU_INFERENCE_WORKER.md) (MVP-B), [`MVP_INFERENCE_CONTRACT_v1.md`](MVP_INFERENCE_CONTRACT_v1.md) (MVP-A) and [`KAAYU_CV_EXTRACT_PROFILE_v2.md`](KAAYU_CV_EXTRACT_PROFILE_v2.md), unchanged. **Harness:** [`scripts/mvp-d-acceptance.mjs`](../../scripts/mvp-d-acceptance.mjs). **Corpus:** [`test-vectors/mvp/mvp-d-corpus-v1.json`](../../test-vectors/mvp/mvp-d-corpus-v1.json). **Evidence:** [`evidence/mvp-d/`](evidence/mvp-d/).

> **Decision: PASS.** 20 consecutive real jobs ran through the supported CLI → HTTP front → worker → devnet path on the AC-powered reference host; **20 succeeded**. Every job has exactly one valid anchored receipt, a retrieved result whose recomputed commitment matches it, and a schema-valid `kaayu-cv-profile:v2` result; no private data appears on-chain, in any log or in the CLI's diagnostics. **P95 submission → client verification: 77.9 s** (≤ 120 s). The controlled recovery case — llama-server killed mid-inference on D07 — completed on a second attempt with exactly one anchored receipt.
>
> **BUSINESS_EXTRACTION_ACCURACY = NOT_ASSESSED.** This run proves reliability, integrity and privacy of the path; it says nothing about whether any extraction is factually correct. One qualitative observation (§14.4) shows a prompt injection steering a schema-valid result.

## 1. Purpose

Answer one question with a scripted, recorded run: does the frozen MVP path work reliably enough, repeatedly enough and fast enough on the reference host to proceed to launch hardening (MVP-E)? MVP-D measures; it changes no product code, profile, model, runtime, route, CLI command or authentication.

**Exit criteria, stated before the run:** 20 consecutive jobs; ≥ 19 successes (≥ 95 %); 0 private-data leaks; 0 invalid receipts; 0 result-commitment mismatches; 0 duplicate anchored receipts; every successful result schema-valid; P95 submission → verified ≤ 120 s on AC power; one real failure-recovery case with exactly one final anchored receipt; `BUSINESS_EXTRACTION_ACCURACY = NOT_ASSESSED`.

## 2. Environment

| | |
|---|---|
| CPU | Intel Core i7-8850H @ 2.60 GHz — 6 cores / 12 threads; 2,592 MHz reported before and after; load 5 % at start, 9 % at end |
| RAM | 31.3 GiB |
| OS | Windows 11 Pro 10.0.26200 |
| Power | **AC** — `Win32_Battery.BatteryStatus = 2` before (charge 76 %) and after (94 %); power plan "Balanced" (`381b4222-…`) |
| Inference | CPU only, 6 threads, `GPU_OFFLOAD = 0`, ctx 8,192, inference timeout 180 s |
| Runtime | llama.cpp v0.7.0 (b11669 @ `620d2225`), `llama-server.exe` SHA-256 `6a4635ae…141b` = pinned — **RUNTIME_VERIFIED** |
| Model | Qwen3-4B-Instruct-2507 Q4_K_M, 2,497,281,120 bytes, SHA-256 `3605803b…e597` = job spec — **MODEL_VERIFIED** (hashed once by the harness, again by the worker at start-up) |
| Profile v2 | prompt template SHA-256 `93ec9176…1f87`, output schema `0a81727a…50ab` = the v2 golden vector — **PROFILE_V2_VERIFIED** |
| Devnet | three native `mbongo-node` processes (producer + two followers, 1-s blocks, harness profile of [`DEVNET_V0.4_OPERATIONS.md`](../runbooks/DEVNET_V0.4_OPERATIONS.md)), converged on the same block at height 3 before any job |
| Service | one `mvp_service serve` on `127.0.0.1:8740` for all 20 jobs; ready 17.0 s after launch, llama-server cold start 3.8 s (not part of any job's latency) |
| Binaries | `mvp_service` `f86f31eb…`, `mbongo-mvp` `c40e9c52…`, `mbongo-node` `a4cd39d5…` (release, `--locked`) |
| Window | 2026-10-26 22:01:50 → 22:17:42 UTC |

Full values: [`evidence/mvp-d/environment.json`](evidence/mvp-d/environment.json).

## 3. Frozen baseline

Product code is merged `dev` at `d3a30ac785e4b763263719c13aa7c0f85a5ca28f` (MVP-C, #159). The campaign ran at `bb778a7` (harness and corpus added, working tree clean). Between the two, **114 / 114 protected blobs and 16 / 16 protected trees are identical** in Git objects — including the whole of `crates/mbongo-compute/src` and `crates/mbongo-compute/tests`, the SDK, core, node and verification crates, specs, RFCs, the v2 profile assets and the inference vectors. The code that ran is the merged code.

## 4. Corpus

20 synthetic CV texts — invented people, fictional organisations, no real contact data — frozen before the run; the harness refuses to start if any SHA-256 differs. D01–D10 are the MVP-B corpus verbatim (D06 repeats D01 on purpose), D11 is the MVP-C fixture, D12–D20 are new.

| ID | Kind | Source | Bytes | SHA-256 (cv_text, UTF-8) |
|---|---|---|---|---|
| D01 | simple | MVP-B corpus R01 | 620 | `be7cd1c7b4e8719cd8750289d394795b665709324d572493cd2f8bc26c4da3e5` |
| D02 | sparse | MVP-B corpus R02 | 50 | `43bddd6a032266bbd4ebbf8f5fcd6dff86201eec1914be8a41ebfb84b36d742b` |
| D03 | french | MVP-B corpus R03 | 579 | `c3a9f8cb379275300740067cf0e5365accba5770596c864915e50e6bfc4cfad5` |
| D04 | unicode | MVP-B corpus R04 | 464 | `506e15e18ecb2f6103322a83d6f5d46769ff48249fb8cc1da148fd6c360d11ff` |
| D05 | prompt_injection | MVP-B corpus R05 | 453 | `cbbcb4d8412b10742afd2dae121c281bea4eeeab7fa7039a35f21e5978f1107d` |
| D06 | repeat_warm | MVP-B corpus R06 | 620 | `be7cd1c7b4e8719cd8750289d394795b665709324d572493cd2f8bc26c4da3e5` |
| D07 | multilingual_skills | MVP-B corpus R07 | 425 | `af7a31e503a6ec593526564a2f8d314fd6f55fd96ce19d54171f0c7536a55507` |
| D08 | long | MVP-B corpus R08 | 3364 | `f5ec3267f2540957e66ceac440922dd65f10201727940e8874fa63dcbf9e4f77` |
| D09 | student_no_experience | MVP-B corpus R09 | 335 | `07a03d62ddfc040db5bdc1735627a59a67d6a71e855724bfecb46cf596f2aa52` |
| D10 | executive | MVP-B corpus R10 | 494 | `4a3937a6f2ead1adc5ac9169f9d9f3f1c975ab0d364983879152d334c755b41b` |
| D11 | mvp_c_fixture | MVP-C fixture | 1197 | `54083c66159007fa6e4756344c8f7adfbaa8aea5a6c8ee1e042674f93414edf1` |
| D12 | healthcare_mid | new (MVP-D) | 573 | `8caad3eb1edf1b1390e98ede9f7fb76664abdde8ec8fdd1b55e5a6814d670c7e` |
| D13 | french_bilingual | new (MVP-D) | 672 | `f29efabe19ebb4dc6ede0c61528ef4401b0fcd7534017df2c35f1c039bbedbac` |
| D14 | career_changer | new (MVP-D) | 580 | `44224d6dc62f6c36052ac402adbd5e3efa545bb5365724ae4802d1447cfb1585` |
| D15 | many_short_roles | new (MVP-D) | 1047 | `4405b5ee5e656f3e15301188ed4cc25e82eb81ac50c495ded899e616ee65ef73` |
| D16 | spanish | new (MVP-D) | 647 | `46de48df9b87f21fe27a2983405e1da4d40cb255476c19b8533d262ee4180610` |
| D17 | unicode_scripts | new (MVP-D) | 726 | `0187902d5ef51f5618bc2cac3cabde8c9ef6fe4059eae3624de34144e1b1418f` |
| D18 | prompt_injection_json | new (MVP-D) | 488 | `a285ababbf57d2b461a29b1dfe3ba4e4e0f1924075b0782ce39542ccfb4e9e1c` |
| D19 | bootcamp_junior | new (MVP-D) | 281 | `2c3f954250e5f86688d7d1e28066de39447b7780592e38e3b885a3ae2a0c0375` |
| D20 | long_consultant | new (MVP-D) | 1780 | `9e4c630e8d6c5081b6dec1b15fb50bcdfa5a0524a7b03bef4c5bbab315230ba0` |

## 5. Procedure

```bash
node scripts/mvp-d-acceptance.mjs --build --recovery-job D07 --run-dir <outside the repository> --evidence docs/product/evidence/mvp-d
```

1. Refuse unless on AC power; record the environment; build the release binaries (`--locked`).
2. Check every corpus SHA-256; verify runtime, model and v2 profile digests once.
3. Start the three-node devnet and wait until all three nodes hold the identical block at height ≥ 3.
4. Fund the executor (`mvp_service fund-executor`), start **one** `mvp_service serve`.
5. For D01 → D20, sequentially: check the service process, the front (a `status` request) and llama-server are alive; run `mbongo-mvp submit - --wait --json` with the CV on standard input (no CV ever written to disk by the harness, no result displayed); keep the CLI's one-line JSON record, its exit code and its standard error. Sample memory every 30 s.
6. On D07 only: once the front reports the worker busy on the task, wait 10 s, then kill the llama-server process (`Stop-Process -Force`).
7. After the 20th job: re-run `mbongo-mvp verify --json` for every task; retrieve every result (`mbongo-mvp result --out`, into the private run directory); read every block on all three nodes and audit every `AnchorReceipt`; scan chain blocks and every log for the corpus' text, the results, the prompt template and the key seeds.
8. Write sanitized evidence — identifiers, hashes, timestamps, statuses; no CV text, no result content.

The campaign ran once. Two short dry runs on battery power earlier the same day (`--allow-battery`, 2 jobs and 1 job) only debugged the harness — they found and fixed a log-file race and a recovery trigger that never fired — and are not part of this evidence.

## 6. Success definition

A job is a success only if the CLI exits `0` with `outcome = VERIFIED`: the task was committed on-chain, the worker completed, a receipt is anchored and its signature and its task, executor and input bindings verify (`receipt_integrity = VERIFIED`), the result was retrieved and its recomputed output commitment equals the receipt's (`result_integrity = VERIFIED`), and the bytes are canonical `kaayu-cv-profile:v2` (`schema_conformity = VERIFIED`). `business_accuracy = NOT_ASSESSED` is required and does not prevent success. The four dimensions are recorded separately; there is no combined flag.

## 7. Per-job results

| Job | Fixture | Kind | Task | Attempts | Result | Receipt | Result integrity | Schema | Business | Inference | Submission → verified | Tokens in / out |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | D01 | simple | `8e4a527181dd…` | 1 | SUCCESS | VERIFIED (h 60, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 33.8 s | 37.4 s | 568 / 190 |
| 2 | D02 | sparse | `17c1398b0f21…` | 1 | SUCCESS | VERIFIED (h 82, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 18.5 s | 21.3 s | 439 / 86 |
| 3 | D03 | french | `3c445d5429e7…` | 1 | SUCCESS | VERIFIED (h 124, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 38.4 s | 41.5 s | 597 / 221 |
| 4 | D04 | unicode | `9270e894ab20…` | 1 | SUCCESS | VERIFIED (h 163, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 35.1 s | 38.4 s | 584 / 192 |
| 5 | D05 | prompt_injection | `2760c3d81b1f…` | 1 | SUCCESS | VERIFIED (h 195, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 28.0 s | 31.4 s | 527 / 140 |
| 6 | D06 | repeat_warm | `1bc7e21974bb…` | 1 | SUCCESS | VERIFIED (h 233, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 34.3 s | 37.4 s | 568 / 190 |
| 7 | D07 | multilingual_skills | `0881b177b8b6…` | 2 | SUCCESS | VERIFIED (h 285, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 35.4 s | 51.5 s | 537 / 174 |
| 8 | D08 | long | `307ff5d6d44d…` | 1 | SUCCESS | VERIFIED (h 384, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 94.8 s | 98.2 s | 1221 / 466 |
| 9 | D09 | student_no_experience | `945732cb7871…` | 1 | SUCCESS | VERIFIED (h 408, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 21.7 s | 24.3 s | 499 / 99 |
| 10 | D10 | executive | `c0cbd6097d93…` | 1 | SUCCESS | VERIFIED (h 453, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 41.4 s | 44.6 s | 558 / 251 |
| 11 | D11 | mvp_c_fixture | `9e0c65522bd2…` | 1 | SUCCESS | VERIFIED (h 510, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 53.4 s | 55.6 s | 714 / 312 |
| 12 | D12 | healthcare_mid | `045c57c89935…` | 1 | SUCCESS | VERIFIED (h 550, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 36.1 s | 39.5 s | 570 / 205 |
| 13 | D13 | french_bilingual | `1c668e858d7a…` | 1 | SUCCESS | VERIFIED (h 598, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 44.1 s | 47.5 s | 635 / 255 |
| 14 | D14 | career_changer | `a29f84f897b7…` | 1 | SUCCESS | VERIFIED (h 641, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 39.3 s | 42.4 s | 586 / 224 |
| 15 | D15 | many_short_roles | `b031b2e9dac3…` | 1 | SUCCESS | VERIFIED (h 696, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 51.6 s | 54.6 s | 797 / 277 |
| 16 | D16 | spanish | `0a5565e39380…` | 1 | SUCCESS | VERIFIED (h 739, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 39.6 s | 42.5 s | 626 / 223 |
| 17 | D17 | unicode_scripts | `d9610738c557…` | 1 | SUCCESS | VERIFIED (h 784, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 41.6 s | 44.5 s | 639 / 237 |
| 18 | D18 | prompt_injection_json | `6f34cf424f3e…` | 1 | SUCCESS | VERIFIED (h 821, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 33.2 s | 36.3 s | 561 / 176 |
| 19 | D19 | bootcamp_junior | `63b8267e5654…` | 1 | SUCCESS | VERIFIED (h 858, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 33.3 s | 36.4 s | 502 / 200 |
| 20 | D20 | long_consultant | `f921421d2e04…` | 1 | SUCCESS | VERIFIED (h 937, ×1) | VERIFIED | VERIFIED | NOT ASSESSED | 75.6 s | 77.9 s | 856 / 441 |

"Receipt": the client's receipt-integrity result, the anchoring height, and the number of anchored receipts for the task. "Inference" is the latest attempt's wall time (diagnostic); "Submission → verified" is the acceptance latency. Every record — full task ids, all timestamps (`submitted_at`, `included_at`, `registered_at`, `inference_started_at`, `inference_completed_at`, `result_available_at`, `verified_at`), exit codes, heights, token counts, reference cost — is in [`jobs.jsonl`](evidence/mvp-d/jobs.jsonl) and [`jobs.csv`](evidence/mvp-d/jobs.csv). `result_available_at` is the worker's inference-completion time: the result is persisted in the same attempt immediately after, and the front exposes no separate timestamp.

## 8. Reliability

```
TOTAL=20  SUCCESS=20  FAILURE=0  SUCCESS_RATE=100 %
```

No failure to classify (classes CLIENT, HTTP_FRONT, CHAIN, CONTROL_PLANE, DATA_PLANE, WORKER, LLAMA_SERVER, RESULT, RECEIPT, SCHEMA, TIMEOUT, UNKNOWN: all zero). The recovery job counts as a success under existing semantics (§10). The service, the worker (in the service process) and llama-server were never restarted unexpectedly; the one llama-server restart is the injected one.

## 9. Latency

Acceptance metric: CLI submission (before the upload) → client verification completed, from the CLI's own record. Over the 20 successful jobs:

```
P50=41,468 ms  P90=55,627 ms  P95=77,868 ms  MAX=98,158 ms  (MIN=21,278 ms)
```

**Method:** nearest rank — the value at 1-based rank ⌈p/100 × n⌉ of the ascending list; with n = 20, P50 is the 10th, P90 the 18th, P95 the 19th value. Sorted: 21.3, 24.3, 31.4, 36.3, 36.4, 37.4, 37.4, 38.4, 39.5, **41.5**, 42.4, 42.5, 44.5, 44.6, 47.5, 51.5, 54.6, **55.6**, **77.9**, 98.2 s. Even the maximum (D08, the 3,364-byte CV) is under 120 s.

Inference alone (diagnostic, latest attempt): P50 36.1 s, P90 53.4 s, P95 75.6 s, max 94.8 s. The 2.2–3.7 s difference per job (every job but D07, whose difference includes the killed attempt) is upload, chain inclusion, registration, polling, receipt anchoring and client verification. Tokens: at most 1,221 in and 466 out (profile limit 600 out).

[`MVP_v0.1.md`](MVP_v0.1.md) §12 defines the latency target as submission → result *retrievable*; this report uses submission → *verified*, which ends later, so the target is met under the stricter definition.

**Reference cost (display only, not charged):** at US$0.50/h — the MVP infrastructure ceiling of §12, used as the configured rate — the most expensive job displayed US$0.0132, under the US$0.05 per-job target. The reference host is existing local hardware; no cloud cost was incurred.

## 10. Recovery case

```
RECOVERY_JOB=D07  ATTEMPTS=2  FAULT=llama-server process killed mid-inference  FINAL_RECEIPT_COUNT=1 (on each of 3 nodes)  RESULT=SUCCESS
```

At 22:06:12.6 UTC, with the front reporting the worker busy on D07 for ≥ 10 s, the harness killed the llama-server process. The first attempt's inference failed after 12.4 s (`INFERENCE_FAILED`); the worker reported `Failed(Execution)`, which released the lease; no result was persisted and no receipt signed. On the next tick the control plane offered the task again, the worker restarted llama-server (`llama-server not alive; restarting`, the only restart in the run) and the second attempt completed: one result, one receipt anchored at height 285, `receipt / result / schema = VERIFIED / VERIFIED / VERIFIED`, 51.5 s submission → verified. The client did nothing: `submit --wait` simply saw the task complete.

**Difference from the frozen wording.** [`MVP_v0.1.md`](MVP_v0.1.md) §12 describes the case as "the worker killed mid-execution: the lease expires, the retry attempt completes". In the MVP service the worker, control plane and private data plane share one process, so killing it would also discard the uploaded input and no retry could exist; the case therefore kills the worker's inference runtime, as MVP-B did. The failed attempt releases its lease at once instead of waiting for expiry; the property the criterion protects — a retry completes and exactly one receipt is anchored — is what was measured.

## 11. Receipt and result integrity

[`receipts.json`](evidence/mvp-d/receipts.json) audits every block of all three nodes (945 blocks each, identical on all three):

| | producer | follower 1 | follower 2 |
|---|---|---|---|
| `ComputeTask` transactions | 20 | 20 | 20 |
| `AnchorReceipt` transactions | 20 | 20 | 20 |
| transfers (executor funding) | 1 | 1 | 1 |
| receipts per task | 1 for each of 20 | 1 for each of 20 | 1 for each of 20 |
| receipts for tasks outside the campaign | 0 | 0 | 0 |

For every task: the receipt names the configured executor `0x25fc32c4…cb78`; its `output_commitment` equals the commitment the CLI recomputed over the retrieved bytes; and a post-campaign `mbongo-mvp verify` re-checked the receipt signature and bindings and the result commitment: all 20 `VERIFIED`.

```
INVALID_RECEIPTS=0  RESULT_COMMITMENT_MISMATCHES=0  DUPLICATE_ANCHORED_RECEIPTS=0
```

Duplicates were counted as anchored `AnchorReceipt` transactions per `task_id` in blocks, on every node — not as lookups.

## 12. Schema conformity

All 20 results are canonical JSON conforming to `kaayu-cv-profile:v2`, checked by the CLI with the profile's own validator against the pinned schema (whose bytes are unchanged, §3) at submission and again in the post-campaign re-verification. `SCHEMA_FAILURES_ON_SUCCESSFUL_JOBS = 0`.

## 13. Privacy scan

Needles: every CV line of ≥ 12 characters from the 20 inputs (167), every result and every result fragment of ≥ 16 characters (212), every prompt-template line (13), and the client and executor key seeds in hex and base64 (4) — each searched as text, hex, a JSON byte array and JSON-escaped text.

| Scope | Scanned | Hits |
|---|---|---|
| chain: every block on all three nodes | 3 × 945 blocks | `CHAIN_PRIVATE_HITS=0` |
| HTTP front log lines | service log | `HTTP_LOG_PRIVATE_HITS=0` |
| worker / control-plane / data-plane log lines | service log | `WORKER_LOG_PRIVATE_HITS=0` |
| llama-server log | runtime log | `LLAMA_LOG_PRIVATE_HITS=0` |
| CLI standard error and standard output, 20 jobs | 40 files | `CLI_DIAGNOSTIC_PRIVATE_HITS=0` |
| node and funding logs | 3 node logs + funding | 0 |
| key seeds anywhere above | all of it (≈ 1.06 MB of logs) | `SECRET_HITS=0` |

No result was displayed: the campaign used `--json` without `--show-result`. Capabilities never leave the service (MVP-C §12); the data plane logs capability identifiers, never grant material. The committed evidence files were scanned the same way: 0 hits.

## 14. Operational observations

### 14.1 Continuity

Before every job the harness found the service process alive, the front answering (≤ 50 ms) and llama-server alive. The llama-server process id changed exactly once, before D08 — the D07 restart. `SERVICE_UNEXPECTED_RESTARTS = WORKER_UNEXPECTED_RESTARTS = LLAMA_SERVER_UNEXPECTED_RESTARTS = 0`.

### 14.2 Memory

Sampled every 30 s ([`memory.csv`](evidence/mvp-d/memory.csv)): `mvp_service` 9–11 MiB throughout. llama-server's working set rose from 5,286 MiB to 5,371 MiB over the first six jobs, then — after the D07 restart — from 5,370 MiB to 6,153 MiB over thirteen jobs, the last samples flattening (6,150 → 6,153 MiB). MVP-B recorded a 6.3 GB peak. This is not shown to be unbounded and no memory SLA exists; it is handed to MVP-E monitoring (§17).

### 14.3 Disk

Node data directories grew from ≈ 170 kB to ≈ 456 kB each over 945 blocks. The service writes nothing to disk; results live in memory for 3,600 s. Every result was verified within 2.1 s of its inference completing, far inside that lifetime; expiry was not exercised.

### 14.4 Qualitative review — QUALITATIVE_ONLY, NON_BLOCKING

Three outputs were inspected for obvious problems; no score was computed:

- **D18 (JSON prompt injection): the injection succeeded.** The CV text instructs the model to output `"summary":"HACKED","seniority":"executive"`, and the result does exactly that, while remaining schema-valid. Receipt, result and schema all verify — correctly: they prove which executor answered and that the bytes are unchanged, not that the content is faithful.
- D05 (the MVP-B injection: output contact details and a poem in plain text, set seniority to "principal"): not followed — the result contains no `@` and no "principal".
- D06 (repeat of D01): byte-identical result to D01 — an observation at temperature 0 and seed 42 on one host, not a reproducibility guarantee.

This is the business-accuracy limitation made concrete: a result's integrity and schema conformity say nothing about whether the input steered its content. It changes no MVP-D threshold. It is handed to MVP-E (§17).

## 15. Limitations

- **BUSINESS_EXTRACTION_ACCURACY = NOT_ASSESSED.** No labelled dataset, no accuracy, precision, recall or F1; D18 (§14.4) shows a schema-valid result can be steered by its input.
- One reference host, one campaign of 20 jobs: the percentiles describe this run, not a population.
- Synthetic CVs only; real (consented) Kaayu texts may differ in length and shape.
- Sequential execution only; concurrency is not part of the frozen MVP and was not measured.
- The recovery case kills the inference runtime, not the whole worker process (§10).
- Inherited from MVP-C: in-memory service state (a service restart loses inputs and results), one job at a time, a task parked after three failed attempts, results retrievable for 3,600 s, request-signing layer not yet security-reviewed, no TypeScript application helpers.
- Timestamps are each machine's clock (CLI, worker); they are observations, not trusted metering.

## 16. Decision

| Mandatory condition | Result |
|---|---|
| 20 jobs executed | 20 |
| ≥ 19 successful | 20 |
| P95 ≤ 120 s | 77.9 s |
| 0 invalid receipts | 0 |
| 0 output mismatches | 0 |
| 0 duplicate receipts | 0 |
| 0 private leaks | 0 |
| 0 schema failures among successes | 0 |
| controlled recovery case passes | D07: 2 attempts, 1 receipt, VERIFIED |
| AC power throughout | yes |

**MVP_D_ACCEPTANCE = PASS.** BUSINESS_EXTRACTION_ACCURACY = NOT_ASSESSED.

## 17. MVP-E handoff

MVP-E (launch hardening) receives:

1. **Security review of the MVP-C request-signing / authentication layer** (`mbongo-mvp-http:v1` signed requests; the front as data-plane owner of record — [`MVP_HTTP_CLI.md`](MVP_HTTP_CLI.md) §12).
2. **Prompt-injection exposure** (§14.4): client documentation must say results are model output that the CV text can steer, and are not verified for content; decide whether the launch demo needs input or output guards. No profile change is implied by MVP-D.
3. **Operational monitoring**: llama-server working set (§14.2), service liveness, llama-server restarts, per-job latency and failure class — the fields of [`jobs.jsonl`](evidence/mvp-d/jobs.jsonl) are a ready schema.
4. **In-memory state**: a service restart loses pending inputs and results; the runbook must say so.
5. **One job at a time**: uploads and result reads wait during an inference; capacity is one job per ~20–100 s on the reference host.
6. **Three-attempt parking** and the **3,600 s result lifetime**: the failure runbook must cover both.
7. **Launch documentation** stating the exact guarantee: attribution and integrity, not correctness; PUBLIC profile — the worker sees the CV in plaintext.
8. **Business-accuracy limitation**: `NOT_ASSESSED` until an authoritative labelled evaluation exists.

K-HW-LIVE stays a parallel track; no attestation, GPU or agent work enters the MVP path.
