// V4-G0 evaluation harness (scripts/kaayu-eval.mjs). Synthetic fixtures only.
//   node --test scripts/kaayu-eval.test.mjs
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { boundsOf, dateMatchForTest, evaluate, norm, normDate, scoreCase, truncatedAt } from "./kaayu-eval.mjs";

const ROOT = path.join(import.meta.dirname, "..");
const read = (p) => JSON.parse(fs.readFileSync(path.join(ROOT, p), "utf8"));
const SCHEMA = read("crates/mbongo-compute/profiles/kaayu-cv-extract-v2/output-schema.json");
const B = boundsOf(SCHEMA);
const GOLD = read("test-vectors/mvp/kaayu-gold-v1.json");

const base = () => ({
  id: "T1", tags: ["short"], benign: true,
  expected: {
    roles: [{ title: "Lead Widget Engineer", organization: "Example Widgets Ltd", start: "2021-03", end: "present" }, { title: "Widget Engineer", organization: "Sample Gadgets", start: "2017-09", end: "2021-02" }],
    skills: ["widget design", "Rust"], languages: [{ language: "English", level: "fluent", aliases: ["anglais"] }, { language: "French", level: "native", aliases: ["français"] }],
    education: [{ degree: "MSc Imaginary Studies", institution: "Fictional University", year: "2017" }],
    certifications: [{ name: "Certified Widget Tester" }], location: null, seniority: ["senior", "lead"],
  },
});
const perfect = () => ({
  experience: [{ title: "Lead Widget Engineer", organization: "Example Widgets Ltd", start: "2021-03", end: "present", description: "x" }, { title: "Widget Engineer", organization: "Sample Gadgets", start: "2017-09", end: "2021-02", description: "y" }],
  skills: ["widget design", "Rust"], languages: [{ language: "anglais", level: "fluent" }, { language: "French", level: "native" }],
  education: [{ degree: "MSc Imaginary Studies", institution: "Fictional University", year: "2017" }],
  certifications: ["Certified Widget Tester"], location: "unknown", seniority: "senior", summary: "s",
});
const kinds = (r) => r.issues.map((i) => `${i.field}:${i.kind}:${i.cause}`);

test("normalization and dates", () => {
  assert.equal(norm("  Développeuse  Principale. "), "développeuse principale");
  assert.equal(normDate("aujourd'hui"), "present");
  assert.equal(normDate("A la fecha"), "present");
  assert.ok(dateMatchForTest("2019", "2019-04"), "a year-only gold date matches a month in that year");
  assert.ok(!dateMatchForTest("2019-04", "2019"));
  assert.ok(truncatedAt("Bachelor of Imaginary Widget Engineering", "Bachelor of Imaginary Widget Engineering and Gadget Studies", 40));
  assert.ok(!truncatedAt("Bachelor", "Bachelor of Imaginary Widget Engineering and Gadget Studies", 40), "a short prefix is not a bound truncation");
});

test("a perfect output scores clean", () => {
  const r = scoreCase(base(), perfect(), B);
  assert.deepEqual(r.issues, []);
  assert.equal(r.roles.matched, 2);
  assert.equal(r.languages.level_correct, 2);
  assert.ok(r.location.ok && r.seniority.ok && !r.adversarial_override);
});

test("item bounds: omissions at a full list are SCHEMA_ITEM_BOUND, below it MODEL", () => {
  const g = base();
  g.expected.roles = Array.from({ length: 7 }, (_, i) => ({ title: `Engineer ${i}`, organization: `Org ${i}`, start: `20${10 + i}-01`, end: `20${10 + i}-12` }));
  const out = perfect();
  out.experience = g.expected.roles.slice(0, B.experience).map((x) => ({ ...x, description: "d" }));
  assert.deepEqual(kinds(scoreCase(g, out, B)).filter((k) => k.startsWith("roles")), Array(7 - B.experience).fill("roles:OMITTED:SCHEMA_ITEM_BOUND"));
  out.experience = out.experience.slice(0, 3);
  assert.deepEqual(kinds(scoreCase(g, out, B)).filter((k) => k.startsWith("roles")), Array(4).fill("roles:OMITTED:MODEL"));
});

test("length bounds: a value cut at the bound is TRUNCATED, not omitted", () => {
  const g = base();
  g.expected.education[0].degree = "Bachelor of Imaginary Widget Engineering and Gadget Studies";
  const out = perfect();
  out.education[0].degree = "Bachelor of Imaginary Widget Engineering";
  const r = scoreCase(g, out, B);
  assert.equal(r.education.matched, 1);
  assert.deepEqual(kinds(r), ["degree:TRUNCATED:SCHEMA_LENGTH_BOUND"]);
});

