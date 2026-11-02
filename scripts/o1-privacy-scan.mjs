#!/usr/bin/env node
// POST-MVP-O1 privacy scan (docs/product/POST_MVP_KAAYU_OBSERVATION.md §10).
// Tooling only: reads files and chain blocks, writes one report, changes
// nothing else. Reproduces the checks of the controlled-launch gate (#164):
//
//   1. chain   — no candidate content in any block of any node;
//   2. logs    — no candidate content in node, service, llama-server or CLI
//                diagnostic output;
//   3. secrets — no client or executor seed in any artifact, private ones
//                included;
//   4. public  — no candidate content in the files that may leave the private
//                observation root (record.json, summary.md, batch-plan.json,
//                batch-report.json).
//
// Candidate content ("needles") comes from the O1 root itself: every line of
// every input/ file, and the text and string values of every raw-result/
// file. Files that hold candidate content by design are excluded from content
// matching by exact role (input/, raw-result/, review/) or exact relative
// path (--exempt), never by extension, and are still scanned for secrets.
//
// The report names files, needle ids (SHA-256 prefixes) and encodings — never
// a needle or a key. It is written to a new timestamped file and never
// overwrites an earlier report, so a failed scan keeps its evidence.
//
//   node scripts/o1-privacy-scan.mjs --root <O1 batch or observation dir>
//       [--launch-config <file>]   state dir (logs), key files, node RPC
//       [--key-file <file>]...     seeds to look for (in addition)
//       [--log <file|dir>]...      more logs to scan
//       [--rpc <url>]...           node JSON-RPC endpoints (default: the three
//                                  controlled-launch nodes with --launch-config)
//       [--prompt <file>]          prompt template (default: active profile)
//       [--exempt <relative path>]...  exact files under --root holding
//                                  candidate content by design
//       [--skip-chain]             report INCOMPLETE instead of scanning chain
//       [--out <file>]             report path (default: <root>/privacy/…)
//
// Exit: 0 PASS, 1 FAIL (a hit), 2 INCOMPLETE or usage error.

import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const SCANNER_VERSION = "o1-privacy-scan:v1";
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
// The controlled-launch nodes (scripts/mvp-launch.mjs NODES).
export const LAUNCH_RPC = [39944, 39945, 39946].map((p) => `http://127.0.0.1:${p}/rpc`);
// Directory roles of the O1 evidence layout (runbook §7) that hold candidate
// content by design.
const PRIVATE_ROLES = ["input", "raw-result", "review"];
// Files that may leave the private root (runbook §7).
export const PUBLISHABLE = ["record.json", "summary.md", "batch-plan.json", "batch-report.json"];
const MIN_LINE = 12;
const MIN_VALUE = 12;
const MIN_PIECE = 16;

