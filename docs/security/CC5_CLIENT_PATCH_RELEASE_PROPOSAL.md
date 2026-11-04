# CC-5 Client Patch Release — Proposal (`mvp-v0.1.1`)

| | |
|---|---|
| **Status** | **PROPOSAL. NOT RECOMMENDED in its current state, and not approved.** Nothing here is deployed, tagged or merged. |
| **Why not recommended** | The candidate failed its frozen criteria: 0 of 14 obeyed attacks detected on the independent corpus, and 2 new benign rejections. See [`SEC178_SCREEN_HARDENING.md`](SEC178_SCREEN_HARDENING.md) §8. |
| **Purpose** | Defines exactly what a release would be, so that the decision is about content and not about mechanics. |
| **Date** | 2026-11-03 |

## 1. Identity of a release, if approved

| | |
|---|---|
| Version | `mvp-v0.1.1`, a new annotated tag. `mvp-v0.1.0` is never moved |
| Change control | CC-5, recorded in a new section of `MVP_v0.1.md` §20, naming the CC-3 text it amends |
| Scope | The **`mbongo-mvp` client binary only**. The screen runs only in the client (`mvp/cli.rs`) |
| Unchanged | `kaayu-cv-extract:v3`, the worker, `mvp_service`, the runtime and model pins, protocol v0.4, SDK 0.2.0, every receipt |
| Compatibility | A v0.1.1 client works against a v0.1.0 service. Exit codes and result fields are unchanged; only which results are `CONTENT_FLAGGED` changes |
| Operator impact | Some results that returned exit 0 return exit 12, including some **benign** ones. The runbook needs a row for benign rejections |

## 2. Preconditions

All are required, and each needs a separate maintainer approval where marked.

| # | Precondition | State |
|---|---|---|
| 1 | Option chosen in `SEC178_SCREEN_HARDENING.md` §8.6 (**approval**) | Open |
| 2 | Frozen criteria met on a **new** held-out corpus, through the real client path | Not met: R1, R2 |
| 3 | #183 out of draft, merge gate passed, PR and push CI reported separately (**approval**) | Not started |
| 4 | Security review and release record updated | Not started |
| 5 | Release gate on the exact release commit, with the launch smoke (**approval**) | Not started |
| 6 | Promotion to `main` through the audited-milestone route (**approval**) | Not started |

## 3. Rollback

- Operators return to the `mvp-v0.1.0` client binary. No state migrates, because the screen stores nothing.
- The tag stays as a record, and the release record states the withdrawal.
- A rollback restores the 6 undetected overrides on the earlier corpora. Human review stays the control either way.

## 4. What a release would and would not claim

**Would state:** the measured counts, each with its corpus, including 0 of 14 on the independent corpus if that still holds.

**Would never state:** general prompt-injection immunity, or that `CLEAR` means uninfluenced. `PROMPT_INJECTION_IMMUNITY_CLAIMED = NO` stays, along with mandatory human review.

## 5. Alternative: a documentation-only CC-5

If the maintainer chooses option A, CC-5 needs **no binary release**. It amends CC-3's stated property so that the screen is a best-effort signal and human review is the safety boundary, and the released client stays as it is.
