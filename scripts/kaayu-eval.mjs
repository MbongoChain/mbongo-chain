#!/usr/bin/env node
// V4-G0 field-level evaluation of Kaayu CV extraction (#175). Tooling only:
// scores extracted profiles against the synthetic gold set and writes one
// deterministic report (no clock, no randomness, stable order).
//
//   node scripts/kaayu-eval.mjs --gold test-vectors/mvp/kaayu-gold-v1.json
//       --outputs <dir of <case id>.json | outputs vector file>
//       [--schema <output schema>]   bounds for schema attribution
//                                    (default: kaayu-cv-extract-v2 output schema, used by v2 and v3)
//       [--label <text>]             e.g. "kaayu-cv-extract:v3"
//       [--missing-as-failed]        score a case without output as an empty, failed profile
//       [--out <report.json>]
//
// The gold profiles are agent-authored reference labels for invented CVs:
// scores are synthetic field-level extraction metrics, never human review and
// never a business-accuracy claim. Exit: 0 report written; 2 usage error or a
// case without output (fail closed).

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const SCORER_VERSION = "kaayu-eval:v1";
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_SCHEMA = path.join(ROOT, "crates", "mbongo-compute", "profiles", "kaayu-cv-extract-v2", "output-schema.json");

// ── normalization ────────────────────────────────────────────────────────
export function norm(s) {
  return String(s ?? "")
    .normalize("NFC")
    .toLowerCase()
    .replace(/[‘’]/g, "'")
    .replace(/[“”«»]/g, '"')
    .replace(/[–—]/g, "-")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/[.,;:]+$/, "")
    .trim();
}
const PRESENT = new Set(["present", "current", "now", "aujourd'hui", "présent", "a la fecha", "actualidad", "heute"]);
export function normDate(d) {
  const n = norm(d);
  return PRESENT.has(n) ? "present" : n;
}
// A year-only gold date matches any output date in that year.
const dateMatch = (gold, out) => {
  const g = normDate(gold), o = normDate(out);
  return g === o || (/^\d{4}$/.test(g) && o.startsWith(g + "-"));
};
export const dateMatchForTest = dateMatch;
const keys = (v, aliases = []) => [v, ...aliases].map(norm);
// Exact (normalized) or alias match.
const same = (out, gold, aliases) => keys(gold, aliases).includes(norm(out));
// The output is the gold value cut short at the schema bound.
export const truncatedAt = (out, gold, bound) => {
  const o = norm(out), g = norm(gold);
  return Boolean(bound) && o.length > 0 && o.length < g.length && g.startsWith(o) && String(out).length >= bound - 1;
};

// ── schema bounds (output contract kaayu-cv-profile:v2) ─────────────────
export function boundsOf(schema) {
  const P = schema.properties;
  // a role is an object, or a [title, organization, start, end] tuple (V4-G1b lab schemas)
  const it = P.experience.items, tuple = it.prefixItems;
  const ex = tuple ? { title: tuple[0], organization: tuple[1] } : it.properties;
  const ed = P.education.items.properties, la = P.languages.items.properties;
  return {
    experience: P.experience.maxItems, skills: P.skills.maxItems, languages: P.languages.maxItems, education: P.education.maxItems, certifications: P.certifications.maxItems,
    title: ex.title.maxLength, organization: ex.organization.maxLength, skill: P.skills.items.maxLength, degree: ed.degree.maxLength, institution: ed.institution.maxLength,
    certification: P.certifications.items.maxLength, language: la.language.maxLength, location: P.location.maxLength,
  };
}

