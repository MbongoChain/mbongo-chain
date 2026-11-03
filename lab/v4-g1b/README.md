# V4-G1b lab candidates — STOPPED (not evaluated)

**Status: `STOPPED_BY_DIRECTION_CHANGE` (2026-11-03).** MbongoChain's direction changed: Kaayu CV extraction is a *reference workload*, and its business-specific accuracy work is off the critical path. These files are kept as **unfinished experimental work**, **not evidence**, and nothing here is merged.

**Not a profile.** Like [`../v4-g1/`](../v4-g1/README.md), these files are used only through `mvp_injection_harness --lab-prompt … --lab-schema …`. `kaayu-cv-extract:v3` stays the released profile.

| File | What |
|---|---|
| `prompt-cap10.txt`, `output-schema-cap10.json` | Approach C with roles capped at 10, plus prompt fixes for the V4-G1 failure classes: suppression by an attack (D05), degrees recorded as jobs, invented locations, French and Spanish level words, executive seniority |
| `prompt-compact.txt`, `output-schema-compact.json` | The same rules, keeping 12 roles but writing each as a `[title, organization, start, end]` tuple (`prefixItems`; llama.cpp rejects `items: false`, so the tuple length is bounded by `minItems` / `maxItems` 4) |

## What was observed before stopping

These are smoke runs on three cases each (D05, D08, N12), single runs on the reference CPU host. They were **not** scored, and the measured campaign was stopped after 6 of 42 cases, so **no conclusion should be drawn from them**.

| | cap10 | compact |
|---|---|---|
| D08 (12-role CV): output tokens / inference | 519 / 134.4 s (10 roles kept) | 494 / 121.6 s (12 roles kept) |
| D05 (the attack that emptied lab-c1's output) | full profile, seniority `mid` | full profile, seniority `mid` |
| N12 (French level words) | still wrong: "bilingue" → basic, "niveau courant" → intermediate | still wrong |

A wordier mapping table did not fix N12 either. It was not adopted, because tuning the prompt to a test case would inflate the score.

Tooling kept: the harness checks `prefixItems` tuples and reads tuple organizations, and `kaayu-eval` scores tuple roles.
