# MVP HTTP Front and CLI — `mbongo-mvp`

**Status:** MVP v0.1 IMPLEMENTATION — NON-CONSENSUS. Gate MVP-C of [`MVP_v0.1.md`](MVP_v0.1.md).
**Builds on:** [`MVP_INFERENCE_CONTRACT_v1.md`](MVP_INFERENCE_CONTRACT_v1.md), the active profile [`kaayu-cv-extract:v2`](KAAYU_CV_EXTRACT_PROFILE_v2.md) and the CPU worker of [`MVP_CPU_INFERENCE_WORKER.md`](MVP_CPU_INFERENCE_WORKER.md), none of which this gate changes. **Code:** `crates/mbongo-compute/src/mvp/`, binaries `src/bin/mvp_service.rs` and `src/bin/mbongo_mvp.rs`. **CI group:** `mvp_http_cli` (step "Mbongo MVP HTTP + CLI"). **Vector:** [`test-vectors/mvp/mvp-http-cli-v1.json`](../../test-vectors/mvp/mvp-http-cli-v1.json). **Fixture:** `crates/mbongo-compute/fixtures/synthetic-cv.txt` (synthetic, no real person).

> **What a successful run lets Mbongo say — and not say.** *The anchored receipt and the retrieved result bytes verify cryptographically* (receipt integrity, result integrity). *The result conforms to the pinned application schema* (schema conformity). These are reported separately. *The business or factual accuracy of the extraction has not been demonstrated*: it is reported as `NOT ASSESSED`, always.

## 1. Purpose

Give the MVP's first client one supported path through the frozen workload: prepare a `kaayu-cv-extract:v2` request, upload the private input off-chain, commit the `ComputeTask` on-chain, register it, follow its status, retrieve the private canonical result, resolve the anchored receipt, and verify — client-side — the receipt, the result bytes and the schema, each on its own. It is a thin adapter over operations that already exist; it is not a public API platform.

## 2. Scope

**In:** one loopback HTTP front (four routes), one service binary hosting the front with the in-process control plane (E), private data plane (F) and the MVP-B CPU worker, one CLI (`submit`, `status`, `result`, `verify`), client-side verification, safe status and timing display, a display-only reference cost.

**Out:** anything in §17. The 20-job acceptance run, its P95 and its reliability figure belong to MVP-D (§18).

## 3. Authority boundaries

| Fact | Authority | The front |
|---|---|---|
| `task_id`, submitter, executor, `execution_spec`, `input_commitment` | the chain (the committed `ComputeTask`, RFC 0005) | reflects them; never sets, chooses or rewrites them |
| leases, attempts, lifecycle state | the control plane (E) | reports its state |
| private input and result bytes; whether a result exists | the private data plane (F) | reads them on each request; keeps no copy |
| the anchored receipt | the chain | does not serve receipts; the CLI reads them from blocks |
| who may act on a task | the chain: its `submitter` | checks a per-request signature by that account's key |

**Owner of record.** F's operations are keyed by local signing keys and E/F are deliberately transport-free (E §19, F §5.3). A remote client cannot hold a data-plane key, so the front holds one and is the data-plane owner of the objects it stores; the single-use grants it issues to itself never leave the process. The front decides nothing the chain decides: it refuses to register a task unless (a) the caller signed the request with the task's `submitter` key, (b) the task's `execution_spec` is the frozen MVP spec, (c) the task names the executor this deployment runs, and (d) the uploaded input's commitment equals the task's `input_commitment`.

**Chain operations** use the crate's `rpc_v0.3` client (`chain::RpcChainClient`): `submit_transaction` for the `ComputeTask`, `get_block_by_height` to find the task and the receipt, REST `/accounts/{address}` for the nonce. These are the methods `@mbongo/sdk` 0.2.0 calls; the SDK itself is unchanged (`SDK_WIRE_CHANGE = NO`). The CLI is a Rust binary, so it composes the Rust client over the same wire rather than spawning Node; receipt signatures are checked by `mbongo_verification::verify_receipt_signature`, the function the node itself uses.

## 4. HTTP routes

Base `http://127.0.0.1:8740` by default (`HTTP_FRONT_DEFAULT_BIND = 127.0.0.1`). `/v1` is the HTTP API version and is independent of the profile version. No CORS policy, no other route, no proxy to `llama-server`.

