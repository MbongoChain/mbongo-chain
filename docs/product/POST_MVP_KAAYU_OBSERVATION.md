# POST-MVP-O1 — Kaayu real-workload observation

**Status:** NORMATIVE PROCESS (observation runbook, non-consensus). **Ready; not started.** O1 starts only on an explicit maintainer instruction that names the batch plan (§5).
**Baseline:** `mvp-v0.1.0` — release commit `099b8c0832f9c8e3f6bf45150fa0064358a21969` ([`MVP_v0.1.0_RELEASE.md`](MVP_v0.1.0_RELEASE.md)).
**Operates with:** [`MVP_LAUNCH_RUNBOOK.md`](MVP_LAUNCH_RUNBOOK.md) (startup, health, incidents, privacy incident), [`MVP_HTTP_CLI.md`](MVP_HTTP_CLI.md) (client). **Record format:** [`o1/observation-record-v1.schema.json`](o1/observation-record-v1.schema.json), with an [example](o1/observation-record-v1.example.json) (illustrative: values from the synthetic D13 run of gate #164 in batch `O1-B00`; its per-field ratings show the format and are not a recorded human review). **Agent skill:** `.claude/skills/mbongochain-kaayu-observation/`.

> **What O1 is.** Observation and measurement of the released MVP on Kaayu workloads: run them through the controlled launch exactly as released, record what happens, have a person judge the extracted profiles, classify every outcome. **O1 changes nothing.** It produces evidence from which the next engineering decision is chosen (§11) — after the batch, not during it.

## 1. Scope

**In scope:** running Kaayu CVs (synthetic, or real under §4) through the released controlled launch; collecting the per-workload evidence of §6; human review of each extracted profile (§8); classifying each outcome (§9); privacy checks (§10); a batch report and a dominant-constraint recommendation (§11).

**Out of scope — not done in O1, under any finding:**

- consensus, block format, RPC wire, SDK wire changes;
- GPU work; an `InferenceBackend` abstraction; a second model or runtime;
- marketplace, MBO, agents, DeepInfer, ODS, DeepSeek Harness / Super-agent;
- confidential-compute work;
- profile or output-schema changes during an active batch;
- autonomous fixes: no observation becomes a bug-fix pull request by itself (§11);
- implementing the open follow-ups #166, #167, #169.

O1 runs the released code. If a run needs a product-code change to continue, the batch stops (§9.2).

## 2. Baseline and environment

- The code is the release commit `099b8c08…`, built `--locked` from a clean tree; `mvp-launch.mjs verify` must pass (it checks the commit, the tree, the profile v3 prompt and schema digests, the runtime and model digests, and the keys).
- The environment is the controlled launch profile ([`MVP_LAUNCH_RUNBOOK.md`](MVP_LAUNCH_RUNBOOK.md) §1–§6): one host on AC power, three-node private devnet, every stack socket on loopback, mDNS disabled, `health` green with `exposure` reporting `P2P_TCP=LOOPBACK_ONLY`, `MDNS=DISABLED`, `UDP_NON_LOOPBACK=0`, `RPC=LOOPBACK_ONLY`, `REST=LOOPBACK_ONLY`.
- Fresh private client and executor keys outside the repository, as for the launch (runbook §3).
- No configuration, profile, schema or binary changes once a batch has started.

## 3. Prerequisites before the first O1 batch

| # | Prerequisite | State at this document's commit |
|---|---|---|
| P1 | baseline builds and `verify` passes at `099b8c08` | met at release (gate #164) |
| P2 | a reviewed, committed privacy-scan tool implementing §10 | `scripts/o1-privacy-scan.mjs` (tooling, not product code; tests `scripts/o1-privacy-scan.test.mjs`, synthetic fixtures only). Met once it is merged with green CI; until then **not met** |
| P3 | the batch plan (§5) approved by the maintainer | not started |
| P4 | for any real CV: the admission conditions of §4 | not started |

## 4. Real-data admission

A real Kaayu CV may be submitted only when **every** condition below is satisfied and recorded (by reference, never by content) in the observation's private admission record:

1. **Authorization.** Explicit authorization to process this CV in O1, from whoever is entitled to give it for Kaayu.
2. **Named reviewer.** A named human reviewer who will perform §8 for this observation.
3. **Retention rule.** An explicit rule for how long the private input, result and review are kept in the private observation directory, and how they are deleted.
4. **Plaintext acknowledgment.** Acknowledgment that the Mbongo-controlled worker sees the CV in plaintext (`PROVIDER_RESISTANT_CONFIDENTIALITY = NO`).
5. **No GitHub.** Confirmation that no real CV content, result content or reviewer free text about the candidate will be committed, or pasted into an issue, pull request or comment.
6. **No CI.** Confirmation that no real CV or result content will enter a CI run or CI artifact.
7. **Private directory only.** Confirmation that raw private data stays in the approved private observation directory (§7) and nowhere else.

If any condition is not satisfied, the observation uses **synthetic data** instead (the corpora under `test-vectors/mvp/` or new synthetic CVs written for O1). This document makes no legal or compliance claim beyond these operating conditions; a privacy incident involving real data follows runbook §12, including Kaayu's data-protection process.

## 5. Batch plan

A batch is fixed **before** it starts, approved by the maintainer, and not changed while it runs:

| Field | Meaning |
|---|---|
| batch ID | `O1-B<NN>` |
| size | number of observations planned |
| data class | `SYNTHETIC`, `AUTHORIZED_REAL`, or a stated mix |
| baseline | release commit and the binary hashes `verify` records |
| reviewer(s) | named, per §4.2 |
| review mode | `HUMAN_EACH` — the named reviewer reviews every observation before the next starts (default; **required for `AUTHORIZED_REAL`**); or `AUTO_THEN_SAMPLE` — `SYNTHETIC` batches only: automated evaluation of every observation, human review of a stated sample (§8.1) |
| retention rule | per §4.3 |
| stop rules | §9.2, unchanged |

Failed, stopped and flagged observations stay in the batch. Nothing is dropped, re-run silently or replaced.

## 6. Per-observation procedure

1. **Check.** `health` green and `exposure` as §2 (record the report).
2. **Stage.** Put the input in the observation's `input/` directory (§7). Record its byte length and the input commitment the CLI prints — never its content.
3. **Submit.** `mbongo-mvp submit <input> --wait --json` (record the JSON line and stderr in `cli/`). Do not edit, retry or resubmit to obtain a better result; a retry is a new observation.
4. **Retrieve.** `mbongo-mvp result <task> --out raw-result/result.json` **within the 3,600 s result lifetime and before any service restart** — afterwards the result cannot be recovered.
5. **Verify.** `mbongo-mvp verify <task> --input <input> --json` (record in `verification/`). Record receipt integrity, result integrity, schema conformity and the injection screen as four separate values.
6. **Review.** The named reviewer applies §8 against the input and records the review in `review/`. In `AUTO_THEN_SAMPLE` mode (§8.1) the operator writes `review/auto-evaluation.json` instead, and the human review follows for the sample, without blocking the next observation.
7. **Scan.** Run the privacy checks of §10 (record in `privacy/`).
8. **Record.** Write `record.json` (§7, schema) and, optionally, a human-readable `summary.md` without candidate content.

Restarts follow runbook §7 and §9: finish every job, retrieve every result, then stop. After a restart, earlier tasks are `not_registered`, `result` and `verify` answer `TASK_NOT_REGISTERED` — that is the released behaviour, not an observation failure.

## 7. Evidence layout

All raw evidence lives **outside the repository**, under a private root the operator chooses (written here as `$MBONGO_O1_ROOT`; never a path inside a Git working tree, a synced public folder or a CI workspace):

```
$MBONGO_O1_ROOT/
  O1-B<NN>/
    batch-plan.json            the approved plan (§5); no candidate content
    launch/                    verify, health, start logs for the batch
    <observation-id>/          e.g. O1-B01-007
      admission/               §4 references (authorization, reviewer, retention) — real data only
      input/                   the CV text                                  PRIVATE
      raw-result/              result.json as retrieved                     PRIVATE
      cli/                     submit JSON line, stderr                     (no content by design; still private)
      verification/            verify output, text and JSON
      review/                  §8 rubric; auto-evaluation.json (§8.1)        PRIVATE (may quote the CV)
      metrics/                 timings, health snapshots
      privacy/                 §10 scan report
      record.json              the observation record (§7, schema)          no candidate content
      summary.md               optional, no candidate content
    batch-report.json          §11
```

**What may ever leave the private root:** `record.json`, `batch-plan.json`, `batch-report.json` and `summary.md` — and only after the §10 check that they contain no candidate content. Task IDs, receipt heights, input commitments, digests and timings are public chain facts or measurements and may appear in them. Nothing under `input/`, `raw-result/` or `review/` is ever committed, attached to an issue or uploaded to CI.

### 7.1 Observation record

One record per workload, JSON, validated against [`o1/observation-record-v1.schema.json`](o1/observation-record-v1.schema.json). It carries identifiers, measurements, the four verification values, the review ratings (§8) and the classification (§9) — **never** raw CV text, result text or reviewer remarks that quote the candidate. Required content:

| Group | Fields |
|---|---|
| identity | observation ID, batch ID, recorded-at timestamp, workload class (`SYNTHETIC` / `AUTHORIZED_REAL`) |
| baseline | release version, release SHA, execution profile, inference profile, runtime build and commit, model SHA-256 |
| task | task ID, task height, receipt height, input byte length, input commitment |
| timings | submission-to-verified latency, inference latency |
| outcomes | submit exit code, CLI outcome, verify exit code, receipt integrity, result integrity, schema conformity, injection screen (and findings) |
| review | status (`PENDING` / `DONE`), per-field ratings, omission count, hallucination count, materially misleading, human accept |
| classification | failure class (§9), subclass, blocking, operator notes (no candidate content) |
| privacy | scanner version, log / chain / secret hits |

`business_extraction_accuracy` is fixed to `NOT_ASSESSED` in every record: one reviewed profile is a judgment, not an accuracy measurement.

## 8. Business-quality review rubric

Schema conformity says the output has the right shape. It says nothing about whether the facts are right. The reviewer compares each extracted field with the CV and rates it:

| Rating | Meaning |
|---|---|
| `CORRECT` | matches the CV |
| `PARTIAL` | right in part — incomplete, imprecise or cut short (truncation is `PARTIAL` with the `TRUNCATED` flag) |
| `INCORRECT` | present in the CV, extracted wrongly |
| `OMITTED` | present in the CV, missing from the output |
| `HALLUCINATED` | in the output, not supported by the CV |
| `NOT_APPLICABLE` | absent from both |

Fields reviewed, at minimum:

| Field | Covers |
|---|---|
| `current_title` | most recent job title |
| `previous_titles` | earlier job titles |
| `employers` | organization names |
| `employment_dates` | start and end of each role |
| `education` | degree, institution, year |
| `languages` | languages and levels |
| `skills` | listed skills |
| `certifications` | certifications |
| `location` | location |
| `summary` | the summary is a faithful, factual description (no praise, ranking or invented facts) |
| `seniority` | the seniority interpretation is defensible from the CV |

Recorded separately: **omission count**, **hallucination count**, **materially misleading** (yes/no: would a recruiter be misled about the candidate on a point that matters), **human accept** (yes/no). A profile with `CONTENT_FLAGGED` is reviewed too: the reviewer records whether the flag was warranted.

Review notes stay in `review/` (private). The record carries ratings and counts only.

### 8.1 Automated evaluation of synthetic observations (`AUTO_EVALUATION`)

For a `SYNTHETIC` batch whose plan sets review mode `AUTO_THEN_SAMPLE` (§5), the operator — typically the coding agent — may evaluate every observation automatically, because the source CV is known and invented. It never applies to `AUTHORIZED_REAL` data.

- **Separate artifact.** The assessment goes to the private `review/auto-evaluation.json`, labelled `"assessment_type": "AUTO_EVALUATION"` and `"not_human_review": true`, naming the evaluator. It holds per-field ratings on the §8 scale with a rationale each, provisional omission and hallucination counts, mechanical checks (values at a schema bound, items kept versus present in the CV), and the likely cause of each issue (`SCHEMA` bound versus `MODEL` extraction). A maintainer decision quoted there is marked as such.
- **The record stays human.** `record.json` review fields carry only decisions a human actually made. Without them, `review.status` is `PENDING`; an automated assessment never sets `human_accept`, `materially_misleading`, the counts, the field ratings or the reviewer. The operator may classify the observation provisionally (`classification.subclass = AUTO_EVALUATION_PROVISIONAL`, with an operator note).
- **Human sample.** The batch plan states the sample the named reviewer completes (at least one observation per batch). The sample must include any observation whose automated assessment is used to support a decision.
- **No interruptions for non-blocking findings.** In this mode the operator does not stop for ordinary business-quality findings (partial fields, truncation, omissions). The §9.2 stop rules apply unchanged.
- **Human approval stays mandatory** for every consequential decision: accepting a batch's findings, the §11 recommendation and next gate, any profile, schema or product change, and any admission of real data. An automated assessment informs these decisions and never makes them.
- **Reporting.** The batch report keeps human-review counts and automated counts in separate sections and states how many observations a human reviewed. Neither is an accuracy claim.

O1-B01 first used this under a maintainer instruction given during the batch (recorded in its private `governance-addendum-001.json`); its records are not rewritten.

**No aggregate accuracy claim.** Per-batch counts are reported (§11), but `BUSINESS_EXTRACTION_ACCURACY` stays `NOT_ASSESSED` until a later, explicitly approved evaluation gate defines a reviewed sample, a method and a threshold, and changes it.

## 9. Failure taxonomy

Every observation gets exactly one primary class (and optional secondary notes). Classes keep product problems apart; a business-quality problem is never reported as an integrity failure, and the reverse.

| Class | Examples | Blocking |
|---|---|---|
| `NONE` | everything verified; review recorded | — |
| `RUNTIME_FAILURE` | service crash; inference failure or timeout; parked task; receipt not anchored; result retrieval failure (other than the documented post-restart `TASK_NOT_REGISTERED`); devnet divergence; networking failure | stop after 2 consecutive, or as runbook §10 says |
| `INTEGRITY_FAILURE` | receipt invalid (signature, task, executor or input binding); result commitment mismatch; duplicate or invalid receipt | **stop the batch at once** (runbook §10) |
| `SECURITY_FAILURE` | unauthorized request accepted; replay accepted; private-data leak into logs, chain or committed evidence; unexpected network exposure (`exposure` not as §2); an adversarial override **not** flagged by the screen | **stop the batch at once** (runbook §10, §12) |
| `SCHEMA_FAILURE` | output fails the output contract (`verify` exit 11) | stop the batch; maintainer review |
| `BUSINESS_QUALITY_FAILURE` | wrong extraction; omission; hallucination; misleading summary; field truncation such as the known degree bound (#167); a flag the reviewer judged unwarranted | not blocking; recorded |
| `OPERATOR_UX_ISSUE` | confusing CLI output; manual friction; poor diagnostics | not blocking; recorded |

`CONTENT_FLAGGED` (exit 12) is the released injection control working: classify it by what the reviewer finds (an unwarranted flag is `BUSINESS_QUALITY_FAILURE`; a warranted one is `NONE` with a note).

### 9.1 Not failures

The documented released behaviour is never classified as a failure: post-restart `not_registered`, `TASK_NOT_REGISTERED` from `result` or `verify` after a restart, `RESULT_EXPIRED` after 3,600 s, business accuracy `NOT ASSESSED`.

### 9.2 Stop rules

Stop the batch, keep every artifact, and report to the maintainer when: any `INTEGRITY_FAILURE` or `SECURITY_FAILURE`; any `SCHEMA_FAILURE`; two consecutive `RUNTIME_FAILURE`s; `exposure` not as §2; a privacy check hit (§10); or the run cannot continue without a code, profile, schema or configuration change. The operator does not patch, re-run selectively or work around; failed observations stay in the record.

## 10. Privacy checks

Reusing the #164 lessons. For each observation, and over the whole batch at its end — with [`scripts/o1-privacy-scan.mjs`](../../scripts/o1-privacy-scan.mjs) (`--root` the batch or observation directory, `--launch-config` for the launch state logs, key files and node RPC; exit 0 `PASS`, 1 `FAIL`, 2 `INCOMPLETE`; one new report per run under `privacy/`, naming files and needle hashes, never content or keys):

1. **Chain.** No candidate content (CV lines, result values) in any block on any of the three nodes.
2. **Logs.** No candidate content in node, service, `llama-server` or CLI diagnostic output.
3. **Secrets.** No client or executor seed, private key or capability in any artifact — including the private ones.
4. **Committed evidence.** No candidate content in anything that may leave the private root (§7): `record.json`, `batch-plan.json`, `batch-report.json`, `summary.md`.

**Intentional content versus leakage.** The input, the retrieved result and the private review contain candidate content by design; they are excluded from content matching, by exact role, and still scanned for secrets. An evaluator report that quotes committed public test-vector markers (as `adversarial-report.json` did at #164) is classified the same way, by exact file name and never by a broad pattern. Any other hit is leakage: a `SECURITY_FAILURE`, handled under runbook §12.

## 11. After the batch — decision discipline

1. **Report.** `batch-report.json` and a short human summary: counts per class, per-field rating counts, omission and hallucination counts, materially-misleading and human-accept counts, latency percentiles (nearest rank), privacy results, stop events. No accuracy percentage is claimed.
2. **Classify the dominant constraint** — the one that most limits Kaayu's use of the product, with the counts that show it — as exactly one of:

   | Category | Typical evidence |
   |---|---|
   | `PROFILE_QUALITY` | business-quality failures dominate and trace to the prompt or extraction behaviour |
   | `SCHEMA` | the output contract itself causes failures (bounds, missing fields — e.g. #167) |
   | `INFERENCE_BACKEND` | runtime failures or limits trace to the CPU runtime/model path |
   | `GPU` | latency or capacity is the measured binding constraint and CPU-side options are shown insufficient |
   | `PERSISTENCE` | in-memory state loss (restarts, expiry) blocks real use |
   | `OPERATOR_UX` | operator friction dominates |
   | `SECURITY` | a security finding dominates |
   | `NO_CHANGE` | the release meets the observed need; keep observing |

   No category is preselected; `GPU` in particular needs measured latency or capacity evidence, not expectation.
3. **Propose, then decide.** The recommendation names the evidence and one next gate, and how many of the batch's observations a human reviewed (§8.1). The maintainer chooses. Only then is engineering work opened, as its own gate with its own scope.

## 12. Standing positions during O1

```
BUSINESS_EXTRACTION_ACCURACY=NOT_ASSESSED
PROMPT_INJECTION_IMMUNITY_CLAIMED=NO
PROVIDER_RESISTANT_CONFIDENTIALITY=NO
PRIVATE_STATE_PERSISTENCE=NO
CLI_VERIFY_AFTER_RESTART=NOT_SUPPORTED_WITHOUT_RESULT
```
