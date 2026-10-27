# MVP Launch Runbook — controlled MVP operations

**Status:** CONTROLLED MVP OPERATIONS — NON-CONSENSUS. Gate MVP-E of [`MVP_v0.1.md`](MVP_v0.1.md).
**Operates:** the MVP measured in [`MVP_D_ACCEPTANCE.md`](MVP_D_ACCEPTANCE.md), with the hardening of [`MVP_LAUNCH_SECURITY_REVIEW.md`](../security/MVP_LAUNCH_SECURITY_REVIEW.md) (active profile `kaayu-cv-extract:v3`). **Tooling:** [`scripts/mvp-launch.mjs`](../../scripts/mvp-launch.mjs) (`verify`, `start`, `health`, `smoke`, `stop`), configuration template [`config/mvp-launch.env.example`](../../config/mvp-launch.env.example). **Client:** [`MVP_HTTP_CLI.md`](MVP_HTTP_CLI.md).

> **What this launch is.** A **CONTROLLED MVP LAUNCH**: one operator, one Mbongo-operated worker, one host, a private three-node devnet, synthetic or consented CVs, the Kaayu internal client. It is not a public, permissionless or production deployment, and nothing here should be read as one.
>
> **What Mbongo guarantees.** Execution attribution and byte integrity: the named executor signed a receipt bound to this task, this input commitment and this output commitment, and the retrieved bytes are exactly the committed ones. **It does not prove that the extracted candidate facts are correct** (`BUSINESS_EXTRACTION_ACCURACY = NOT_ASSESSED`). Results are AI-generated structured extraction; a person reviews them before any recruiting action, and no candidate is rejected, ranked or hired on this output alone.
>
> **What stays private, from whom.** The CV and the result stay **off-chain**. The Mbongo-controlled worker **sees the plaintext** (PUBLIC profile, `PRIVATE_OFFCHAIN`). There is no provider-resistant confidentiality, no TEE and no encryption from the provider in this launch.

## 1. Boundary of the controlled launch

| | |
|---|---|
| Hosts | one (the reference host: i7-8850H, 31.3 GiB, Windows 11, CPU-only inference) |
| Chain | private three-node devnet on that host (producer + two followers), all ports loopback |
| Service | one `mvp_service serve` (front + control plane + private data plane + worker), loopback `127.0.0.1:8740` |
| Inference | pinned `llama-server` b11669 (v0.7.0 @ `620d2225`) on `127.0.0.1:9931`, GPU offload 0, pinned Qwen3-4B-Instruct-2507 Q4_K_M |
| Profile | `mbongo-infer:v1 / kaayu-cv-extract:v3` — fixed; no model, runtime, profile or executor option exists |
| Clients | the Kaayu internal client on the same host, through `mbongo-mvp` |
| Concurrency | one inference at a time |
| Power | AC (latency figures assume it) |

**Network.** Every listener is loopback: node RPC and REST by default, node P2P with `--p2p-host 127.0.0.1` (set by `mvp-launch.mjs`), the front (it refuses any other bind), `llama-server` (fixed in code). `mvp-launch.mjs health` lists every listening socket of the stack and alerts on any non-loopback address. The service terminates **no TLS**: plaintext HTTP is acceptable on loopback only. A client on another host requires an external trusted boundary — a VPN, or a reverse proxy that terminates TLS and restricts clients — in front of `127.0.0.1:8740` (`TLS_REQUIRED_AT_EXTERNAL_BOUNDARY`). Do not widen a bind to reach it.

**Rate limiting.** None beyond the bounds below; the loopback boundary and a single trusted client are the control. Built-in bounds: request bodies ≤ 64 KiB (refused before authentication), ≤ 64 uploads awaiting registration, the replay store capped at 100,000 recent requests (then `503 AUTH_REPLAY_STORE_FULL`), only authenticated requests recorded. A flood of status requests is answered (300 in a few seconds in the test group) and does not block the worker; a flood of bad signatures leaves no state.

## 2. Build and provenance

```bash
git fetch origin && git checkout <reviewed SHA>          # the merged, reviewed commit
cargo build --release --locked -p mbongo-node --bin mbongo-node
cargo build --release --locked -p mbongo-compute --bin mvp_service --bin mbongo-mvp
scripts/setup-mvp-inference-runtime.sh                     # once: pinned llama.cpp + model into ~/.cache/mbongo/inference
```

