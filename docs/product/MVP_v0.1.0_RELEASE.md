# Mbongo MVP v0.1.0 — release record

**Status:** CURRENT (release record, non-consensus). Records what was released and what it does and does not provide. It decides nothing new: scope is decided by [`MVP_v0.1.md`](MVP_v0.1.md), operation by [`MVP_LAUNCH_RUNBOOK.md`](MVP_LAUNCH_RUNBOOK.md), the client surface by [`MVP_HTTP_CLI.md`](MVP_HTTP_CLI.md), security by [`MVP_LAUNCH_SECURITY_REVIEW.md`](../security/MVP_LAUNCH_SECURITY_REVIEW.md). Where this record and one of those disagree, that document wins and this one is wrong.
**Next phase:** observation only — [`POST_MVP_KAAYU_OBSERVATION.md`](POST_MVP_KAAYU_OBSERVATION.md) (O1).

## 1. Identity

| | |
|---|---|
| Version | `mvp-v0.1.0` (annotated tag) |
| Release commit | `099b8c0832f9c8e3f6bf45150fa0064358a21969` |
| Release tree | `527f0bc9574e69f91ea0efb614897e73212f074c` |
| `main` at release | `fbfe5f607141906d3f0cb599e2042351463bc9bb` (merge commit; same tree as the release commit) |
| Protocol | v0.4, frozen ([`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md)); the MVP changed no protocol surface |
| Launch | `CONTROLLED_MVP_LAUNCH = COMPLETE`: controlled-launch gate [#164](https://github.com/MbongoChain/mbongo-chain/issues/164) passed on the release commit, 2026-11-02 |
| Tracking | [#148](https://github.com/MbongoChain/mbongo-chain/issues/148) (closed), milestone *MVP v0.1* (closed) |

The tag targets the release commit, not the `main` merge commit; both carry the same tree. CI evidence: push CI on the release commit and pull-request CI on the `main` commit both passed (details in the #164 publication comment).

## 2. Runtime contract — what the release does

- **Workload.** One single-turn extraction: a candidate CV, as plain text, to the JSON profile `kaayu-cv-profile:v2`, through the inference profile `mbongo-infer:v1 / kaayu-cv-extract:v3` ([`KAAYU_CV_EXTRACT_PROFILE_v3.md`](KAAYU_CV_EXTRACT_PROFILE_v3.md); change controls CC-2 and CC-3 of [`MVP_v0.1.md`](MVP_v0.1.md) §20).
- **Worker.** One Mbongo-operated CPU worker: pinned llama.cpp `llama-server` b11669 (v0.7.0 @ `620d2225`) and the pinned Qwen3-4B-Instruct-2507 Q4_K_M model (SHA-256 `3605803b…e597`), verified at every start ([`MVP_CPU_INFERENCE_WORKER.md`](MVP_CPU_INFERENCE_WORKER.md)). One inference at a time.
- **Interfaces.** A signed HTTP front (loopback) and the `mbongo-mvp` CLI: `submit`, `status`, `result`, `verify` ([`MVP_HTTP_CLI.md`](MVP_HTTP_CLI.md)).
- **On-chain.** The `ComputeTask` commitment and the executor-signed `AnchorReceipt` binding task, input commitment and output commitment.
- **Off-chain.** The CV and the result. They never touch the chain.
- **Who sees plaintext.** The Mbongo-controlled worker processes the CV in plaintext (PUBLIC profile). **Provider-resistant confidentiality is not provided.**
- **Private state.** In process memory only; nothing private is written to disk. A persisted result expires 3,600 s after it is written.
- **After a service restart** ([`MVP_LAUNCH_RUNBOOK.md`](MVP_LAUNCH_RUNBOOK.md) §7):
  - public task and receipt facts are rediscovered from the chain;
  - private registration, input and result state is lost;
  - an earlier task shows as `not_registered` (attempts 0, no result) with its receipt height still visible;
  - `result` answers `TASK_NOT_REGISTERED`;
  - `mbongo-mvp verify` cannot complete without the result bytes — it exits `5` with `TASK_NOT_REGISTERED`;
  - receipt persistence is checked independently, on-chain;
  - earlier tasks are never re-executed automatically.
- **What the client verifies** — separately, never folded together: receipt integrity, result integrity, schema conformity; business accuracy is `NOT ASSESSED`; the injection screen is its own line, and a flagged result is rejected from normal use (`CONTENT_FLAGGED`, exit 12).

## 3. Controlled network boundary

The controlled launch profile ([`MVP_LAUNCH_RUNBOOK.md`](MVP_LAUNCH_RUNBOOK.md) §1, §6; security review §8, §8.1) requires, and `mvp-launch.mjs health` checks over every socket owned by a process of the stack:

| Surface | Required |
|---|---|
| node P2P (TCP) | loopback only (`--p2p-host 127.0.0.1`) |
| node RPC, REST | loopback only |
| mDNS | disabled (`--no-mdns`) |
| UDP sockets of the stack outside loopback | 0 |
| HTTP front, `llama-server` | loopback only |

These statements hold for the controlled launch profile started by `mvp-launch.mjs`. They do not describe a node started with its defaults (P2P on all interfaces, mDNS on) or any multi-host, container or public configuration.

## 4. Limitations

```
BUSINESS_EXTRACTION_ACCURACY=NOT_ASSESSED
PROMPT_INJECTION_IMMUNITY_CLAIMED=NO
PROVIDER_RESISTANT_CONFIDENTIALITY=NO
PRIVATE_STATE_PERSISTENCE=NO
PUBLIC_CHAIN_FACTS_REDISCOVERED=YES
RESULT_RECOVERY_AFTER_RESTART=NO
CLI_VERIFY_AFTER_RESTART=NOT_SUPPORTED_WITHOUT_RESULT
```

- Human review of every extracted profile is mandatory; no consequential recruiting decision may rest on extraction output alone.
- Model resistance to prompt injection is best effort; the release property is that detected adversarial overrides are never delivered as success (CC-3). The screen sees only the artifacts it can see: a quoted value echoed, or prompt text copied. Overrides through unquoted or punctuated values are **not** detected, and neither are attacks phrased without a listed instruction marker: on an independent synthetic corpus the model obeyed 14 of 16 attacks and all 14 were accepted (#178). `CLEAR` never means uninfluenced, and human review of every profile stays mandatory.
- The output contract bounds string lengths; values longer than a bound are cut (observed: a 43-character degree title cut at 40 — [#167](https://github.com/MbongoChain/mbongo-chain/issues/167)).
- Accepted risks and pre-public-launch work are listed in the security review §11.

## 5. Open post-launch follow-ups

Not part of the release; not to be folded into an observation run.

| Issue | Subject |
|---|---|
| [#166](https://github.com/MbongoChain/mbongo-chain/issues/166) | replay-harness diagnostics |
| [#167](https://github.com/MbongoChain/mbongo-chain/issues/167) | Kaayu profile string-length bounds |
| [#169](https://github.com/MbongoChain/mbongo-chain/issues/169) | `verify` checks receipt integrity when the result is unavailable after a restart |

## 6. Where the evidence is

| Evidence | Location |
|---|---|
| launch-gate runs (four candidates, three stopped, one PASS) and publication | [#164](https://github.com/MbongoChain/mbongo-chain/issues/164) comments |
| MVP-D acceptance, v3 revalidation, CC-3 adversarial run | `docs/product/evidence/` |
| launch-gate raw outputs (logs, smoke, privacy scans) | operator's private evidence directory; not committed (they contain synthetic CV and result content) |
