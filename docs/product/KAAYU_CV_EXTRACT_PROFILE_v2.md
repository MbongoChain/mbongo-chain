# Kaayu CV Extract Profile v2 — `kaayu-cv-extract:v2`

**Status:** NORMATIVE APPLICATION PROFILE — NON-CONSENSUS — MVP v0.1, **active MVP profile**. Change control CC-1 of [`MVP_v0.1.md`](MVP_v0.1.md) §20.1 (gate MVP-B.1).
**Supersedes for the MVP:** `kaayu-cv-extract:v1` ([`MVP_INFERENCE_CONTRACT_v1.md`](MVP_INFERENCE_CONTRACT_v1.md) §11–§15), which stays frozen, byte-reproducible and resolvable.
**Generic contract:** `mbongo-infer:v1`, unchanged except the amendment of §6 below, which is isolated and **requires maintainer approval**.
**Code:** `crates/mbongo-compute/src/profiles/`. **Assets:** `crates/mbongo-compute/profiles/kaayu-cv-extract-v2/`. **Vector:** [`test-vectors/inference/kaayu-cv-extract-v2.json`](../../test-vectors/inference/kaayu-cv-extract-v2.json). **Test group:** `kaayu_profile_v2` (CI step "Mbongo Kaayu CV Profile v2").

> **One sentence.** v2 asks the model for a concise recruiting profile — the five most recent positions, one-sentence descriptions, a short summary — instead of an exhaustive transcription, so that its largest legal output fits a 600-token budget; on the reference CPU it completes all ten synthetic CVs, including the two v1 truncated, with P95 98.5 s.

## 1. The v1 defect