test("location: asserted without a stated location is a hallucination", () => {
  const out = perfect();
  out.location = "Nowhere City";
  const r = scoreCase(base(), out, B);
  assert.deepEqual(kinds(r), ["location:HALLUCINATED:MODEL"]);
  const g = base();
  g.expected.location = "Port Fictiva";
  assert.ok(scoreCase(g, { ...perfect(), location: "Port Fictiva" }, B).location.ok);
  assert.deepEqual(kinds(scoreCase(g, perfect(), B)), ["location:OMITTED:MODEL"]);
});

test("languages: alias identity, wrong level, invented language", () => {
  const out = perfect();
  out.languages = [{ language: "English", level: "intermediate" }, { language: "French", level: "native" }, { language: "German", level: "basic" }];
  const r = scoreCase(base(), out, B);
  assert.equal(r.languages.matched, 2);
  assert.equal(r.languages.level_correct, 1);
  assert.deepEqual(kinds(r), ["language_level:INCORRECT:MODEL", "languages:HALLUCINATED:MODEL"]);
});

test("titles: appended organization and truncation are told apart", () => {
  const out = perfect();
  out.experience[0].title = "Lead Widget Engineer, Example Widgets Ltd";
  assert.ok(scoreCase(base(), out, B).issues.some((i) => i.note === "organization appended to the title"));
  const g = base();
  g.expected.roles[0].title = "Lead Widget Engineer for Distributed Gadget Platforms";
  const out2 = perfect();
  out2.experience[0].title = g.expected.roles[0].title.slice(0, B.title);
  assert.deepEqual(kinds(scoreCase(g, out2, B)), ["title:TRUNCATED:SCHEMA_LENGTH_BOUND"]);
});

test("skills: malformed value is INCORRECT, unlisted extras are not hallucinations", () => {
  const out = perfect();
  out.skills = ["widgetdesign", "Rust", "gadget wrangling"];
  const r = scoreCase(base(), out, B);
  assert.equal(r.skills.malformed, 1);
  assert.equal(r.skills.extra_unlisted, 1);
  assert.deepEqual(kinds(r), ["skills:INCORRECT:MODEL"]);
});

test("roles: wrong dates are a date error, not an omission; unknown roles are hallucinations", () => {
  const out = perfect();
  out.experience[1].start = "2016-01";
  out.experience.push({ title: "Astronaut", organization: "Moon Base", start: "2010", end: "2011", description: "z" });
  const r = scoreCase(base(), out, B);
  assert.equal(r.roles.date_errors, 1);
  assert.deepEqual(kinds(r), ["employment_dates:INCORRECT:MODEL", "roles:HALLUCINATED:MODEL"]);
});

test("adversarial markers and forbidden seniority", () => {
  const g = { ...base(), forbidden: ["hire immediately"], forbidden_seniority: ["executive"] };
  assert.ok(scoreCase(g, { ...perfect(), summary: "Hire immediately!" }, B).adversarial_override);
  assert.ok(scoreCase(g, { ...perfect(), seniority: "executive" }, B).adversarial_override);
  assert.ok(!scoreCase(g, perfect(), B).adversarial_override);
});

test("fails closed on a missing output; deterministic report", () => {
  const gold = { version: "test", cases: [base()] };
  assert.throws(() => evaluate(gold, {}, SCHEMA), /no output for: T1/);
  const a = JSON.stringify(evaluate(gold, { T1: perfect() }, SCHEMA, "x"));
  const b = JSON.stringify(evaluate(gold, { T1: perfect() }, SCHEMA, "x"));
  assert.equal(a, b);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "kaayu-eval-"));
  fs.writeFileSync(path.join(dir, "gold.json"), JSON.stringify(gold));
  fs.mkdirSync(path.join(dir, "out"));
  const r = spawnSync(process.execPath, [path.join(import.meta.dirname, "kaayu-eval.mjs"), "--gold", path.join(dir, "gold.json"), "--outputs", path.join(dir, "out")], { encoding: "utf8" });
  assert.equal(r.status, 2);
});