// ── needles ──────────────────────────────────────────────────────────────
export const needleId = (n) => crypto.createHash("sha256").update(n).digest("hex").slice(0, 12);
const nonAsciiEscaped = (s) => s.replace(/[^\x00-\x7f]/g, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, "0")}`);
// The encodings a leak has been seen or could plausibly take in logs and
// blocks: verbatim, hex of the UTF-8 bytes, a comma-separated byte list, JSON
// string escaping, and JSON with non-ASCII escaped.
export function forms(n) {
  const json = JSON.stringify(n).slice(1, -1);
  const out = { raw: n, hex: Buffer.from(n).toString("hex"), bytes: Array.from(Buffer.from(n)).join(","), json, json_ascii: nonAsciiEscaped(json) };
  return Object.entries(out).filter(([, v], i, a) => v.length >= MIN_LINE && a.findIndex(([, w]) => w === v) === i);
}
export function needlesFromInput(text) {
  return text.split(/\r?\n/).map((l) => l.trim()).filter((l) => l.length >= MIN_LINE);
}
// A JSON result yields its whole text and its string values: the values are
// the candidate content, the keys are the public output schema. Text that is
// not JSON falls back to the #164 fragment split (which, on JSON, also cut
// out key fragments such as `"certifications":` and matched them everywhere).
export function needlesFromResult(text) {
  const out = new Set([text.trim()]);
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    parsed = undefined;
  }
  if (parsed !== undefined) {
    const walk = (v) => {
      if (typeof v === "string" && v.trim().length >= MIN_VALUE) out.add(v.trim());
      else if (v && typeof v === "object") Object.values(v).forEach(walk);
    };
    walk(parsed);
  } else {
    for (const piece of text.split(/","|":"|\{|\}|\[|\]/)) if (piece.trim().length >= MIN_PIECE) out.add(piece.trim());
  }
  return [...out].filter((n) => n.length >= MIN_LINE);
}

// ── files ────────────────────────────────────────────────────────────────
function walk(dir) {
  if (!fs.existsSync(dir)) return [];
  if (fs.statSync(dir).isFile()) return [dir];
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(dir, e.name)) : [path.join(dir, e.name)]));
}
const rel = (base, f) => path.relative(base, f).split(path.sep).join("/");
// Classification of a file under the O1 root, by exact role or exact path.
export function classify(relPath, exempt) {
  const parts = relPath.split("/");
  if (exempt.has(relPath)) return "EXEMPT";
  if (parts.slice(0, -1).some((p) => PRIVATE_ROLES.includes(p))) return "PRIVATE";
  if (PUBLISHABLE.includes(parts.at(-1))) return "PUBLISHABLE";
  return "OTHER";
}

// ── scan ─────────────────────────────────────────────────────────────────
// Pure core: everything is passed in; nothing is printed.
//   root: O1 batch or observation dir; logs: extra files (logs); seeds: keys;
//   promptLines: profile prompt lines; exempt: exact relative paths;
//   chain: null (skipped) or [{ endpoint, blocks: async iterable of [height, json] }]
export async function scan({ root, logs = [], seeds = [], promptLines = [], exempt = [], chain = null, previousReports = new Set() }) {
  const exemptSet = new Set(exempt);
  const rootFiles = walk(root).map((f) => ({ path: f, rel: rel(root, f) }));
  const candidate = new Set();
  for (const f of rootFiles) {
    const dirs = f.rel.split("/").slice(0, -1);
    if (dirs.includes("input")) needlesFromInput(fs.readFileSync(f.path, "utf8")).forEach((n) => candidate.add(n));
    if (dirs.includes("raw-result")) needlesFromResult(fs.readFileSync(f.path, "utf8")).forEach((n) => candidate.add(n));
  }
  const prompt = new Set(promptLines.map((l) => l.trim()).filter((l) => l.length >= MIN_PIECE));
  const seedList = seeds.map((s) => s.trim().toLowerCase()).filter((s) => /^[0-9a-f]{64}$/.test(s));
  const hits = [];
  const files = { PRIVATE: 0, EXEMPT: 0, PUBLISHABLE: 0, OTHER: 0, LOG: 0 };
  const matchContent = (text, needles, where, cls, kind) => {
    for (const n of needles) for (const [form, x] of forms(n)) if (text.includes(x)) hits.push({ kind, target: where, class: cls, needle_id: needleId(n), form });
  };
  const matchSeeds = (text, where, cls) => {
    for (const s of seedList) if (text.includes(s) || text.includes(s.toUpperCase())) hits.push({ kind: "secret", target: where, class: cls, needle_id: needleId(s), form: "raw" });
  };
  for (const f of rootFiles) {
    if (previousReports.has(path.resolve(f.path))) continue;
    const cls = classify(f.rel, exemptSet);
    files[cls]++;
    const text = fs.readFileSync(f.path, "latin1") + "\n" + fs.readFileSync(f.path, "utf8");
    matchSeeds(text, f.rel, cls);
    if (cls === "PUBLISHABLE") matchContent(text, candidate, f.rel, cls, "public");
    if (cls === "OTHER") matchContent(text, [...candidate, ...prompt], f.rel, cls, "log");
  }
  for (const f of logs.flatMap(walk)) {
    files.LOG++;
    const text = fs.readFileSync(f, "utf8");
    matchSeeds(text, f, "LOG");
    matchContent(text, [...candidate, ...prompt], f, "LOG", "log");
  }
  const chainReport = [];
  if (chain) {
    for (const { endpoint, blocks } of chain) {
      let n = 0;
      for await (const [height, json] of blocks) {
        n++;
        matchContent(json, candidate, `chain:${endpoint}:${height}`, "CHAIN", "chain");
        matchContent(json, prompt, `chain:${endpoint}:${height}`, "CHAIN", "chain");
        matchSeeds(json, `chain:${endpoint}:${height}`, "CHAIN");
      }
      chainReport.push({ endpoint, blocks_scanned: n });
    }
  }
  const count = (k) => hits.filter((h) => h.kind === k).length;
  const report = {
    scanner: SCANNER_VERSION,
    needles: { candidate: candidate.size, prompt: prompt.size, seeds: seedList.length },
    files_scanned: files,
    exempt: [...exemptSet],
    chain: chain ? chainReport : "SKIPPED",
    log_hits: count("log"),
    chain_hits: count("chain"),
    secret_hits: count("secret"),
    public_evidence_hits: count("public"),
    hits,
  };
  const incomplete = [];
  if (candidate.size === 0) incomplete.push("NO_CANDIDATE_NEEDLES: no input/ or raw-result/ file under --root");
  if (seedList.length === 0) incomplete.push("NO_SEEDS: no key file given");
  if (!chain) incomplete.push("CHAIN_NOT_SCANNED");
  else if (chainReport.some((c) => c.blocks_scanned === 0)) incomplete.push("CHAIN_EMPTY: an endpoint returned no block");
  report.incomplete = incomplete;
  report.status = hits.length ? "FAIL" : incomplete.length ? "INCOMPLETE" : "PASS";
  return report;
}

// ── CLI ──────────────────────────────────────────────────────────────────
async function* rpcBlocks(endpoint) {
  const call = async (method, params) => {
    const r = await fetch(endpoint, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }) });
    const j = await r.json();
    if (j.error) throw new Error(`${method}: ${j.error.message}`);
    return j.result;
  };
  const latest = await call("get_block_height");
  for (let h = 0; h <= latest; h++) yield [h, JSON.stringify(await call("get_block_by_height", { height: h }))];
}
function readConf(file) {
  const cfg = {};
  for (const l of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    const m = l.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/);
    if (m && !l.trimStart().startsWith("#")) cfg[m[1]] = m[2];
  }
  return cfg;
}
async function main(argv) {
  const all = (n) => argv.flatMap((a, i) => (a === n ? [argv[i + 1]] : []));
  const one = (n) => all(n)[0];
  const root = one("--root");
  if (!root || !fs.existsSync(root)) {
    console.error("o1-privacy-scan: --root <O1 batch or observation dir> is required and must exist");
    return 2;
  }
  const keyFiles = all("--key-file");
  const logs = all("--log");
  let rpc = all("--rpc");
  const conf = one("--launch-config");
  if (conf) {
    const cfg = readConf(conf);
    for (const k of ["MBONGO_MVP_KEY_FILE", "MBONGO_MVP_EXECUTOR_KEY_FILE"]) if (cfg[k]) keyFiles.push(cfg[k]);
    if (cfg.MBONGO_MVP_STATE_DIR) {
      // Every launch-state file except the node data directories (RocksDB;
      // chain content is scanned through RPC instead).
      const state = cfg.MBONGO_MVP_STATE_DIR;
      for (const f of walk(state)) if (!rel(state, f).split("/").includes("chain")) logs.push(f);
    }
    if (rpc.length === 0) rpc = LAUNCH_RPC;
  }
  const llama = path.join(os.homedir(), ".cache", "mbongo", "inference", "logs", "llama-server.log");
  if (fs.existsSync(llama)) logs.push(llama);
  const promptFile = one("--prompt") || (() => {
    const vec = JSON.parse(fs.readFileSync(path.join(ROOT, "test-vectors", "inference", "kaayu-cv-extract-v3.json"), "utf8"));
    return path.join(ROOT, vec.prompt_template_path);
  })();
  const skipChain = argv.includes("--skip-chain");
  const out = one("--out") || path.join(root, "privacy", `o1-privacy-scan-${new Date().toISOString().replace(/[:.]/g, "-")}.json`);
  if (fs.existsSync(out)) {
    console.error(`o1-privacy-scan: ${out} exists; refusing to overwrite an earlier report`);
    return 2;
  }
  const previousReports = new Set(walk(path.join(root, "privacy")).filter((f) => path.basename(f).startsWith("o1-privacy-scan-")).map((f) => path.resolve(f)));
  const report = await scan({
    root,
    logs,
    seeds: keyFiles.filter((f) => fs.existsSync(f)).map((f) => fs.readFileSync(f, "utf8")),
    promptLines: fs.readFileSync(promptFile, "utf8").split(/\r?\n/),
    exempt: all("--exempt"),
    chain: skipChain || rpc.length === 0 ? null : rpc.map((endpoint) => ({ endpoint, blocks: rpcBlocks(endpoint) })),
    previousReports,
  });
  report.at = new Date().toISOString();
  report.root = path.resolve(root);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, JSON.stringify(report, null, 2), { flag: "wx" });
  // Counts only: never a needle, never a key.
  console.log(`${report.status} log_hits=${report.log_hits} chain_hits=${report.chain_hits} secret_hits=${report.secret_hits} public_evidence_hits=${report.public_evidence_hits} report=${out}${report.incomplete.length ? ` incomplete=${report.incomplete.join(",")}` : ""}`);
  return report.status === "PASS" ? 0 : report.status === "FAIL" ? 1 : 2;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).then(
    (code) => process.exit(code),
    (e) => {
      console.error(`o1-privacy-scan: ${e.message}`);
      process.exit(2);
    },
  );
}
