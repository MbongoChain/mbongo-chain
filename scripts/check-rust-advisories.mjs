// Compares a `cargo audit --json` report with the committed advisory register.
// Nothing is suppressed: every finding is printed, known or not.
//   node scripts/check-rust-advisories.mjs <audit.json>
// Fails on a finding the register lacks, on a register entry no longer
// reported, and on a report that does not cover the committed Cargo.lock.
import fs from "node:fs";
import path from "node:path";

const ROOT = path.join(import.meta.dirname, "..");
const REGISTER = "docs/development/rust-advisories.json";
const reportFile = process.argv[2];
if (!reportFile) throw new Error("usage: check-rust-advisories.mjs <audit.json>");

const report = JSON.parse(fs.readFileSync(reportFile, "utf8"));
const lockFile = process.env.RUST_ADVISORY_LOCK ?? path.join(ROOT, "Cargo.lock");
const registerFile = process.env.RUST_ADVISORY_REGISTER ?? path.join(ROOT, REGISTER);
const register = JSON.parse(fs.readFileSync(registerFile, "utf8"));

// Coverage: the report has to describe every package of the committed lock.
const locked = (fs.readFileSync(lockFile, "utf8").match(/^\[\[package\]\]$/gm) ?? []).length;
const scanned = report.lockfile?.["dependency-count"];
if (locked === 0 || scanned !== locked) {
  throw new Error(`coverage: Cargo.lock has ${locked} packages, the report scanned ${scanned}`);
}
console.log(`coverage: ${scanned} of ${locked} packages in Cargo.lock`);
console.log(`advisory database: ${report.database?.["advisory-count"]} advisories, commit ${report.database?.["last-commit"]}`);

const key = (f) => `${f.id} ${f.package} ${f.version}`;
const found = [];
for (const v of report.vulnerabilities?.list ?? []) {
  found.push({ kind: "vulnerability", id: v.advisory.id, package: v.package.name, version: v.package.version, title: v.advisory.title.trim() });
}
for (const [kind, list] of Object.entries(report.warnings ?? {})) {
  for (const w of list) {
    found.push({ kind, id: w.advisory?.id ?? "YANKED", package: w.package.name, version: w.package.version, title: w.advisory?.title?.trim() ?? "Yanked from the registry" });
  }
}
if (report.vulnerabilities?.count !== (report.vulnerabilities?.list ?? []).length) {
  throw new Error("the report's vulnerability count does not match its list");
}

for (const entry of register.findings) {
  for (const field of ["id", "kind", "package", "version", "scope", "status", "decision"]) {
    if (!entry[field]) throw new Error(`register entry ${key(entry)} has no ${field}`);
  }
}
const known = new Map(register.findings.map((e) => [key(e), e]));
if (known.size !== register.findings.length) throw new Error("the register holds a duplicate");

const seen = new Set(found.map(key));
const fresh = found.filter((f) => !known.has(key(f)));
const stale = register.findings.filter((e) => !seen.has(key(e)));

for (const f of found) {
  const entry = known.get(key(f));
  console.log(`${entry ? entry.status : "NEW"}\t${f.kind}\t${f.id}\t${f.package} ${f.version}\t${f.title}`);
}
const count = (kind) => found.filter((f) => f.kind === kind).length;
console.log(`findings: ${found.length}, of which ${count("vulnerability")} vulnerabilities, ${count("unsound")} unsound, ${count("unmaintained")} unmaintained, ${count("yanked")} yanked`);

for (const f of fresh) console.error(`not in the register: ${key(f)}: ${f.title}`);
for (const e of stale) console.error(`in the register, no longer reported: ${key(e)}`);
if (fresh.length || stale.length) {
  console.error(`${fresh.length} new, ${stale.length} stale. Update ${REGISTER}: see docs/development/rust-advisories.md`);
  process.exit(1);
}
console.log(`register matches: ${found.length} findings, none new, none stale`);