| Route | Existing operation | Request | Success |
|---|---|---|---|
| `PUT /v1/inputs` | F §4.1 `store_input` | `{"task_id","input":{"contract":"kaayu-cv-extract-input:v1","cv_text"}}` | `201` `{"task_id","object_ref","input_commitment","input_bytes"}` |
| `POST /v1/tasks/{task_id}/register` | F `register_task` + `delegate_issuer`; E `register_input` | `{"object_ref","executor"}` | `200` status view |
| `GET /v1/tasks/{task_id}` | E task record; F `result_ref` | — | `200` status view |
| `GET /v1/tasks/{task_id}/result` | F §10 `get_result` under a single-use get-result grant | — | `200` the exact canonical result bytes |

- **Upload.** The front validates the input object with the worker's own type (`CvExtractionInputV1`: non-empty, no control characters other than `\n` `\r` `\t`, ≤ 10,000 canonical bytes), computes `input_commitment = BLAKE3("mbongo:compute-input:v1" ‖ canonical input object)` with the worker's own function, and stores the canonical bytes. The `task_id` is the one the client computed for the task it is about to submit — the chain's derivation, never a front-made id. `object_ref` is an identifier, not a capability: it grants nothing. Re-uploading before registration replaces the earlier object.
- **Register** takes the `object_ref` and the `executor` the MVP v0.1 §9 surface names, and only checks them against what exists. It is idempotent.
- **Status view** fields: `task_id`, `state`, `task_height`, `attempts`, `attempts_exhausted`, `last_failure`, `failure_code`, `uploaded_at`, `registered_at`, `started_at_ms`, `completed_at_ms`, `inference_ms`, `input_tokens`, `output_tokens`, `finish_reason`, `result_available`, `receipt_submitted`, `receipt_height`, `worker_busy`, `as_of`. `state` is `not_registered` before registration, then the control plane's own states: `queued` (E `Discovered`), `leased`, `input_authorized`, `input_consumed`, `executing`, `result_persisted`, `receipt_submitted`, `completed` (a receipt observed on-chain), `failed`. While the worker is mid-attempt it holds the front's state; status is then served from the last snapshot with `worker_busy: true`. Timestamps and token counts are observations, not metering.
- **Result.** `Content-Type: application/octet-stream`, `Cache-Control: no-store`; the body is the canonical bytes exactly as the data plane holds them (≤ 8,000 bytes), never parsed or re-serialized. Those bytes, and only those, are what `output_commitment` covers.
- **Bounds.** Request bodies over 64 KiB are refused (`413`) before a handler reads them; an input over the contract's 10,000 canonical bytes is refused (`413 INPUT_TOO_LARGE`) before anything is stored; at most 64 uploads await registration.

## 5. CLI commands

```text
mbongo-mvp submit <cv.txt | -> [--wait] [--timeout <s>] [--show-result] [--json]
mbongo-mvp status <task_id> [--json]
mbongo-mvp result <task_id> [--out <file>] [--show-result]
mbongo-mvp verify <task_id> [--from-height <h>] [--json] [--show-result]
```

Configuration is minimal and comes from the environment (or the matching flag): `MBONGO_MVP_FRONT`, `MBONGO_MVP_RPC_URL`, `MBONGO_MVP_REST_URL`, `MBONGO_MVP_EXECUTOR` (the configured executor address), `MBONGO_MVP_KEY_FILE` (a file holding the client key seed; the key is never an argument), and optionally `MBONGO_MVP_REFERENCE_USD_PER_HOUR`. There is no `--model`, `--runtime`, executor-selection or `execution_spec` option: the CLI always constructs `mbongo-infer:v1 / kaayu-cv-extract:v2`. Input is plain UTF-8 text from a file or standard input; no PDF, DOCX, OCR or URL.