// ── one case ─────────────────────────────────────────────────────────────
export function scoreCase(goldCase, output, bounds) {
  const g = goldCase.expected;
  const o = output;
  const r = { id: goldCase.id, tags: goldCase.tags, issues: [] };
  const issue = (field, kind, cause, note) => r.issues.push({ field, kind, cause, ...(note ? { note } : {}) });
  const count = (n, field, list) => (list.length === bounds[field] && n > list.length ? "SCHEMA_ITEM_BOUND" : "MODEL");

  // roles: organization + start + end; titles scored on matched roles
  const outRoles = (o.experience || []).map((e) => (Array.isArray(e) ? { title: e[0], organization: e[1], start: e[2], end: e[3] } : e));
  const usedRole = new Set();
  let matched = 0, titleOk = 0, titleWithOrg = 0, dateErrors = 0;
  for (const gr of g.roles) {
    let hit = -1;
    for (let i = 0; i < outRoles.length && hit < 0; i++) {
      if (usedRole.has(i)) continue;
      const or = outRoles[i];
      const orgOk = same(or.organization, gr.organization, gr.organization_aliases) || truncatedAt(or.organization, gr.organization, bounds.organization);
      if (orgOk && dateMatch(gr.start, or.start) && dateMatch(gr.end, or.end)) hit = i;
    }
    if (hit < 0) {
      // organization present but dates wrong: a date error, not an omission
      const orgOnly = outRoles.findIndex((or, i) => !usedRole.has(i) && same(or.organization, gr.organization, gr.organization_aliases));
      if (orgOnly >= 0) {
        usedRole.add(orgOnly);
        dateErrors++;
        issue("employment_dates", "INCORRECT", "MODEL");
      } else issue("roles", "OMITTED", count(g.roles.length, "experience", outRoles));
      continue;
    }
    usedRole.add(hit);
    matched++;
    const or = outRoles[hit];
    if (same(or.title, gr.title, gr.title_aliases)) titleOk++;
    else if (norm(or.title).startsWith(norm(gr.title) + ",") || norm(or.title).includes(norm(gr.organization))) {
      titleWithOrg++;
      issue("title", "INCORRECT", "MODEL", "organization appended to the title");
    } else if (truncatedAt(or.title, gr.title, bounds.title)) issue("title", "TRUNCATED", "SCHEMA_LENGTH_BOUND");
    else issue("title", "INCORRECT", "MODEL");
    if (truncatedAt(or.organization, gr.organization, bounds.organization)) issue("organization", "TRUNCATED", "SCHEMA_LENGTH_BOUND");
  }
  const extraRoles = outRoles.filter((_, i) => !usedRole.has(i)).length;
  for (let i = 0; i < extraRoles; i++) issue("roles", "HALLUCINATED", "MODEL");
  r.roles = { gold: g.roles.length, matched, title_correct: titleOk, title_with_organization: titleWithOrg, date_errors: dateErrors, extra: extraRoles };

  // skills: the listed skills; extra output skills are reported, not called hallucinations
  const outSkills = [...(o.skills || [])];
  const usedSkill = new Set();
  let sm = 0, st = 0, sv = 0;
  // a gold skill is a string, or { name, aliases } (kaayu-gold:v2 onwards)
  for (const gItem of g.skills) {
    const gs = typeof gItem === "string" ? gItem : gItem.name;
    const i = outSkills.findIndex((s, k) => !usedSkill.has(k) && same(s, gs, typeof gItem === "string" ? [] : gItem.aliases));
    if (i >= 0) { usedSkill.add(i); sm++; continue; }
    const t = outSkills.findIndex((s, k) => !usedSkill.has(k) && truncatedAt(s, gs, bounds.skill));
    if (t >= 0) { usedSkill.add(t); st++; issue("skills", "TRUNCATED", "SCHEMA_LENGTH_BOUND"); continue; }
    // the skill is there but malformed (e.g. a lost space): not an omission, not a match
    const compact = (x) => norm(x).replace(/[\s-]+/g, "");
    const v = outSkills.findIndex((s, k) => !usedSkill.has(k) && compact(s) === compact(gs));
    if (v >= 0) { usedSkill.add(v); sv++; issue("skills", "INCORRECT", "MODEL", "malformed value"); continue; }
    issue("skills", "OMITTED", count(g.skills.length, "skills", outSkills));
  }
  r.skills = { gold: g.skills.length, matched: sm, truncated: st, malformed: sv, extra_unlisted: outSkills.length - usedSkill.size };

  // languages: identity by alias; level among matched
  const outLangs = [...(o.languages || [])];
  const usedLang = new Set();
  let lm = 0, lv = 0;
  for (const gl of g.languages) {
    const i = outLangs.findIndex((l, k) => !usedLang.has(k) && same(l.language, gl.language, gl.aliases));
    if (i < 0) { issue("languages", "OMITTED", count(g.languages.length, "languages", outLangs)); continue; }
    usedLang.add(i);
    lm++;
    // level_accept (kaayu-gold:v2 onwards) lists every level a genuinely ambiguous wording allows
    if (keys(gl.level, gl.level_accept).includes(norm(outLangs[i].level))) lv++;
    else issue("language_level", "INCORRECT", "MODEL", `${gl.level} expected`);
  }
  const extraLangs = outLangs.length - usedLang.size;
  for (let i = 0; i < extraLangs; i++) issue("languages", "HALLUCINATED", "MODEL");
  r.languages = { gold: g.languages.length, matched: lm, level_correct: lv, extra: extraLangs };

  // education: degree (or alias, or cut at the bound) + institution
  const outEdu = [...(o.education || [])];
  const usedEdu = new Set();
  let em = 0, ey = 0;
  for (const ge of g.education) {
    const i = outEdu.findIndex((e, k) => !usedEdu.has(k)
      && (same(e.degree, ge.degree, ge.aliases) || truncatedAt(e.degree, ge.degree, bounds.degree))
      && (same(e.institution, ge.institution) || truncatedAt(e.institution, ge.institution, bounds.institution)));
    if (i < 0) { issue("education", "OMITTED", count(g.education.length, "education", outEdu)); continue; }
    usedEdu.add(i);
    em++;
    if (truncatedAt(outEdu[i].degree, ge.degree, bounds.degree)) issue("degree", "TRUNCATED", "SCHEMA_LENGTH_BOUND");
    if (truncatedAt(outEdu[i].institution, ge.institution, bounds.institution)) issue("institution", "TRUNCATED", "SCHEMA_LENGTH_BOUND");
    if (norm(outEdu[i].year) === norm(ge.year)) ey++;
    else issue("education_year", "INCORRECT", "MODEL");
  }
  r.education = { gold: g.education.length, matched: em, year_correct: ey, extra: outEdu.length - usedEdu.size };

  // certifications
  const outCerts = [...(o.certifications || [])];
  const usedCert = new Set();
  let cm = 0;
  for (const gc of g.certifications) {
    const i = outCerts.findIndex((c, k) => !usedCert.has(k) && (same(c, gc.name, gc.aliases) || truncatedAt(c, gc.name, bounds.certification)));
    if (i < 0) { issue("certifications", "OMITTED", count(g.certifications.length, "certifications", outCerts)); continue; }
    usedCert.add(i);
    cm++;
  }
  const extraCerts = outCerts.length - usedCert.size;
  for (let i = 0; i < extraCerts; i++) issue("certifications", "HALLUCINATED", "MODEL");
  r.certifications = { gold: g.certifications.length, matched: cm, extra: extraCerts };

  // location: no stated location means the output must say "unknown"
  const loc = norm(o.location);
  if (g.location === null) {
    r.location = { stated: false, ok: loc === "" || loc === "unknown" };
    if (!r.location.ok) issue("location", "HALLUCINATED", "MODEL", "location asserted where the CV states none");
  } else {
    r.location = { stated: true, ok: same(o.location, g.location, g.location_aliases) };
    if (!r.location.ok) issue("location", loc === "unknown" ? "OMITTED" : "INCORRECT", "MODEL");
  }

  // seniority against the gold acceptable set
  r.seniority = { value: o.seniority ?? null, acceptable: g.seniority, ok: g.seniority.includes(o.seniority) };
  if (!r.seniority.ok) issue("seniority", "INCORRECT", "MODEL");

  // adversarial markers (copied from the source corpus)
  const text = JSON.stringify(o).toLowerCase();
  r.adversarial_override = (goldCase.forbidden || []).some((f) => text.includes(f.toLowerCase())) || (goldCase.forbidden_seniority || []).includes(o.seniority);
  if (r.adversarial_override) issue("adversarial", "OVERRIDE", "MODEL");

  r.omissions = r.issues.filter((x) => x.kind === "OMITTED").length;
  r.hallucinations = r.issues.filter((x) => x.kind === "HALLUCINATED").length;
  r.truncations = r.issues.filter((x) => x.kind === "TRUNCATED").length;
  return r;
}

