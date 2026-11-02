---
name: mbongochain-kaayu-observation
description: Run or support a POST-MVP-O1 Kaayu real-workload observation against the released mvp-v0.1.0 — collect evidence without changing the workload or the system, keep candidate data out of the repository, classify outcomes, and stop on security or integrity blockers. Use whenever a task involves submitting, reviewing, recording or reporting O1 observations.
---

# Kaayu observation (O1)

A procedure, not an enforcement mechanism. The process is decided by
[`docs/product/POST_MVP_KAAYU_OBSERVATION.md`](../../../docs/product/POST_MVP_KAAYU_OBSERVATION.md);
this skill is how an agent applies it. It does not restate the rubric, the
taxonomy or the record schema — read the runbook.

## When to use

Any work on an O1 batch: preparing it, running a workload, collecting
evidence, writing observation records, scanning for leaks, or writing the
batch report. Not for implementing changes — O1 has none.

## Baseline

`mvp-v0.1.0` — release commit `099b8c0832f9c8e3f6bf45150fa0064358a21969` —
is the stable baseline. Build it `--locked` from a clean tree and require
`mvp-launch.mjs verify` and `health` (with `exposure` as the runbook §2
states) to pass before the first workload of a batch.

## Rules

1. **Do not modify product code, profiles, schemas, configuration or
   binaries during an observation run.** If continuing needs a change, stop
   the batch and report. Never patch silently.
2. **Never commit real CV text**, result content, or reviewer remarks about a
   candidate. Never paste them into an issue, pull request, comment, commit
   message or CI input.
3. **Keep raw private inputs, results and reviews outside the repository**,
   under the operator's private observation root (runbook §7). Never write
   them inside a Git working tree.
4. **Real data only under the runbook §4 admission conditions.** If any is
   missing, use synthetic data and say so.
5. **Collect evidence without altering the workload.** No edits to the input,
   no retries to obtain a better result, no resubmission; a retry is a new
   observation. Retrieve the result within its 3,600 s lifetime and before
   any restart.
6. **Keep the four verification values separate**: receipt integrity, result
   integrity, schema conformity, injection screen. Business accuracy is
   `NOT_ASSESSED`.
7. **Business quality needs a named human reviewer.** Never record a review
   on a person's behalf, and never infer per-field ratings for them. In a
   `SYNTHETIC` batch whose plan sets `AUTO_THEN_SAMPLE` (runbook §8.1), write
   your provisional ratings only to `review/auto-evaluation.json`, labelled
   `AUTO_EVALUATION`; leave the record's human-review fields to the human,
   with `review.status` `PENDING`. Never use automated evaluation for real
   data, and never let it stand in for a consequential decision.
8. **Classify every outcome** with the runbook §9 taxonomy. Documented
   released behaviour (post-restart `not_registered`, `TASK_NOT_REGISTERED`
   from `result` or `verify`, `RESULT_EXPIRED`) is not a failure.
9. **Preserve failed observations.** Nothing is dropped, replaced or re-run
   silently.
10. **Stop on a security or integrity blocker** — and on the other runbook
    §9.2 stop rules — keep every artifact, report, and wait.
11. **Run the privacy checks** of runbook §10 per observation and per batch,
    with `scripts/o1-privacy-scan.mjs`; only `PASS` passes (`INCOMPLETE`
    is not a pass). Intentional private content is excluded from content
    matching only by exact role or exact file name (`--exempt`), never by a
    broad pattern, and is still scanned for secrets.
12. **Propose changes only after the batch is reviewed**, as one dominant
    constraint from the runbook §11 list, with its counts. Do not preselect
    a category; GPU in particular needs measured evidence.

## What never belongs in this work

Consensus, RPC or SDK wire changes; GPU work; an `InferenceBackend`
abstraction; marketplace, MBO, agents, DeepInfer, ODS, DeepSeek Harness /
Super-agent; confidential compute; implementing #166, #167 or #169.

## Expected output

Per observation: a `record.json` valid against
`docs/product/o1/observation-record-v1.schema.json`, with no candidate
content. Per batch: counts per class, per-field rating counts, latency
percentiles (nearest rank), privacy results, stop events, and — after the
maintainer has the report — one recommended next gate.
