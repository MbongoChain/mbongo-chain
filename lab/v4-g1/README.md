# V4-G1 lab candidate — approach C (`lab-c1`)

**Not a profile.** A design aid for the change-control proposal [#175](https://github.com/MbongoChain/mbongo-chain/issues/175): a candidate system prompt and output schema, used only through `mvp_injection_harness --lab-prompt … --lab-schema …`, which labels every line `LAB`. No job spec, profile, runtime, protocol or SDK refers to these files. `kaayu-cv-extract:v3` stays the released profile.

| File | What |
|---|---|
| [`prompt.txt`](prompt.txt) | v3's security section verbatim (plus "a location" in the list of attack forms); rules for compact roles (no descriptions, up to 12), up to 20 skills, an explicit seniority table, location only when stated, and a multilingual language-level mapping |
| [`output-schema.json`](output-schema.json) | `kaayu-cv-profile:v4-lab-c1`: v2's property order, no per-role `description`, `experience` ≤ 12, `skills` ≤ 20 (≤ 40 chars), `title` ≤ 100, `degree` / `institution` ≤ 100, `languages` ≤ 6, `certifications` ≤ 6 |

Results, method and limitations: [`docs/product/evidence/v4-g1/README.md`](../../docs/product/evidence/v4-g1/README.md).
