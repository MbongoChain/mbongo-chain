// O1 privacy scanner (scripts/o1-privacy-scan.mjs). Synthetic fixtures only:
// every CV, result and key below is invented for these tests.
//   node --test scripts/o1-privacy-scan.test.mjs
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { classify, forms, needleId, needlesFromInput, needlesFromResult, scan } from "./o1-privacy-scan.mjs";

const CV = "Synthetic Candidate Alpha\nSenior Widget Engineer, Example Widgets Ltd (2019-03 to present)\nBSc Imaginary Studies, Fictional University, 2014\nLangues : français, anglais\n";
const RESULT = JSON.stringify({ summary: "Senior widget engineer with seven years of fictional experience", experience: [{ organization: "Example Widgets Ltd", title: "Senior Widget Engineer" }], location: "Nowhere City" });
const SEED = crypto.createHash("sha256").update("o1-test-client-seed").digest("hex");
const PROMPT = ["You are a careful extraction engine for synthetic tests only.", "short"];
const LINE = "Senior Widget Engineer, Example Widgets Ltd (2019-03 to present)";

// One batch with one observation, laid out as runbook §7.
function batch() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "o1-scan-"));
  const obs = path.join(root, "O1-B99-001");
  const w = (p, t) => {
    fs.mkdirSync(path.dirname(path.join(obs, p)), { recursive: true });
    fs.writeFileSync(path.join(obs, p), t);
  };
  w("input/cv.txt", CV);
  w("raw-result/result.json", RESULT);
  w("review/review.md", `Reviewer notes quoting the CV: ${LINE}`);
  w("cli/submit.stderr", "task_id 00ff\ntask committed at height 12\n");
  w("verification/verify.json", '{"outcome":"VERIFIED"}');
  w("record.json", '{"record_version":"o1-observation-record:v1","task":{"task_id":"00ff"}}');
  return { root, obs, w };
}
async function* blocks(list) {
  for (const [h, b] of list.entries()) yield [h, JSON.stringify(b)];
}
const cleanChain = () => [{ endpoint: "test-node", blocks: blocks([{ h: 0 }, { tx: "AnchorReceipt", input_commitment: "ab".repeat(32) }]) }];
const run = (b, extra = {}) => scan({ root: b.root, seeds: [SEED], promptLines: PROMPT, chain: cleanChain(), ...extra });
// One leak can match several needles (a CV line also contains result
// values), so tests assert which needle and where, not an exact count.
const leakOf = (report, needle) => report.hits.filter((h) => h.needle_id === needleId(needle));
const noContentIn = (report) => {
  const t = JSON.stringify(report);
  for (const n of [...needlesFromInput(CV), ...needlesFromResult(RESULT), SEED]) assert.ok(!t.includes(n), "the report never contains a needle or a key");
};

test("needles: CV lines and result values, never short fragments", () => {
  assert.ok(needlesFromInput(CV).includes(LINE));
  assert.ok(!needlesFromInput("a\nshort line\n").length);
  const r = needlesFromResult(RESULT);
  assert.ok(r.includes("Example Widgets Ltd") && r.includes("Nowhere City") && r.includes(RESULT));
  assert.ok(!r.includes("location"), "keys are not needles");
  // Key fragments of the public output schema are never needles (the #164
  // fragment split produced `"certifications":` from `{"certifications":[]`).
  const withEmpty = needlesFromResult(JSON.stringify({ certifications: [], education: [{ degree: "Imaginary Degree of Testing" }] }));
  assert.ok(withEmpty.every((n) => !/^"?[a-z_]+"?:?$/.test(n)), JSON.stringify(withEmpty));
  assert.ok(withEmpty.includes("Imaginary Degree of Testing"));
  // Text that is not JSON still uses the fragment split.
  assert.ok(needlesFromResult('not json: "a long enough synthetic fragment here"').length >= 1);
});

test("encodings: raw, hex, byte list, JSON escaping, ASCII-escaped JSON", () => {
  const f = Object.fromEntries(forms("Langues : français, anglais"));
  assert.equal(f.hex, Buffer.from("Langues : français, anglais").toString("hex"));
  assert.equal(f.json_ascii, "Langues : fran\\u00e7ais, anglais");
});

test("classification is by exact role or exact path, never by extension", () => {
  const ex = new Set(["O1-B99-001/adversarial-report.json"]);
  assert.equal(classify("O1-B99-001/input/cv.txt", ex), "PRIVATE");
  assert.equal(classify("O1-B99-001/raw-result/result.json", ex), "PRIVATE");
  assert.equal(classify("O1-B99-001/review/notes.md", ex), "PRIVATE");
  assert.equal(classify("O1-B99-001/record.json", ex), "PUBLISHABLE");
  assert.equal(classify("O1-B99-001/adversarial-report.json", ex), "EXEMPT");
  assert.equal(classify("O1-B99-001/other-report.json", ex), "OTHER");
  assert.equal(classify("O1-B99-001/cli/input", ex), "OTHER", "a file named like a role is not a role");
});

test("clean batch: PASS; intentional private content is not a hit", async () => {
  const r = await run(batch());
  assert.equal(r.status, "PASS", JSON.stringify(r.hits));
  assert.equal(r.files_scanned.PRIVATE, 3);
  assert.equal(r.files_scanned.PUBLISHABLE, 1);
  noContentIn(r);
});