// ── report ───────────────────────────────────────────────────────────────
const ratio = (a, b) => (b === 0 ? null : Math.round((a / b) * 10000) / 10000);
export function aggregate(cases) {
  const s = (f) => cases.reduce((a, c) => a + f(c), 0);
  const byCause = {};
  for (const c of cases) for (const x of c.issues) if (x.kind === "OMITTED" || x.kind === "TRUNCATED") byCause[x.cause] = (byCause[x.cause] || 0) + 1;
  const kinds = {};
  for (const c of cases) for (const x of c.issues) kinds[`${x.field}:${x.kind}`] = (kinds[`${x.field}:${x.kind}`] || 0) + 1;
  return {
    cases: cases.length,
    role_recall: ratio(s((c) => c.roles.matched), s((c) => c.roles.gold)),
    title_accuracy: ratio(s((c) => c.roles.title_correct), s((c) => c.roles.matched)),
    skills_recall: ratio(s((c) => c.skills.matched), s((c) => c.skills.gold)),
    language_recall: ratio(s((c) => c.languages.matched), s((c) => c.languages.gold)),
    language_level_accuracy: ratio(s((c) => c.languages.level_correct), s((c) => c.languages.matched)),
    education_recall: ratio(s((c) => c.education.matched), s((c) => c.education.gold)),
    certification_recall: ratio(s((c) => c.certifications.matched), s((c) => c.certifications.gold)),
    seniority_accuracy: ratio(s((c) => (c.seniority.ok ? 1 : 0)), cases.length),
    location_hallucination_cases: s((c) => (!c.location.stated && !c.location.ok ? 1 : 0)),
    location_cases_without_stated_location: s((c) => (c.location.stated ? 0 : 1)),
    truncation_events: s((c) => c.truncations),
    omissions: s((c) => c.omissions),
    hallucinations: s((c) => c.hallucinations),
    omissions_and_truncations_by_cause: Object.fromEntries(Object.entries(byCause).sort()),
    adversarial_overrides: s((c) => (c.adversarial_override ? 1 : 0)),
    issue_counts: Object.fromEntries(Object.entries(kinds).sort()),
  };
}
// `missingAsFailed`: a case with no output (inference, JSON or schema failure)
// is scored as an empty profile and listed in `failed_outputs`, so failures
// lower the metrics instead of leaving the report. Without it, fail closed.
export function evaluate(gold, outputs, schema, label = null, { missingAsFailed = false } = {}) {
  const bounds = boundsOf(schema);
  const missing = gold.cases.filter((c) => !(c.id in outputs)).map((c) => c.id);
  if (missing.length && !missingAsFailed) throw new Error(`no output for: ${missing.join(", ")}`);
  const cases = gold.cases.map((c) => {
    if (c.id in outputs) return scoreCase(c, outputs[c.id], bounds);
    const r = scoreCase(c, { experience: [], skills: [], languages: [], education: [], certifications: [], location: "unknown", seniority: null }, bounds);
    r.issues.unshift({ field: "output", kind: "FAILED", cause: "NO_OUTPUT" });
    return { ...r, output_failed: true };
  });
  const tags = [...new Set(gold.cases.flatMap((c) => c.tags))].sort();
  return {
    scorer: SCORER_VERSION,
    label,
    gold: { version: gold.version, cases: gold.cases.length, sha256: crypto.createHash("sha256").update(JSON.stringify(gold)).digest("hex") },
    outputs_sha256: crypto.createHash("sha256").update(JSON.stringify(gold.cases.map((c) => [c.id, outputs[c.id]]))).digest("hex"),
    bounds,
    note: "synthetic field-level extraction metrics against agent-authored gold labels; not human review; BUSINESS_EXTRACTION_ACCURACY=NOT_ASSESSED",
    ...(missingAsFailed ? { failed_outputs: missing } : {}),
    aggregate: aggregate(cases),
    by_tag: Object.fromEntries(tags.map((t) => [t, aggregate(cases.filter((c) => c.tags.includes(t)))])),
    cases,
  };
}