`kaayu-cv-profile:v1` allowed 15 positions with 300-character descriptions, 40 skills of 60 characters, a 600-character summary, 10 education entries, 10 languages and 15 certifications. Its own job spec allowed 400 generated tokens. Measured with the model's tokenizer (`POST /tokenize` on the pinned llama-server), a maximal legal v1-shaped output under the bounds MVP-B.1 first proposed (5 positions × 180 characters, 30 skills, 400-character summary) already costs **1,338 tokens** compact and **1,686** as the model formats it. MVP-B's real run (#152, #153) showed the consequence: R08 (12 positions) and R10 (executive) stopped at exactly 400 tokens with `finish_reason: "length"` and were correctly refused as `OUTPUT_NOT_JSON`; allowed to finish they needed 1,331 and 463 tokens.

**v1 allowed output cardinality incompatible with its own generation budget.** The runtime, the model, schema enforcement and the worker were correct.

## 2. Design

**v2 narrows the output to the recruiting information needed by the MVP**, then sizes the budget to the narrowed output — not the other way round.

| | Rule |
|---|---|
| `FULL_CV_TRANSCRIPTION` | **NO** |
| `MOST_RELEVANT_EXPERIENCE_ONLY` | **YES** — positions only (no projects, studies or volunteering), the 5 most recent, most recent first |
| identity | unchanged: no name, contact, address, birth date or identifier field exists |
| judgement | extraction only: no ranking, suitability, hiring recommendation or protected-attribute inference |
| output format | the prompt requires **compact single-line JSON**: the pinned runtime otherwise emits 2-space indented JSON, which costs 53% more tokens for the same content (R01: 336 tokens as emitted, 219 compact) and is discarded by canonicalization anyway |

### 2.1 Output bounds (`kaayu-cv-profile:v2`)

Same field set, same required list, same enumerations as v1; only bounds change. Character counts are Unicode scalar values.

| Field | v1 | v2 | MVP-B.1 recommendation | Why v2 differs from the recommendation |
|---|---|---|---|---|
| `experience` maxItems | 15 | **5** | 5 | — |
| `experience[].description` | 300 | **160** | 160–200 | lower end: each position is the largest token cost |
| `experience[].title`, `organization` | 120 | **40** | — | five of each must fit the budget |
| `experience[].start`, `end` | 10 | **7** | — | `YYYY-MM`, `YYYY` or `present` |
| `summary` | 600 | **200** | ≈ 400 | 400 alone ≈ 90 tokens; one or two sentences |
| `location` | 120 | **40** | — | city, region or country |
| `skills` maxItems × length | 40 × 60 | **10 × 24** | 30 | 30 skills at the recommended length ≈ 240 tokens, half the budget |
| `education` maxItems | 10 | **2** (degree, institution 40; year 4) | 5 | highest or most recent two |
| `languages` maxItems | 10 | **3** (language 16) | 8 | — |
| `certifications` maxItems | 15 | **3** (40 each) | 8 | most relevant three |

The recommendations were measured before adoption: with five 160–180-character descriptions, **no** set of the recommended cardinalities fits 600 tokens (§4). Keeping five positions with concise descriptions was preferred over keeping long lists of skills, degrees and certifications; the trade-off is stated, not hidden.

### 2.2 Prompt

`prompt-template.txt` (SHA-256 `93ec9176…1f87`, 378 tokens; v1: 421). Twelve rules: use only the CV, never invent, compact single-line JSON conforming to `kaayu-cv-profile:v2`, identity-free, extraction only, a one-or-two-sentence summary of at most 200 characters, up to 10 skills, **employment positions only; every position when the CV lists 5 or fewer, exactly the 5 most recent otherwise**, one-sentence descriptions of at most 120 characters (the schema's hard bound is 160, so the grammar never cuts a sentence the model is still writing), up to 2 education entries, 3 languages, 3 certifications, no translation.

Two wordings were rejected by measurement on the real model: without the explicit "exactly the 5 most recent" rule the long CV returned **1** position; with descriptions capped at 90 characters the grammar cut the first description and the model then closed the array, again **1** position.

### 2.3 Job spec

The v2 job spec is the v1 job spec with exactly five fields changed:

| Field | v1 | v2 |
|---|---|---|
| `profile.version` | 1 | **2** |
| `profile.output_contract` | `kaayu-cv-profile:v1` | **`kaayu-cv-profile:v2`** |
| `profile.prompt_template_sha256` | `6c950b48…6d63` | **`93ec9176…1f87`** |
| `profile.output_schema_sha256` | `81521447…91af5` | **`0a81727a…50ab`** |
| `generation.max_tokens` | 400 | **600** |

Runtime (llama.cpp `llama-server` v0.7.0 @ `620d2225`), model (Qwen3-4B-Instruct-2507 Q4_K_M, SHA-256 `3605803b…e597`), temperature 0, top-p 1.0, seed 42, input contract `kaayu-cv-extract-input:v1` and the byte limits are unchanged. `job_spec_digest = f38886b234dd411be1b90b25f94b21e4ec10afa5720317c3630dc0b9df4aac6f`; `execution_spec = "mbongo-infer:v1" ‖ digest` (47 bytes, framing unchanged).

## 3. The generic-contract amendment (requires approval)

`mbongo-infer:v1` §6 bounded `generation.max_tokens` to `[1, 400]` for **every** profile, so a v2 job spec with 600 is invalid under the contract as frozen. MVP-B.1 asks for a budget of 512–600 and also for the generic contract to stay unchanged; both cannot hold. The smallest amendment that resolves it keys the cap by `profile.version`: version 1 keeps `[1, 400]` exactly, version ≥ 2 may declare up to 600 (`bounds::max_tokens_cap`). Every version-1 job spec, vector, negative vector and test keeps its outcome (`inference_contract` 25/25 unchanged). Framing, field set, canonicalization, digest, validation order and error taxonomy are untouched. It is a separate commit so it can be accepted or rejected alone.

The alternative that needs no amendment — staying within 400 tokens — was measured: with descriptions capped at 90 characters R08 completes in 198 tokens but returns **1** position instead of 5; with five 160-character descriptions R08 needs 466 tokens.

## 4. Maximum-output analysis

Every array at `maxItems`, every string at exactly `maxLength`, filled with realistic recruiting prose (`kaayu_v2::maximal_profile`), tokenized by the pinned server:

| Candidate bounds | compact tokens | as indented |
|---|---|---|
| MVP-B.1 recommendation (5×180 desc, 30 skills, summary 400, 5 edu, 8 lang, 8 cert) | 1,338 | 1,686 |
| recommendation with tight string lengths | 1,065 | 1,413 |
| 5×160, 20 skills, summary 300, 3 edu, 5 lang, 5 cert | 747 | 1,013 |
| 5×160, 15 skills, summary 280, 3 edu, 5 lang, 5 cert | 679 | 935 |
| **v2: 5×160, 10 skills, summary 200, 2 edu, 3 lang, 3 cert** | **530** | 739 |

| Result | Value |
|---|---|
| `MAX_LEGAL_CANONICAL_BYTES` | 2,789 |
| `MAX_LEGAL_ESTIMATED_TOKENS` | 530 compact (measured, not estimated) |
| headroom of the theoretical maximum under 600 | 11.7% |
| headroom of the largest real output (R08, 466) | **22.3%** |

The theoretical maximum fits only as compact JSON. If the model ignored the formatting rule and every field were at its maximum, the indented form (739) would exceed 600 and be refused as `OUTPUT_NOT_JSON` — never accepted. Every real output in the corpus was single-line.

## 5. Real evidence (reference host, AC power)

Intel Core i7-8850H, 6 cores / 12 threads, 31.3 GB, Windows 11, CPU only, 6 threads, llama.cpp b11669 = v0.7.0 @ `620d2225c`, model SHA-256 re-verified at worker start-up. Run through the MVP-B worker profile with v2 (`mvp_inference_harness corpus`), 2026-10-26.

| Case | Result | finish | in / out tokens | ms |
|---|---|---|---|---|
| R01 simple | schema-valid | stop | 568 / 190 | 35,369 |
| R02 sparse | schema-valid | stop | 439 / 86 | 19,674 |
| R03 French | schema-valid | stop | 597 / 221 | 39,767 |
| R04 Unicode | schema-valid | stop | 584 / 192 | 36,091 |
| R05 injection | schema-valid, no `@`, no out-of-enum value | stop | 527 / 140 | 28,848 |
| R06 repeat | byte-identical to R01 (same host) | stop | 568 / 190 | 35,605 |
| R07 multilingual | schema-valid | stop | 537 / 174 | 32,822 |
| R08 long (12 positions) | schema-valid, **5 positions** | stop | 1,221 / 466 | 98,480 |
| R09 student | schema-valid, 0 positions | stop | 499 / 99 | 23,302 |
| R10 executive | schema-valid, 3 positions | stop | 558 / 251 | 42,763 |

| Metric | v1 (MVP-B, 2026-10-17) | v2 |
|---|---|---|
| schema pass | 8/10 | **10/10** |
| truncations (`finish_reason: length`) | 2 | **0** |
| `max_tokens` | 400 | 600 |
| P50 | 48.6 s | **35.4 s** |
| P95 | 55.4 s (completed cases only) | **98.5 s** (all ten) |
| max | 55.4 s | 98.5 s |
| long CV (R08) | failed, 400/400 tokens | 466/600 tokens, 98.5 s |
| executive CV (R10) | failed, 400/400 tokens | 251/600 tokens, 42.8 s |

`MVP_LATENCY_TARGET` (P95 ≤ 120 s) is met. The three must-refuse inputs were refused before the model; the in-flight server kill failed closed and recovered. Log scan: 166 private fragments, 0 hits. Live chain: the worker executed a v2 task end to end (receipt anchored and verified by the client, result commitment matched, 67 blocks and all logs clean).

**Qualitative (non-consensus).** R08 returned exactly the five most recent of its twelve positions, most recent first, with no invented position; R10 its three real positions; R09 no position (its project is not a job). `RECENT_RELEVANT_EXPERIENCE_PRESERVED = YES`. Summaries were 70–123 characters.

**Measurement conditions matter.** On battery with background load the same host was about 25–30% slower (an earlier v2 draft's R08 took 124 s on battery and 96 s on AC). The latency claim is for the reference host on AC power.

**A worker finding.** llama.cpp v0.7.0 builds the grammar with required properties in schema order. The MVP-B worker had re-serialized the schema with sorted keys, forcing the model to write `certifications` first and `summary` last; on R08 that yielded 3 positions. The worker now sends the pinned schema bytes verbatim (MVP-B, #153); the numbers above are from that worker.

## 6. What v2 does not change

`mbongo-infer:v1` framing and canonicalization, the input contract and `input_commitment`, the output commitment rule, the receipt and what it proves (attribution and bindings, not correctness), `ComputeTask`, RPC, SDK, the runtime, the model, the error taxonomy, PRIVATE_OFFCHAIN / provider-visible plaintext. No GPU, alternate runtime, agent, ODS, marketplace, MBO or metering.

## 7. Versioning

`kaayu-cv-extract` version 2 is a new execution identity: a new digest and a new `execution_spec`. Version 1 remains resolvable by the repository registry (`KaayuProfiles`) and its vector reproduces; it is **SUPERSEDED_FOR_MVP**. A later change to any v2 asset, bound or parameter is a version 3 under the same change control.