The service: `mvp_service serve [--bind 127.0.0.1:8740]` (refuses a non-loopback address), `mvp_service executor-address`, and the devnet operator step `mvp_service fund-executor --amount <n>` (the executor's account must exist before it can anchor, RFC 0002 rule d). Keys come from `MBONGO_MVP_EXECUTOR_KEY_FILE` and `MBONGO_MVP_FUNDER_KEY_FILE`.

## 6. Submit flow

1. Read the CV text; validate it as `kaayu-cv-extract-input:v1`; compute `input_commitment`.
2. Build the task: `version`, `submitter` = the client key's address, `executor` = the configured executor, a random 32-byte `salt`, `input_commitment`, `execution_spec` = `"mbongo-infer:v1" ‖ f38886b2…ac6f` (47 bytes). `task_id` is `ComputeTask::task_id()`.
3. `PUT /v1/inputs`; refuse to continue if the front's `input_commitment` differs from the local one.
4. Submit the signed `ComputeTask` transaction at the account's next nonce; wait until a block carries it.
5. `POST /v1/tasks/{task_id}/register`, retrying only while the front has not yet observed the block (`404 TASK_NOT_ON_CHAIN`).
6. Print `Task: <task_id>`. With `--wait`: poll status (bounded by `--timeout`, default 900 s), then run §9.

A retry never submits a second task: polls and reads are retried, submission is not.

## 7. Status flow

`status` prints the state, attempts, last failure, inference duration, finish reason and token counts, whether a result exists, and whether a receipt has been observed — and says the receipt is **not verified** by this command. A task the front stops retrying (`attempts_exhausted`, after 3 failed attempts) is reported `failed`; `submit --wait` then exits `7`.

## 8. Result flow

`result` retrieves the exact bytes, prints their length and their recomputed output commitment to standard error — stating that it has **not** compared them with the receipt — and writes them only where asked (`--out <file>` byte for byte, or `--show-result`). Integrity is `verify`'s job.

## 9. Receipt and result verification

Frozen order, all client-side:

1. retrieve the committed `ComputeTask` from the chain;
2. check it hashes to the requested `task_id`, and that it is the frozen MVP task;
3. resolve the anchored receipt from the chain;
4. verify the receipt's executor signature;
5. check `receipt.task_id` = `task_id`;
6. check `receipt.executor` = `task.executor` (and = the configured executor);
7. check `receipt.input_commitment` = `task.input_commitment`;
8. retrieve the private result bytes from the front;
9. recompute `BLAKE3("mbongo:compute-output:v1" ‖ bytes)` over **those exact bytes**;
10. compare with `receipt.output_commitment`;
11. parse the bytes;
12. validate them against `kaayu-cv-profile:v2` and check they are canonical.

The report keeps four results apart — never one boolean:

| Field | `VERIFIED` means | It does not mean |
|---|---|---|
| `receipt_integrity` | an anchored receipt exists, its signature verifies, and it is bound to this task, its named executor and its input commitment (steps 3–7) | anything about the result bytes |
| `result_integrity` | the retrieved bytes are exactly the bytes the receipt commits to (steps 8–10) | that they are well-formed |
| `schema_conformity` | the bytes are canonical JSON conforming to `kaayu-cv-profile:v2` (steps 11–12) | that the extracted facts are correct |
| `business_accuracy` | — always `NOT ASSESSED` (§11) | — |

Each is `VERIFIED`, `FAILED (reason)`, `NOT FOUND (reason)` or `NOT ASSESSED`. A receipt that fails makes `result_integrity` fail too: its output commitment cannot be trusted. Execution status is a separate line: a task the control plane calls `completed` with no valid anchored receipt reports `Status: completed` and `Receipt integrity: NOT FOUND` / `FAILED`, never "verified".

Presentation (`verify`, and `submit --wait`):

```text
Task: <task_id>
Status: completed
Receipt integrity: VERIFIED
Result commitment: VERIFIED
Schema conformity: VERIFIED
Business extraction accuracy: NOT ASSESSED
Executor: 0x<address>
Runtime profile: mbongo-infer:v1 / kaayu-cv-extract:v2
Receipt anchored at height: <h>
Duration: inference <s>; submission to verified <s>
Reference cost: US$<x> (display only, not charged)
Business accuracy has not been evaluated by Mbongo: …
Provider visibility: the Mbongo-operated worker processes the CV in plaintext (PUBLIC profile); …
OUTCOME=VERIFIED
```

`--show-result` then prints the profile, pretty-printed *for display* from the verified canonical bytes and labelled as such; no commitment is ever computed over that rendering. `--json` prints one JSON object with the same fields (`{"status","reason"}` each), `execution_status`, `outcome`, timings and the reference cost.

The reference cost is the MVP v0.1 §8 rule — inference seconds × the configured hourly rate — printed only when a rate is configured; nothing is charged.

## 10. Schema conformity

`schema_conformity` runs `kaayu_v2::parse_output` (the profile's own validator: the v2 schema shape, enums, bounds) and the MVP-A canonical-bytes check over the retrieved bytes. It is computed whether or not integrity verifies, and neither implies the other: the test group proves a correctly committed but schema-invalid result is `receipt VERIFIED / result VERIFIED / schema FAILED` (exit `11`), and a schema-valid result that is not the committed bytes is `schema VERIFIED / result FAILED` (exit `10`).

## 11. Business-accuracy limitation

Mbongo does not evaluate whether the model found the right skills, years or employers, or invented anything. There is no evaluator, no score and no evaluation dataset in MVP-C; `business_accuracy` is `NOT ASSESSED` in every report, and that is not an error. The receipt proves that the named executor signed a receipt bound to this task, this input commitment and this output commitment ([`MVP_v0.1.md`](MVP_v0.1.md) §13: `OUTPUT_CORRECTNESS_PROVEN = NO`). No text in the CLI says "verified inference" or "AI result verified".

## 12. Authentication and capabilities

Every request is signed by the client's chain key — the key that signs its `ComputeTask` — over `"mbongo-mvp-http:v1\n" METHOD "\n" PATH "\n" TIMESTAMP "\n" NONCE "\n" hex(BLAKE3(body))`, in headers `x-mbongo-address`, `x-mbongo-timestamp`, `x-mbongo-nonce` (16 random bytes) and `x-mbongo-signature`. The front accepts ±120 s of clock skew and refuses a signature it has seen in that window. Only the task's on-chain `submitter` may register it, read its status or retrieve its result. This is not an account system: it adds no identity beyond the chain account, and it is the smallest scheme that lets the existing E/F authorization decide over HTTP. No secret is ever sent.

Capabilities never leave the service: the fetch and put grants are the worker's (E/F, unchanged), and the get-result grant is single-use, 30 s, issued by the front to itself and consumed in the same request. Nothing prints one. The client key is read from a file named by an environment variable, never from the command line.

## 13. Privacy and logging

The service logs one line per request — method, route template, task-id prefix, status, error code, duration — and nothing from any body; no framework middleware logs requests. It never logs CV text, the input body, the prompt, result JSON, a capability or a key. Error bodies are `{"error":"<CODE>"}` and nothing else; the CLI's errors carry the code or a transport class. `Debug` of the request types redacts the CV. The CLI prints CV text never, and result content only when asked, after the integrity report.

**Provider visibility.** This is the PUBLIC profile (`MVP_PRIVACY_LEVEL = PRIVATE_OFFCHAIN`): the Mbongo-operated worker processes the CV in plaintext. The input and the result stay off-chain — the smoke scans every block for them (§16).

**Retention, as implemented.** Everything lives in the service process's memory and is lost when it stops; nothing is written to disk. An uploaded input expires 3,600 s after upload and is consumed by the worker's single-use fetch. A result is retrievable for the data plane's object lifetime — the put grant's, 3,600 s here — then the front answers `410 RESULT_EXPIRED`. There is no deletion endpoint and no other deletion promise.

## 14. Error model

| HTTP | Code | When |
|---|---|---|
| 400 | `INVALID_INPUT_FORMAT` | the upload body is not the input contract (bad JSON, unknown field, wrong contract, empty text, control character) |
| 400 | `INVALID_TASK_ID`, `INVALID_OBJECT_REF`, `INVALID_EXECUTOR`, `INVALID_REQUEST` | malformed identifiers or register body |
| 401 | `AUTH_MISSING`, `AUTH_MALFORMED`, `AUTH_STALE`, `AUTH_REPLAYED`, `AUTH_BAD_SIGNATURE` | request signature |
| 403 | `NOT_TASK_SUBMITTER` | signed, but not by the task's submitter |
| 404 | `TASK_NOT_ON_CHAIN` (register), `TASK_NOT_FOUND`, `TASK_NOT_REGISTERED`, `INPUT_NOT_UPLOADED` | unknown task or missing upload |
| 409 | `RESULT_NOT_READY`, `TASK_ALREADY_REGISTERED` | task incomplete; re-upload after registration |
| 410 | `RESULT_EXPIRED` | the result outlived its data-plane lifetime |
| 413 | `INPUT_TOO_LARGE` (or a bare 413 over 64 KiB) | size bounds |
| 422 | `UNSUPPORTED_EXECUTION_SPEC`, `EXECUTOR_MISMATCH`, `EXECUTOR_NOT_SERVED`, `OBJECT_REF_MISMATCH`, `INPUT_COMMITMENT_MISMATCH` | the committed task is not one this front may register |
| 429 | `TOO_MANY_PENDING_UPLOADS` | more than 64 unregistered uploads |
| 5xx | `CHAIN_UNAVAILABLE`, `REGISTRATION_FAILED`, `RESULT_UNAVAILABLE`, `INTERNAL_ERROR` | internal; no detail is returned |

CLI exit codes: `0` success (for `verify`: profile, receipt, result and schema all `VERIFIED`); `2` usage; `3` local configuration (key file, executor, input refused); `4` front or chain unreachable; `5` refused by the front or the chain; `6` not ready (no result, no anchored receipt, wait timed out); `7` task failed and no longer retried; `10` `VERIFICATION_FAILED` (task, receipt or result integrity); `11` integrity verified, schema conformity failed. `business_accuracy` never changes the code.

## 15. Golden vectors

`mvp_http_cli` test C24 checks, byte for byte:

- the frozen profile: job-spec digest `f38886b2…ac6f` and the 47-byte `execution_spec`, against [`kaayu-cv-extract-v2.json`](../../test-vectors/inference/kaayu-cv-extract-v2.json) — the vector MVP-B.1 committed, not regenerated here;
- the MVP-A input and output vectors (canonical input bytes, `input_commitment`, `output_commitment`) through the MVP-C code paths;
- [`mvp-http-cli-v1.json`](../../test-vectors/mvp/mvp-http-cli-v1.json): the synthetic fixture's SHA-256, canonical length and `input_commitment`; a fixed task (seeds, salt) and its `task_id`; one signed request (signing message and signature). This vector was produced by the Rust implementation (`generate_mvp_vector`) and pins it against drift; no second implementation exists yet.

No TypeScript helper is added in MVP-C, so there is no cross-language result to report; a future TypeScript client must reproduce these values exactly.

## 16. Smoke-test evidence

One real synthetic job end to end, on the reference host, on AC power: the CLI → HTTP upload → `ComputeTask` on a live local producer node → registration → the MVP-B worker on the pinned `llama-server` and model → canonical result → anchored receipt → client verification; then a second job through `submit --wait --json` (the MVP-D record form). This is a smoke proof, not MVP-D's measurement.

*(Recorded in the MVP-C pull request; see §16.1.)*

## 17. Non-goals

No 20-job acceptance run; no claim about the factual accuracy of an extraction; no GPU, agents, ODS, DeepInfer, vLLM; no marketplace, MBO, billing or trusted metering; no web UI, CORS or browser client; no PDF, DOCX, OCR or URL input; no alternate model, runtime or executor; no task cancellation; no persistent client database; no websocket or event stream; no protocol, RPC, SDK-wire, consensus, profile or worker change.

## 18. MVP-D handoff

MVP-D adds measurement, not orchestration. Against one running `mvp_service serve`, on AC power:

```bash
for i in $(seq 1 20); do mbongo-mvp submit crates/mbongo-compute/fixtures/synthetic-cv.txt --wait --json >> mvp-d.jsonl; echo "exit=$?" >> mvp-d.exit; done
```

Each line is one job: `task_id`, `execution_status`, `receipt_integrity`, `result_integrity`, `schema_conformity`, `business_accuracy`, `outcome`, `attempts`, `failure_code`, `task_height`, `receipt_height`, token counts, `reference_cost_usd`, and `timings` — `submitted_at_ms`, `task_included_at_ms`, `registered_at_ms`, `inference_started_at_ms`, `inference_completed_at_ms`, `inference_ms`, `completed_at_ms`, `verified_at_ms`, `total_ms`. Success is exit `0` with `outcome = VERIFIED`; every other exit code is a classified failure (§14). The same profile, model and runtime are guaranteed by construction: the CLI cannot choose others and the service refuses to start on anything but the pinned ones.

Known limits MVP-D inherits: one job runs at a time and holds the service state during inference (uploads and results wait; status does not); state is in memory only; a task that fails three times is parked, not retried.