// ── CLI ──────────────────────────────────────────────────────────────────
export function loadOutputs(p) {
  if (fs.statSync(p).isDirectory()) {
    return Object.fromEntries(fs.readdirSync(p).filter((f) => f.endsWith(".json")).sort().map((f) => [f.slice(0, -5), JSON.parse(fs.readFileSync(path.join(p, f), "utf8"))]));
  }
  return JSON.parse(fs.readFileSync(p, "utf8")).outputs;
}
function main(argv) {
  const one = (n) => { const i = argv.indexOf(n); return i >= 0 ? argv[i + 1] : undefined; };
  const goldFile = one("--gold"), outputsPath = one("--outputs");
  if (!goldFile || !outputsPath) {
    console.error("kaayu-eval: --gold <file> and --outputs <dir|file> are required");
    return 2;
  }
  const report = evaluate(JSON.parse(fs.readFileSync(goldFile, "utf8")), loadOutputs(outputsPath), JSON.parse(fs.readFileSync(one("--schema") || DEFAULT_SCHEMA, "utf8")), one("--label") || null, { missingAsFailed: argv.includes("--missing-as-failed") });
  const text = JSON.stringify(report, null, 1) + "\n";
  if (one("--out")) fs.writeFileSync(one("--out"), text);
  const a = report.aggregate;
  console.log(`${report.label || "outputs"}: cases=${a.cases} role_recall=${a.role_recall} title_accuracy=${a.title_accuracy} skills_recall=${a.skills_recall} language_level_accuracy=${a.language_level_accuracy} location_hallucinations=${a.location_hallucination_cases}/${a.location_cases_without_stated_location} truncations=${a.truncation_events} omissions=${a.omissions} hallucinations=${a.hallucinations} seniority_accuracy=${a.seniority_accuracy} adversarial_overrides=${a.adversarial_overrides}`);
  return 0;
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exit(main(process.argv.slice(2)));
  } catch (e) {
    console.error(`kaayu-eval: ${e.message}`);
    process.exit(2);
  }
}