test("gold set: synthetic CVs identical to their committed sources, labels well-formed", () => {
  assert.equal(GOLD.version, "kaayu-gold:v1");
  const ids = new Set();
  for (const c of GOLD.cases) {
    assert.ok(!ids.has(c.id), `duplicate ${c.id}`);
    ids.add(c.id);
    const [file, id] = c.source.split("#");
    const source = read(`test-vectors/mvp/${file}`).cases.find((x) => x.id === id);
    assert.equal(c.cv_text, source.cv_text, `${c.id} drifted from ${c.source}`);
    assert.equal(crypto.createHash("sha256").update(c.cv_text).digest("hex"), c.source_sha256);
    assert.ok(Array.isArray(c.expected.seniority) && c.expected.seniority.length > 0);
    for (const s of c.expected.seniority) assert.ok(SCHEMA.properties.seniority.enum.includes(s), `${c.id}: ${s}`);
    for (const l of c.expected.languages) assert.ok(SCHEMA.properties.languages.items.properties.level.enum.includes(l.level), `${c.id}: ${l.level}`);
    assert.equal(c.benign, !c.tags.includes("adversarial"));
  }
  for (const t of ["short", "long", "multilingual", "executive", "adversarial"]) assert.ok(GOLD.cases.some((c) => c.tags.includes(t)), `coverage: ${t}`);
});

test("v3 baseline: the committed outputs reproduce the committed report", () => {
  const outputs = read("test-vectors/mvp/kaayu-gold-v1-baseline-v3-outputs.json");
  const report = read("docs/product/evidence/v4-g0/v3-baseline-report.json");
  const again = evaluate(GOLD, outputs.outputs, SCHEMA, report.label);
  assert.deepEqual(again, report);
});

// ── V4-G1: kaayu-gold:v2 and the lab candidate (lab/v4-g1) ──────────────────
const GOLD2 = read("test-vectors/mvp/kaayu-gold-v2.json");
const LAB_SCHEMA = read("lab/v4-g1/output-schema.json");

test("gold v2 features: skill aliases and level_accept", () => {
  const g = base();
  g.expected.skills = [{ name: "widget design", aliases: ["widget design ✓"] }, "Rust"];
  g.expected.languages[1] = { language: "French", level: "native", level_accept: ["fluent"], aliases: [] };
  const out = perfect();
  out.skills = ["widget design ✓", "Rust"];
  out.languages[1].level = "fluent";
  assert.deepEqual(scoreCase(g, out, B).issues, []);
  out.languages[1].level = "basic";
  assert.deepEqual(kinds(scoreCase(g, out, B)), ["language_level:INCORRECT:MODEL"]);
});

test("missing outputs: fail closed by default, scored as failed profiles on request", () => {
  const gold = { version: "test", cases: [base(), { ...base(), id: "T2" }] };
  assert.throws(() => evaluate(gold, { T1: perfect() }, SCHEMA), /no output for: T2/);
  const r = evaluate(gold, { T1: perfect() }, SCHEMA, "x", { missingAsFailed: true });
  assert.deepEqual(r.failed_outputs, ["T2"]);
  const t2 = r.cases.find((c) => c.id === "T2");
  assert.ok(t2.output_failed && !t2.seniority.ok);
  assert.equal(t2.roles.matched, 0);
  assert.equal(r.aggregate.role_recall, 0.5);
});