| Artifact | Provenance | Verified by |
|---|---|---|
| `mbongo-node`, `mvp_service`, `mbongo-mvp` | built from the reviewed SHA with `--locked` | `mvp-launch.mjs verify` (clean tree, SHA, binary hashes recorded) |
| `llama-server` | upstream release `llama-b11669-bin-win-cpu-x64.zip`, archive SHA-256 `37b3efcf…632b` | executable SHA-256 `6a4635ae…141b` (`verify`, and the worker at every start) |
| model | `unsloth/Qwen3-4B-Instruct-2507-GGUF` @ `a06e946b`, 2,497,281,120 bytes | SHA-256 `3605803b…e597` (`verify`, and the worker at every start) |
| profile v3 | `crates/mbongo-compute/profiles/kaayu-cv-extract-v3/` | prompt SHA-256 `87be4340…0b3e`, schema `0a81727a…50ab` against [`kaayu-cv-extract-v3.json`](../../test-vectors/inference/kaayu-cv-extract-v3.json); the worker refuses to start on any other digest |

## 3. Configuration and keys

Copy [`config/mvp-launch.env.example`](../../config/mvp-launch.env.example) **outside the repository** and fill it. It holds settings and key-file **paths**; keys never appear in it, in a command line or in a log. The full inventory is in [`MVP_LAUNCH_SECURITY_REVIEW.md`](../security/MVP_LAUNCH_SECURITY_REVIEW.md) §6.

Key files: 64 hex characters (a 32-byte Ed25519 seed), one per role, on an encrypted volume, outside the repository, readable by the operator account only. Create the client and executor keys fresh for the launch:

```bash
node scripts/mvp-launch.mjs keygen <dir>/client.key       # random seed, operator-only permissions, never printed
node scripts/mvp-launch.mjs keygen <dir>/executor.key
mvp_service executor-address                                 # with MBONGO_MVP_EXECUTOR_KEY_FILE set: the address for MBONGO_MVP_EXECUTOR
```

**The documented devnet seeds (`0xAA…`, `0xE1…`, …) are public** — they are in the repository. `verify` refuses any of them as the client or the executor key: a public client key would let any local process act as the client, a public executor key would let it sign receipts as the worker. The funder may be the genesis-funded devnet account (`0xAA…`): it only pays for the two accounts. `verify` also checks that `MBONGO_MVP_EXECUTOR` is the executor key's address. Back the keys up offline, encrypted, once. Do not print the environment or a key file in a terminal that is recorded.

## 4. Controlled-launch checklist

Print it; tick it; keep it with the run log.

```
[ ] reviewed SHA checked out, tree clean, binaries built --locked     (mvp-launch.mjs verify)
[ ] runtime and model digests verified                                 (verify)
[ ] active profile kaayu-cv-extract:v3, prompt/schema digests verified  (verify)
[ ] fresh client and executor keys (keygen), outside the repo, operator-only;
    not a public test seed; MBONGO_MVP_EXECUTOR matches the executor key     (verify)
[ ] laptop on AC power
[ ] 3 nodes started, converged: same block at the common height, skew ≤ 3   (start / health)
[ ] every listener loopback (no NON_LOOPBACK_LISTENER alert)            (health)
[ ] mvp_service ready: FRONT_BIND=127.0.0.1:8740, runtime/model lines printed
[ ] llama-server healthy                                               (health)
[ ] no parked tasks, no auth replays, no alerts                        (health)
[ ] launch smoke passes: synthetic job VERIFIED×3 + NOT ASSESSED; adversarial
    job not overridden, screen CLEAR; replay/tamper refused; privacy scan 0   (smoke)
[ ] human-review step in the Kaayu flow confirmed for every extracted profile
```

## 5. Startup

```bash
node scripts/mvp-launch.mjs verify --config <env>           # 1–2: SHA, tree, binaries, runtime, model, profile
node scripts/mvp-launch.mjs start  --config <env> --fund    # 3–8: devnet, convergence, executor account, service
node scripts/mvp-launch.mjs health --config <env>           # every check green
node scripts/mvp-launch.mjs smoke  --config <env>           # 9: launch smoke
```

`start` refuses if `verify` fails; starts the producer, reads its PeerId, starts two followers bootstrapped from it (all ports loopback); waits until the three nodes hold the identical block at height ≥ 3; with `--fund`, creates the executor and client accounts from the funder (needed once per fresh chain: an account must exist before it sends, RFC 0002 rule d); starts `mvp_service serve`, which verifies the runtime, model and profile digests itself and prints `STATE=IN_MEMORY` with the list of §7. `--fund` is for a fresh devnet only.

