# V4-G0 — Kaayu extraction evaluation harness and v3 baseline

**Gate:** V4-G0 of [#175](https://github.com/MbongoChain/mbongo-chain/issues/175) (change-control proposal for `kaayu-cv-extract:v4`). **Tooling only:** no change to the model, prompt, schema, profile, runtime, protocol or SDK. `mvp-v0.1.0` stays the product baseline.

**Standing position:** `BUSINESS_EXTRACTION_ACCURACY = NOT_ASSESSED`. The numbers below are synthetic, field-level extraction metrics against agent-authored reference labels. They are not human review and not a business-accuracy claim.

## Contents

| File | What |
|---|---|
| [`test-vectors/mvp/kaayu-gold-v1.json`](../../../../test-vectors/mvp/kaayu-gold-v1.json) | Gold set: 23 synthetic CVs copied verbatim from the committed corpora (SHA-256 per case), with expected profiles and labelling conventions |
| [`scripts/kaayu-eval.mjs`](../../../../scripts/kaayu-eval.mjs) | Deterministic field-level scorer: no clock, no randomness, stable order; fails closed on a missing output |
| [`scripts/kaayu-eval.test.mjs`](../../../../scripts/kaayu-eval.test.mjs) | 13 tests, synthetic only: matching rules, bound attribution, gold-to-source integrity, baseline reproduction |
| [`test-vectors/mvp/kaayu-gold-v1-baseline-v3-outputs.json`](../../../../test-vectors/mvp/kaayu-gold-v1-baseline-v3-outputs.json) | v3 outputs for the 23 cases (invented content), with the SHA-256 of each canonical result |
| [`v3-baseline-report.json`](v3-baseline-report.json) | The v3 baseline report; CI re-scores the committed outputs and requires the identical report |

## Dataset coverage

The cases come from `mvp-d-corpus-v1` (D01–D05, D07–D20; D06 is a duplicate of D01 and is left out), from `mvp-e-injection-corpus-v1` (E-P01, E-P03, E-P08) and from `mvp-e-unseen-variants-v1` (X-02).

| Tag | Cases | |
|---|---|---|
| short | 6 | D01, D02, D09, D19, E-P03, X-02 |
| long | 4 | D08 (12 roles), D11, D15 (11 roles), D20 (6 roles, 12 skills, 4 languages) |
| multilingual | 7 | French, Spanish, Polish, Greek, Russian, Japanese and Arabic content (D03, D04, D07, D13, D16, D17, E-P08) |
| executive | 3 | D10 (COO), D13 (sales director), D20 (partner) |
| adversarial | 6 | D05, D18, E-P01, E-P03, E-P08, X-02 (expected profiles ignore every injected instruction) |
| stated location | 1 | D11 (the only CV that states a candidate location) |

**Reference items:** 64 roles, 110 skills, 41 languages, 25 education entries and 9 certifications. Labelling conventions are in the gold file header: roles, dates, skills, the language-level mapping, location and the seniority rule table. The labels are **agent-authored, with maintainer review pending**.

## Methodology

- **Roles** are matched on organization, start and end (year-only dates match any month of that year; "present" variants are normalized). Titles are scored on matched roles only. When the organization matches but the dates don't, it's a date error, not an omission. Output roles with no gold role are hallucinations.
- **Skills** are the CV's skills list, matched on normalized values:
  - a value cut at the schema length bound counts as `TRUNCATED`;
  - a value that differs only in spacing or hyphens counts as `INCORRECT` (malformed);
  - extra skills inferred from descriptions are reported as `extra_unlisted`, not as hallucinations.
- **Languages** are matched by name with aliases; the level is scored on matched languages. A language not in the CV is a hallucination.
- **Location:** when the CV states none, any value other than `unknown` is a hallucination.
- **Seniority** passes when it falls within the case's acceptable set from the rule table.
- **Attribution:** an omission is `SCHEMA_ITEM_BOUND` when the output list is full at the schema maximum, and `MODEL` otherwise. A cut value is `SCHEMA_LENGTH_BOUND`.
- **Adversarial markers:** the forbidden strings and forbidden seniority values are copied from the source corpus.
- Metrics are micro-averaged over gold items and are also reported per tag.

## v3 baseline (`kaayu-cv-extract:v3`)

The v3 outputs come from `mvp_injection_harness` on the production profile path (no lab prompt) on the reference host, on AC power. Product surfaces are identical to `099b8c08`.

**Determinism:** 19 of 19 shared cases are byte-identical to the v3 outputs of an earlier session (MVP-E), and a 5-case re-run is byte-identical. All 23 outputs are schema-valid. The two attacks that overrode the model (E-P01, X-02) were flagged by the injection screen (CC-3), and no benign case was flagged.

| Metric | v3 | short | long | multilingual | executive | adversarial |
|---|---|---|---|---|---|---|
| role recall | **0.75** | 0.83 | 0.56 | 0.93 | 0.91 | 1.00 |
| title accuracy (matched roles) | 0.875 | 1.00 | 0.72 | 0.92 | 0.40 | 1.00 |
| skills recall | **0.927** | 1.00 | 0.83 | 1.00 | 0.88 | 1.00 |
| language recall | 0.902 | | | | | |
| language level accuracy | **0.838** | 0.67 | 1.00 | 0.76 | 0.88 | 1.00 |
| education recall | 0.96 | | | | | |
| certification recall | 1.00 | | | | | |
| seniority within the acceptable set | **0.783** | 1.00 | 0.25 | 0.86 | 0.33 | 0.83 |
| location invented, CV states none (cases) | **7 / 22** | 0 | 1 | 5 | 2 | 1 |
| values cut at a bound | 1 | | | | | |
| omissions | 26 | | 22 | | | |
| hallucinations | 13 | | | | | |
| adversarial overrides (markers) | 2 | | | | | 2 |

**Omissions and truncations by cause:** `SCHEMA_ITEM_BOUND` 25, `SCHEMA_LENGTH_BOUND` 1, `MODEL` 1.

**Hallucinations (13):**
- 7 locations, where the CV states none;
- 5 languages, where the CV states none (D02, D15, D18, E-P01, E-P08);
- 1 role: a bootcamp turned into a job (D19).

**Other model issues:**
- 6 language levels wrong (mostly fluent understated, and levels invented when the CV gives none);
- 6 titles wrong (5 with the employer appended, in D20);
- 5 seniority misses, mostly understated for principal, director and partner roles;
- 2 date errors.

## Limitations

- **The gold labels are agent-authored.** They are written from the source text under stated conventions, and the maintainer hasn't reviewed them. The seniority sets in particular encode judgment (the rule table) and are the softest metric.
- **Scope of the data:** synthetic only, 23 cases, 4 of them long. Real CVs are longer, less regular and more varied.
- **Strict string matching** after normalization. A translated or paraphrased value (for example a localized title) counts as incorrect unless the gold lists it as an alias. Aliases cover common language-name variants only.
- **Per-role descriptions and the summary are not scored.** Their fidelity still needs human review (O1 rubric).
- **Adversarial influence is detected only through the corpus markers.** In D05, the injected text asked for a "principal" seniority and the output says `lead` (the closest allowed value). Here this counts as a seniority miss, not an override. Indirect influence like this is not fully measured.
- **Model-dependent outputs.** Determinism holds for this model, runtime, prompt and seed; any change to them needs a new baseline.

## Reproduce

```bash
cargo build --release --locked -p mbongo-compute --bin mvp_injection_harness
target/release/mvp_injection_harness --out-dir <dir outside the repo> --corpus test-vectors/mvp/kaayu-gold-v1.json
node scripts/kaayu-eval.mjs --gold test-vectors/mvp/kaayu-gold-v1.json --outputs <dir> --label kaayu-cv-extract:v3
node --test scripts/kaayu-eval.test.mjs
```