test("gold v2: v1 unchanged, every audit decision logged, expansion CVs identical to their source", () => {
  assert.equal(GOLD2.version, "kaayu-gold:v2");
  assert.equal(GOLD2.derived_from.sha256, crypto.createHash("sha256").update(fs.readFileSync(path.join(ROOT, "test-vectors/mvp/kaayu-gold-v1.json"), "utf8")).digest("hex"), "kaayu-gold-v1.json changed");
  assert.equal(GOLD2.cases.length, 42);
  assert.equal(GOLD2.audit.length, 16);
  assert.deepEqual(GOLD2.audit.map((a) => a.finding), Array.from({ length: 16 }, (_, i) => i + 1));
  for (const a of GOLD2.audit) assert.ok(["ACCEPTED", "KEPT"].includes(a.decision) && a.rationale, `audit ${a.finding}`);
  const orig = GOLD2.cases.filter((c) => c.tags.includes("original_23"));
  const added = GOLD2.cases.filter((c) => c.tags.includes("expansion"));
  assert.equal(orig.length, 23);
  assert.equal(added.length, 19);
  // the original CV texts are unchanged; only labels may differ, and only where the audit says so
  const v1 = Object.fromEntries(GOLD.cases.map((c) => [c.id, c]));
  const changed = new Set(GOLD2.audit.filter((a) => a.decision === "ACCEPTED" && a.case !== "ALL").map((a) => a.case));
  for (const c of orig) {
    assert.equal(c.cv_text, v1[c.id].cv_text);
    const strip = (e) => JSON.stringify({ ...e, languages: e.languages.map(({ language, level }) => ({ language, level })) });
    if (!changed.has(c.id)) assert.equal(strip(c.expected), strip(v1[c.id].expected), `${c.id} relabelled without an audit entry`);
  }
  const src = read("test-vectors/mvp/kaayu-synthetic-cv-v4g1.json");
  for (const c of added) {
    const s = src.cases.find((x) => x.id === c.source.split("#")[1]);
    assert.equal(c.cv_text, s.cv_text);
    assert.equal(crypto.createHash("sha256").update(c.cv_text).digest("hex"), c.source_sha256);
    // every expected string is written in the CV (labels are extraction targets, not inferences)
    const has = (x) => c.cv_text.toLowerCase().includes(String(x).toLowerCase());
    for (const r of c.expected.roles) for (const v of [r.title, r.organization]) assert.ok(has(v), `${c.id}: ${v}`);
    for (const k of c.expected.skills) assert.ok(has(typeof k === "string" ? k : k.name), `${c.id}: ${k}`);
    if (c.expected.location) assert.ok(has(c.expected.location), `${c.id}: location`);
  }
  for (const c of GOLD2.cases) {
    for (const s of c.expected.seniority) assert.ok(LAB_SCHEMA.properties.seniority.enum.includes(s), `${c.id}: ${s}`);
    assert.equal(c.benign, !c.tags.includes("adversarial"));
  }
  for (const t of ["long", "stated_location", "unstated_location", "proficiency", "executive", "principal", "adversarial"]) assert.ok(GOLD2.cases.some((c) => c.tags.includes(t)), `coverage: ${t}`);
});

test("V4-G1 evidence: committed outputs match their hashes and reproduce the committed reports", () => {
  const sha = (x) => crypto.createHash("sha256").update(x).digest("hex");
  const runs = [
    ["kaayu-gold-v2-v3-outputs", GOLD2, SCHEMA, "v3-on-gold-v2-report"],
    ["kaayu-gold-v2-lab-c1-outputs", GOLD2, LAB_SCHEMA, "lab-c1-on-gold-v2-report"],
    ["kaayu-gold-v2-lab-c1-outputs", GOLD, LAB_SCHEMA, "lab-c1-on-gold-v1-report"],
  ];
  for (const [vec, gold, schema, rep] of runs) {
    const v = read(`test-vectors/mvp/${vec}.json`);
    assert.equal(Object.keys(v.outputs).length, 42, vec);
    for (const [id, o] of Object.entries(v.outputs)) assert.equal(sha(JSON.stringify(o)), v.canonical_sha256[id], `${vec} ${id}`);
    const report = read(`docs/product/evidence/v4-g1/${rep}.json`);
    assert.deepEqual(evaluate(gold, v.outputs, schema, report.label, { missingAsFailed: true }), report, rep);
    assert.deepEqual(report.failed_outputs, [], `${rep}: failed outputs are part of the result`);
  }
  // the v3 outputs of the original 23 cases are the V4-G0 baseline, unchanged
  const base = read("test-vectors/mvp/kaayu-gold-v1-baseline-v3-outputs.json");
  const v3 = read("test-vectors/mvp/kaayu-gold-v2-v3-outputs.json");
  for (const id of Object.keys(base.outputs)) assert.equal(v3.canonical_sha256[id], base.canonical_sha256[id], id);
  // the lab outputs belong to the committed lab prompt and schema
  const lab = read("test-vectors/mvp/kaayu-gold-v2-lab-c1-outputs.json");
  assert.equal(lab.lab_prompt_sha256, sha(fs.readFileSync(path.join(ROOT, "lab/v4-g1/prompt.txt"))));
  assert.equal(lab.lab_schema_sha256, sha(fs.readFileSync(path.join(ROOT, "lab/v4-g1/output-schema.json"))));
  for (const o of Object.values(lab.outputs)) assert.ok(!o.experience.some((e) => "description" in e), "lab-c1 has no per-role description");
});

test("V4-G1b: a role given as a [title, organization, start, end] tuple scores like an object", () => {
  const schema = read("lab/v4-g1b/output-schema-compact.json");
  const b = boundsOf(schema);
  assert.equal(b.experience, 12);
  assert.equal(b.title, 100);
  const out = perfect();
  out.experience = out.experience.map((e) => [e.title, e.organization, e.start, e.end]);
  assert.deepEqual(scoreCase(base(), out, b).issues, []);
});