## 6. Health, metrics and alerts

`mvp-launch.mjs health` prints one JSON report and exits non-zero on any alert. It reads nothing private.

| Check | Healthy | Alert |
|---|---|---|
| nodes live | all three answer `get_block_height` | `NODE_DIVERGENCE_OR_DOWN` |
| nodes converged | identical block at the common height, height skew ≤ 3 | `NODE_DIVERGENCE_OR_DOWN` → **stop new submissions** until converged |
| front | an unsigned request answers `401 {"error":"AUTH_MISSING"}` | `HTTP_FRONT_DOWN` |
| worker / llama-server | `GET 127.0.0.1:9931/health` ok | `LLAMA_SERVER_UNAVAILABLE` (the worker restarts it before the next job; alert if it persists) |
| service process | running | `MVP_SERVICE_NOT_RUNNING` (in-memory state lost, §7) |
| exposure | every listener of the stack on loopback | `NON_LOOPBACK_LISTENER` → stop, fix before launch |
| memory | llama-server working set ≤ 7,168 MiB | `LLAMA_MEMORY_ABOVE_7168_MIB` |
| disk | ≥ 5 GiB free on the state volume | `DISK_FREE_BELOW_5_GIB` |
| parked tasks | 0 | `PARKED_TASKS` |
| auth replays | 0 | `AUTH_REPLAYS` → investigate the source |

**Metrics** (from the service log, which carries no content): requests and responses by status and code; registrations; receipts observed; inference outcomes by contract code; failed attempts; parked tasks; authentication failures and replays; `llama-server` restarts; worker errors. Per job, the client's `submit --wait --json` record carries end-to-end and inference latency, attempts and the four verification results — the MVP-D record schema. Receipt-integrity, result-commitment and schema failures are client-side: alert on any `outcome` other than `VERIFIED`, any exit 10 or 11, and any exit 12 (screen flagged).

**Memory policy.** MVP-B peaked at 6.3 GB; MVP-D saw 5.3 → 6.15 GB, flattening, after a restart. That is not shown to be a leak and no automatic restart is configured. The alert threshold, 7,168 MiB, is ~14 % above the highest observation. On the alert, **between jobs** (status shows no task executing), stop `llama-server`; the worker restarts it before the next job — the recovery path MVP-D measured (D07). Never during an inference.

## 7. In-memory state and restarts

The service holds this state **only in process memory**; stopping or crashing it loses it. Nothing is written to disk.

| State | On restart | Class |
|---|---|---|
| uploaded private inputs | lost | LOST_BUT_ACCEPTABLE — the client re-submits a new task |
| persisted private results | lost; cannot be recovered | LOST_BUT_ACCEPTABLE — receipts stay on-chain and verifiable |
| task registrations, input references | lost; tasks registered before are never executed | LOST_BUT_ACCEPTABLE |
| leases, attempts, parked tasks | lost; counts reset | LOST_BUT_ACCEPTABLE |
| status snapshots, inference timings | lost; earlier tasks answer `TASK_NOT_FOUND` | LOST_BUT_ACCEPTABLE |
| request replay history | lost; a captured request is replayable only inside its ±120 s window | LOST_BUT_ACCEPTABLE (`REPLAY_DURABILITY = PROCESS_LIFETIME_ONLY`) |
| front owner key, control-plane issuer key | regenerated at start | RECONSTRUCTIBLE |
| chain state | on disk in the node data directories | durable |

None is launch-blocking for a controlled launch with this procedure: **finish or abandon every job before stopping the service**; after any restart, clients re-submit (a new task; the CLI never resubmits by itself). There is no backup of private data, by design: there is none on disk.

## 8. Operating limits

- **One job at a time.** One inference runs; other uploads and result reads wait for it (status does not). Capacity on the reference host: one job per ~20–100 s.
- **Parking.** A task whose attempts fail three times is *parked*: never offered again. The operator sees it in `health` (`PARKED_TASKS`), in the service log (`task <id> parked after 3 failed attempts`) and in the client's status (`state: failed`, `attempts_exhausted: true`, failure code). There is no un-park: inspect the failure code, fix the cause, and have the client submit a new task.
- **Result lifetime: 3,600 s** after the result is persisted. After it, the private result is gone and cannot be recovered: `GET …/result` answers `410 RESULT_EXPIRED`; `verify` exits `8` and says the receipt is unaffected — the anchored receipt remains on-chain and its integrity still verifies.
- **Duplicate submission.** `submit` is not idempotent: each run creates a new task (new salt, new `task_id`), a new inference and a new receipt. The CLI never resubmits on its own; a client retries **status and result**, never submit.
- **Timeouts.** HTTP client 600 s; inference 180 s; `submit --wait` 900 s (configurable); worker start-up 120 s; lease 900 s.