test("CV line leaked into a CLI diagnostic or a log: FAIL, content not in the report", async () => {
  const b = batch();
  b.w("cli/submit.stderr", `debug: body was ${LINE}`);
  const extraLog = path.join(b.root, "..", `svc-${path.basename(b.root)}.err`);
  fs.writeFileSync(extraLog, `request ${JSON.stringify(LINE)}`);
  const r = await run(b, { logs: [extraLog] });
  assert.equal(r.status, "FAIL");
  assert.deepEqual(new Set(leakOf(r, LINE).map((h) => h.target)), new Set(["O1-B99-001/cli/submit.stderr", extraLog]));
  assert.ok(r.hits.every((h) => h.kind === "log"));
  noContentIn(r);
});

test("result value in publishable evidence: FAIL as public_evidence", async () => {
  const b = batch();
  b.w("record.json", '{"operator_notes":"organization was Example Widgets Ltd"}');
  const r = await run(b);
  assert.equal(r.status, "FAIL");
  assert.equal(r.public_evidence_hits, 1);
});

test("seed anywhere — publishable, log or private file — is a secret hit, never printed", async () => {
  for (const p of ["record.json", "metrics/m.json", "raw-result/result.json", "input/cv.txt"]) {
    const b = batch();
    b.w(p, `${fs.existsSync(path.join(b.obs, p)) ? fs.readFileSync(path.join(b.obs, p), "utf8") : ""} ${SEED.toUpperCase()}`);
    const r = await run(b);
    assert.equal(r.status, "FAIL", p);
    assert.equal(r.secret_hits, 1, p);
    noContentIn(r);
  }
});

test("exemption is exact: only the named file is excluded from content matching, and still seed-scanned", async () => {
  const b = batch();
  b.w("adversarial-report.json", `{"override_reasons":["forbidden:${LINE}"]}`);
  b.w("other-report.json", `{"override_reasons":["forbidden:${LINE}"]}`);
  let r = await run(b, { exempt: ["O1-B99-001/adversarial-report.json"] });
  assert.equal(leakOf(r, LINE).length, 1, "other-report.json still hits");
  assert.ok(r.hits.every((h) => h.target === "O1-B99-001/other-report.json"), "the exempt file does not");
  b.w("adversarial-report.json", `{"key":"${SEED}"}`);
  r = await run(b, { exempt: ["O1-B99-001/adversarial-report.json"] });
  assert.ok(r.hits.some((h) => h.kind === "secret" && h.target === "O1-B99-001/adversarial-report.json"));
});

test("chain: candidate content in a block, in any encoding, is a chain hit", async () => {
  for (const enc of [(s) => s, (s) => Buffer.from(s).toString("hex"), (s) => Array.from(Buffer.from(s)).join(",")]) {
    const chain = [{ endpoint: "node-a", blocks: blocks([{ h: 0 }, { memo: enc(LINE) }]) }];
    const r = await run(batch(), { chain });
    assert.equal(r.status, "FAIL");
    assert.equal(leakOf(r, LINE).length, 1);
    assert.ok(r.hits.every((h) => h.kind === "chain" && h.target === "chain:node-a:1"));
  }
});

test("fails closed: no needles, no seeds, or no chain is INCOMPLETE, never PASS", async () => {
  const empty = fs.mkdtempSync(path.join(os.tmpdir(), "o1-scan-empty-"));
  assert.equal((await scan({ root: empty, seeds: [SEED], chain: cleanChain() })).status, "INCOMPLETE");
  assert.equal((await run(batch(), { seeds: [] })).status, "INCOMPLETE");
  const r = await run(batch(), { chain: null });
  assert.equal(r.status, "INCOMPLETE");
  assert.ok(r.incomplete.includes("CHAIN_NOT_SCANNED"));
  const r2 = await run(batch(), { chain: [{ endpoint: "dead", blocks: blocks([]) }] });
  assert.equal(r2.status, "INCOMPLETE");
});

test("CLI: writes a new report, never overwrites one, prints counts only", () => {
  const b = batch();
  const key = path.join(b.root, "..", `key-${path.basename(b.root)}`);
  fs.writeFileSync(key, `${SEED}\n`);
  b.w("cli/submit.stderr", `leak ${LINE}`);
  const out = path.join(b.obs, "privacy", "o1-privacy-scan-fixed.json");
  const cli = (args) => spawnSync(process.execPath, [path.join(import.meta.dirname, "o1-privacy-scan.mjs"), ...args], { encoding: "utf8" });
  const args = ["--root", b.root, "--key-file", key, "--skip-chain", "--out", out];
  const r = cli(args);
  assert.equal(r.status, 1, r.stderr);
  assert.match(r.stdout, /^FAIL log_hits=[1-9]d* chain_hits=0 secret_hits=0 public_evidence_hits=0 /);
  for (const n of [LINE, SEED]) assert.ok(!r.stdout.includes(n) && !r.stderr.includes(n));
  const report = JSON.parse(fs.readFileSync(out, "utf8"));
  assert.equal(report.status, "FAIL");
  assert.equal(leakOf(report, LINE).length, 1);
  assert.deepEqual(report.chain, "SKIPPED");
  noContentIn(report);
  // A second run never overwrites the failed report.
  const again = cli(args);
  assert.equal(again.status, 2);
  assert.equal(JSON.parse(fs.readFileSync(out, "utf8")).at, report.at);
  // Earlier reports under privacy/ are not themselves scanned.
  const third = cli(["--root", b.root, "--key-file", key, "--skip-chain", "--out", path.join(b.obs, "privacy", "o1-privacy-scan-second.json")]);
  assert.equal(JSON.parse(fs.readFileSync(path.join(b.obs, "privacy", "o1-privacy-scan-second.json"), "utf8")).log_hits, report.log_hits, "the first report is not re-scanned");
});