## 9. Shutdown

```bash
node scripts/mvp-launch.mjs stop --config <env>
```

Stops the service, `llama-server`, then the nodes. **Every uploaded input, every unretrieved result, every registration and every parked task held by the service is lost at this point** (§7). Before stopping: no job executing, every result retrieved and verified. Chain data survives in the state directory; a fresh launch can reuse it or start a new devnet.

## 10. Incident runbook

| Event | Severity | Immediate action |
|---|---|---|
| **receipt invalid** (signature, task, executor or input binding — exit 10, receipt `FAILED`) | HIGH, launch-blocking | **stop the service; do not retry as normal.** Keep the task id, receipt height and logs; investigate before any new submission |
| **result commitment mismatch** (result `FAILED`, exit 10) | HIGH, launch-blocking | same: stop, preserve evidence, investigate |
| **private data in a log or on-chain** | launch-blocking privacy incident | see §12 |
| node divergence | HIGH | stop new submissions; inspect the three nodes' heights and tips; restart the follower that diverged; resume only when `health` is green |
| `llama-server` crash | MEDIUM | none needed for one occurrence: the attempt fails, the worker restarts it, the task retries (MVP-D D07). Repeated → stop, inspect `~/.cache/mbongo/inference/logs/llama-server.log` |
| inference timeout | MEDIUM | the attempt fails and is retried; three failures park the task (§8) |
| task parked | MEDIUM | read its failure code; fix the cause; client submits a new task |
| HTTP front down | HIGH | `mvp_service` exited: restart it (§5); in-memory state is lost (§7); clients re-submit |
| result expired | LOW | expected after 3,600 s; the receipt stays verifiable; re-run the job if the result is still needed |
| auth replay detected (`AUTH_REPLAYED`) | MEDIUM | the request was refused; identify the source (the log line names the signer prefix); if it is not the known client, treat it as a key compromise (§11) |
| screen flagged (exit 12) | MEDIUM | do not use the result; a person reviews the CV; the injection corpus may need the case (security review §3) |

## 11. Key compromise

1. Stop the affected service (`stop`); stop new submissions.
2. Generate a new seed for the compromised role on the operator's machine; store it as in §3; never transmit it.
3. **Client key**: configure the new key file; the old address can no longer act through the front for new tasks. **Executor key**: a new executor address is a new provider identity: fund it (`fund-executor` with the new key file), update `MBONGO_MVP_EXECUTOR` for clients, restart; old tasks naming the old executor will never be answered by the new one. **Funder key** (devnet): move remaining funds if needed, replace.
4. Audit: the service log for requests signed by the old address (`signer <prefix>`), the chain for transactions sent by it, receipts anchored by the old executor after the compromise time.
5. Record what was found; re-issue credentials to the client.

No PKI or automated rotation exists or is needed for a single-operator launch.

## 12. Privacy incident

If a CV fragment, a result, the prompt or a key appears in any log, CLI diagnostic, chain block or artifact: **launch-blocking incident.**

1. Stop the service and the client; stop new submissions.
2. Contain: restrict access to the affected files and the state directory; do not copy them further; for chain data, note the heights (a private devnet can be reset; a block cannot be edited).
3. Identify the source code path and the first occurrence; keep the evidence privately.
4. Fix, add the case to the privacy tests, re-run the smoke's privacy scan to 0 before resuming.
5. If real candidate data was involved, follow Kaayu's data-protection process.

## 13. Backups

Chain data: the node data directories under the state directory; copy them with the nodes stopped (the persistent-devnet procedure is [`DEVNET_V0.4_OPERATIONS.md`](../runbooks/DEVNET_V0.4_OPERATIONS.md)). Private inputs and results: **no backup exists** — they live only in the service's memory (§7). Keys: one offline encrypted copy (§3).

## 14. Smoke evidence

The launch smoke of the MVP-E gate is recorded in [`MVP_LAUNCH_SECURITY_REVIEW.md`](../security/MVP_LAUNCH_SECURITY_REVIEW.md) §12.
