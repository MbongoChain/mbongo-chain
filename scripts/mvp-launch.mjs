#!/usr/bin/env node
// Controlled MVP launch operations (docs/product/MVP_LAUNCH_RUNBOOK.md).
//
//   node scripts/mvp-launch.mjs keygen <key file>       (a fresh private seed, operator-only)
//   node scripts/mvp-launch.mjs verify --config <env file>
//   node scripts/mvp-launch.mjs start  --config <env file> [--fund]
//   node scripts/mvp-launch.mjs health --config <env file>
//   node scripts/mvp-launch.mjs smoke  --config <env file>
//   node scripts/mvp-launch.mjs stop   --config <env file>
//
// The configuration is a KEY=VALUE file (template: config/mvp-launch.env.example).
// It names key FILES, never keys; nothing here prints a key, a capability, a
// CV or a result. State (pids, logs, smoke outputs) lives in
// MBONGO_MVP_STATE_DIR, which must be outside the repository.
//
// This script operates the shipped binaries; it adds no product behaviour.

import { spawn, spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const EXE = process.platform === "win32" ? ".exe" : "";
const BIN = (n) => path.join(ROOT, "target", "release", n + EXE);
const INFER_HOME = path.join(os.homedir(), ".cache", "mbongo", "inference");
const RUNTIME_EXE = path.join(INFER_HOME, "runtime", process.platform === "win32" ? "b11669-win-cpu-x64/llama-server.exe" : "b11669-ubuntu-x64/llama-b11669/llama-server");
const PINNED_RUNTIME_SHA256 = process.platform === "win32" ? "6a4635aeb35656153d4bfeeddf761d24ed84aada9c9108d580737cbed465141b" : "3f2b53013c2eb596c8aca87e4740240a41f95bcebf72e61084d6d5ba0db761cc";
const ACTIVE_VECTOR = path.join(ROOT, "test-vectors", "inference", "kaayu-cv-extract-v3.json");
const FIXTURE = path.join(ROOT, "crates", "mbongo-compute", "fixtures", "synthetic-cv.txt");
const INJECTION_CORPUS = path.join(ROOT, "test-vectors", "mvp", "mvp-e-injection-corpus-v1.json");
const NODES = [
  { name: "producer", rpc: 39944, rest: 38080, p2p: 40333, producer: true },
  { name: "follower1", rpc: 39945, rest: 38081, p2p: 40334 },
  { name: "follower2", rpc: 39946, rest: 38082, p2p: 40335 },
];
const LLAMA_PORT = 9931;

// ── configuration ─────────────────────────────────────────────────────────
const [cmd, ...rest] = process.argv.slice(2);
// The documented DEV / TEST seeds (DEVNET_V0.4_OPERATIONS.md): public, so
// never acceptable as the launch client or executor key. 0xAA… is the
// genesis-funded devnet account and may fund.
const PUBLIC_TEST_SEEDS = ["aa", "e1", "e9", "5e", "c0", "bb", "07"].map((b) => b.repeat(32));
if (cmd === "keygen") {
  const file = rest[0];
  if (!file) {
    console.error("usage: mvp-launch.mjs keygen <key file>");
    process.exit(2);
  }
  if (fs.existsSync(file)) {
    console.error(`mvp-launch: ${file} exists; refusing to overwrite a key`);
    process.exit(2);
  }
  fs.writeFileSync(file, `${crypto.randomBytes(32).toString("hex")}\n`, { mode: 0o600 });
  if (process.platform === "win32") {
    spawnSync("icacls", [file, "/inheritance:r", "/grant:r", `${process.env.USERNAME}:R`], { stdio: "ignore" });
  }
  console.log(`wrote a new 32-byte seed to ${file} (contents not shown)`);
  process.exit(0);
}
const opt = (n) => {
  const i = rest.indexOf(n);
  return i >= 0 ? rest[i + 1] : undefined;
};
const flag = (n) => rest.includes(n);
const die = (m, code = 2) => {
  console.error(`mvp-launch: ${m}`);
  process.exit(code);
};
if (!["verify", "start", "health", "smoke", "stop"].includes(cmd)) die("usage: mvp-launch.mjs verify|start|health|smoke|stop --config <file>");
const cfgPath = opt("--config") || die("--config <env file> is required");
const cfg = {};
for (const line of fs.readFileSync(cfgPath, "utf8").split(/\r?\n/)) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/);
  if (m && !line.trimStart().startsWith("#")) cfg[m[1]] = m[2];
}
for (const k of ["MBONGO_MVP_STATE_DIR", "MBONGO_MVP_KEY_FILE", "MBONGO_MVP_EXECUTOR_KEY_FILE"]) {
  if (!cfg[k] || cfg[k].startsWith("<")) die(`${k} is not set in ${cfgPath}`);
}
const STATE = path.resolve(cfg.MBONGO_MVP_STATE_DIR);
if (STATE.startsWith(ROOT + path.sep)) die("MBONGO_MVP_STATE_DIR must be outside the repository");
fs.mkdirSync(STATE, { recursive: true });
const ENV = { ...process.env };
for (const [k, v] of Object.entries(cfg)) if (k.startsWith("MBONGO_")) ENV[k] = v;
const FRONT = cfg.MBONGO_MVP_FRONT || "http://127.0.0.1:8740";
const MEMORY_ALERT_MIB = Number(cfg.MBONGO_MVP_LLAMA_MEMORY_ALERT_MIB || 7168);
const DISK_ALERT_GIB = Number(cfg.MBONGO_MVP_DISK_ALERT_GIB || 5);
const statePath = path.join(STATE, "launch-state.json");
const readState = () => (fs.existsSync(statePath) ? JSON.parse(fs.readFileSync(statePath, "utf8")) : null);
const writeState = (s) => fs.writeFileSync(statePath, JSON.stringify(s, null, 2));

// ── helpers ───────────────────────────────────────────────────────────────
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString(), ...a);
const sha256 = (b) => crypto.createHash("sha256").update(b).digest("hex");
function sha256File(p) {
  return new Promise((res, rej) => {
    const h = crypto.createHash("sha256");
    fs.createReadStream(p).on("data", (d) => h.update(d)).on("end", () => res(h.digest("hex"))).on("error", rej);
  });
}
function ps(script) {
  if (process.platform !== "win32") return "";
  return (spawnSync("powershell", ["-NoProfile", "-Command", script], { encoding: "utf8" }).stdout || "").trim();
}
function psJson(script) {
  try {
    const t = ps(script);
    return t ? JSON.parse(t) : null;
  } catch {
    return null;
  }
}
const git = (...a) => spawnSync("git", a, { cwd: ROOT, encoding: "utf8" }).stdout.trim();
async function rpc(port, method, params) {
  const r = await fetch(`http://127.0.0.1:${port}/rpc`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }) });
  const j = await r.json();
  if (j.error) throw new Error(`${method}: ${j.error.message}`);
  return j.result;
}
function run(bin, args, { stdin = null } = {}) {
  return new Promise((resolve) => {
    const c = spawn(bin, args, { cwd: ROOT, env: ENV });
    let out = "";
    let err = "";
    c.stdout.on("data", (d) => (out += d));
    c.stderr.on("data", (d) => (err += d));
    c.on("close", (code) => resolve({ code, stdout: out, stderr: err }));
    c.stdin.end(stdin ?? undefined);
  });
}
function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

// ── verify ────────────────────────────────────────────────────────────────
async function verifyArtifacts() {
  const v = { head: git("rev-parse", "HEAD"), dirty_files: git("status", "--short").split("\n").filter(Boolean).length };
  const vec = JSON.parse(fs.readFileSync(ACTIVE_VECTOR, "utf8"));
  v.profile = vec.profile;
  v.prompt_sha256_ok = sha256(fs.readFileSync(path.join(ROOT, vec.prompt_template_path))) === vec.prompt_template_sha256;
  v.schema_sha256_ok = sha256(fs.readFileSync(path.join(ROOT, vec.output_schema_path))) === vec.output_schema_sha256;
  v.runtime_sha256_ok = fs.existsSync(RUNTIME_EXE) && (await sha256File(RUNTIME_EXE)) === PINNED_RUNTIME_SHA256;
  const model = path.join(INFER_HOME, "models", vec.job_spec.model.artifact_name);
  v.model_size_ok = fs.existsSync(model) && fs.statSync(model).size === vec.job_spec.model.artifact_size_bytes;
  log("hashing the model…");
  v.model_sha256_ok = v.model_size_ok && (await sha256File(model)) === vec.job_spec.model.artifact_sha256;
  v.binaries = {};
  for (const b of ["mbongo-node", "mvp_service", "mbongo-mvp"]) v.binaries[b] = fs.existsSync(BIN(b)) ? (await sha256File(BIN(b))).slice(0, 16) : null;
  const seedOf = (f) => (f && fs.existsSync(f) ? fs.readFileSync(f, "utf8").trim().toLowerCase() : null);
  v.client_key_private = !PUBLIC_TEST_SEEDS.includes(seedOf(cfg.MBONGO_MVP_KEY_FILE));
  v.executor_key_private = !PUBLIC_TEST_SEEDS.includes(seedOf(cfg.MBONGO_MVP_EXECUTOR_KEY_FILE));
  v.client_and_executor_keys_distinct = seedOf(cfg.MBONGO_MVP_KEY_FILE) !== seedOf(cfg.MBONGO_MVP_EXECUTOR_KEY_FILE);
  const execAddr = fs.existsSync(cfg.MBONGO_MVP_EXECUTOR_KEY_FILE) ? keyFromSeedFile(cfg.MBONGO_MVP_EXECUTOR_KEY_FILE).address : null;
  v.configured_executor_matches_key = (cfg.MBONGO_MVP_EXECUTOR || "").toLowerCase().replace(/^0x/, "") === execAddr;
  v.ok = v.configured_executor_matches_key && v.client_key_private && v.executor_key_private && v.client_and_executor_keys_distinct && v.dirty_files === 0 && v.prompt_sha256_ok && v.schema_sha256_ok && v.runtime_sha256_ok && v.model_sha256_ok && Object.values(v.binaries).every(Boolean);
  return v;
}

// ── start ─────────────────────────────────────────────────────────────────
function spawnDetached(name, bin, args, extraEnv = {}) {
  const out = fs.openSync(path.join(STATE, `${name}.out`), "a");
  const err = fs.openSync(path.join(STATE, `${name}.err`), "a");
  const c = spawn(bin, args, { cwd: ROOT, env: { ...ENV, RUST_LOG: "info", ...extraEnv }, detached: true, stdio: ["ignore", out, err] });
  c.unref();
  return c.pid;
}
async function peerIdOf(name) {
  for (let i = 0; i < 80; i++) {
    const t = fs.existsSync(path.join(STATE, `${name}.out`)) ? fs.readFileSync(path.join(STATE, `${name}.out`), "utf8") : "";
    const m = t.match(/PeerId:\s*(\S+)/);
    if (m) return m[1];
    await sleep(250);
  }
  throw new Error(`${name}: no PeerId`);
}
async function chainHealth() {
  const heights = await Promise.all(NODES.map((n) => rpc(n.rpc, "get_block_height").catch(() => null)));
  if (heights.some((h) => h === null)) return { ok: false, heights, reason: "a node is not answering" };
  const min = Math.min(...heights);
  const blocks = await Promise.all(NODES.map((n) => rpc(n.rpc, "get_block_by_height", { height: min })));
  const same = blocks.every((b) => JSON.stringify(b) === JSON.stringify(blocks[0]));
  const skew = Math.max(...heights) - min;
  return { ok: same && skew <= 3, heights, common_height: min, same_block_at_common_height: same, height_skew: skew };
}
async function start() {
  if (readState()) die("a launch state already exists: run stop first");
  const v = await verifyArtifacts();
  if (!v.ok) die(`artifact verification failed: ${JSON.stringify(v)}`);
  for (const f of ["producer", "follower1", "follower2", "mvp_service"]) for (const s of ["out", "err"]) fs.rmSync(path.join(STATE, `${f}.${s}`), { force: true });
  const state = { started_at: new Date().toISOString(), head: v.head, nodes: {}, service: null };
  const dataRoot = path.join(STATE, "chain");
  // Every node port on loopback: RPC and REST by the node's default, P2P by flag.
  const args = (n) => ["--rpc-port", String(n.rpc), "--rest-port", String(n.rest), "--p2p-port", String(n.p2p), "--p2p-host", "127.0.0.1", "--data-dir", path.join(dataRoot, n.name)];
  state.nodes.producer = spawnDetached("producer", BIN("mbongo-node"), [...args(NODES[0]), "--producer", "--block-time", "1"]);
  const boot = `/ip4/127.0.0.1/tcp/${NODES[0].p2p}/p2p/${await peerIdOf("producer")}`;
  for (const n of NODES.slice(1)) state.nodes[n.name] = spawnDetached(n.name, BIN("mbongo-node"), [...args(n), "--bootnodes", boot]);
  writeState(state);
  let h = null;
  for (let i = 0; i < 120; i++) {
    await sleep(500);
    h = await chainHealth().catch(() => null);
    if (h && h.ok && h.common_height >= 3) break;
  }
  if (!h || !h.ok) die(`devnet did not converge: ${JSON.stringify(h)}`);
  log(`devnet converged: heights ${h.heights}`);
  if (flag("--fund")) {
    // The executor's account must exist before it anchors (RFC 0002 rule d)
    // and the client's before it submits. fund-executor transfers from the
    // funder to the address of MBONGO_MVP_EXECUTOR_KEY_FILE; pointing that at
    // the client key funds the client the same way.
    for (const [role, keyFile] of [["executor", cfg.MBONGO_MVP_EXECUTOR_KEY_FILE], ["client", cfg.MBONGO_MVP_KEY_FILE]]) {
      const saved = ENV.MBONGO_MVP_EXECUTOR_KEY_FILE;
      ENV.MBONGO_MVP_EXECUTOR_KEY_FILE = keyFile;
      const f = await run(BIN("mvp_service"), ["fund-executor", "--amount", "1000"]);
      ENV.MBONGO_MVP_EXECUTOR_KEY_FILE = saved;
      if (f.code !== 0) die(`funding the ${role} account failed`);
      log(`${role} account funded`);
    }
  }
  state.service = spawnDetached("mvp_service", BIN("mvp_service"), ["serve", "--bind", cfg.MBONGO_MVP_BIND || "127.0.0.1:8740"]);
  writeState(state);
  for (let i = 0; i < 600; i++) {
    const t = fs.readFileSync(path.join(STATE, "mvp_service.out"), "utf8");
    if (t.includes("FRONT_BIND=")) {
      log("service ready");
      process.stdout.write(t.split(/\r?\n/).filter((l) => /^(WORKER_|FRONT_BIND|EXECUTOR|RUNTIME_PROFILE|STATE|  )/.test(l)).join("\n") + "\n");
      return;
    }
    if (!alive(state.service)) die("mvp_service exited during start-up: see mvp_service.err");
    await sleep(250);
  }
  die("mvp_service did not become ready");
}

// ── health and metrics ────────────────────────────────────────────────────
function metrics() {
  const err = fs.existsSync(path.join(STATE, "mvp_service.err")) ? fs.readFileSync(path.join(STATE, "mvp_service.err"), "utf8") : "";
  const lines = err.split(/\r?\n/);
  const front = lines.filter((l) => l.includes("mvp-front:") && / -> \d{3} /.test(l));
  const count = (re) => lines.filter((l) => re.test(l)).length;
  const codes = {};
  for (const l of front) {
    const m = l.match(/ -> (\d{3}) (\S+)/);
    if (m) codes[`${m[1]} ${m[2]}`] = (codes[`${m[1]} ${m[2]}`] || 0) + 1;
  }
  const outcomes = {};
  for (const m of err.matchAll(/cpu-inference: input \S+ .*duration_ms (\d+) .*outcome ([A-Z_]+)/g)) outcomes[m[2]] = (outcomes[m[2]] || 0) + 1;
  return {
    requests: front.length,
    responses_by_code: codes,
    jobs_registered: codes["200 OK"] !== undefined ? front.filter((l) => l.includes("/register") && l.includes("200 OK")).length : 0,
    receipts_observed: count(/control-plane: task \S+ completed at height/),
    inference_outcomes: outcomes,
    attempt_failures: count(/reports Failed\(/),
    parked_tasks: count(/parked after \d+ failed attempts/),
    auth_failures: front.filter((l) => / AUTH_/.test(l)).length,
    auth_replays: front.filter((l) => l.includes("AUTH_REPLAYED")).length,
    llama_server_restarts: count(/llama-server not alive; restarting/),
    worker_errors: count(/worker attempt error/),
  };
}
async function health() {
  const st = readState();
  const report = { at: new Date().toISOString(), alerts: [] };
  report.chain = await chainHealth().catch((e) => ({ ok: false, reason: e.message }));
  if (!report.chain.ok) report.alerts.push("NODE_DIVERGENCE_OR_DOWN: stop new task submissions until the devnet converges");
  try {
    const r = await fetch(`${FRONT}/v1/tasks/${"00".repeat(32)}`);
    report.front = { up: r.status === 401 && (await r.text()) === '{"error":"AUTH_MISSING"}' };
  } catch {
    report.front = { up: false };
  }
  if (!report.front.up) report.alerts.push("HTTP_FRONT_DOWN");
  try {
    const r = await fetch(`http://127.0.0.1:${LLAMA_PORT}/health`);
    report.llama_server = { http_status: r.status, ready: r.ok };
  } catch {
    report.llama_server = { ready: false };
  }
  if (!report.llama_server.ready) report.alerts.push("LLAMA_SERVER_UNAVAILABLE (the worker restarts it before the next job; alert if it persists)");
  // Network exposure: every listening socket of the stack must be loopback.
  const ports = [...NODES.flatMap((n) => [n.rpc, n.rest, n.p2p]), Number(new URL(FRONT).port || 8740), LLAMA_PORT];
  const listen = psJson(`Get-NetTCPConnection -State Listen -LocalPort ${ports.join(",")} -ErrorAction SilentlyContinue | Select-Object LocalAddress,LocalPort | ConvertTo-Json`);
  report.listening = [].concat(listen || []).map((l) => `${l.LocalAddress}:${l.LocalPort}`);
  const exposed = report.listening.filter((a) => !/^(127.0.0.1|::1):/.test(a));
  if (exposed.length) report.alerts.push(`NON_LOOPBACK_LISTENER: ${exposed.join(" ")} — stop and fix before launch`);
  report.service_process = st?.service ? alive(st.service) : false;
  if (!report.service_process) report.alerts.push("MVP_SERVICE_NOT_RUNNING (in-memory inputs and results are lost)");
  const procs = psJson("Get-Process mvp_service,llama-server -ErrorAction SilentlyContinue | Select-Object Name,Id,WorkingSet64 | ConvertTo-Json");
  report.memory_mib = Object.fromEntries([].concat(procs || []).map((p) => [p.Name, Math.round(p.WorkingSet64 / 2 ** 20)]));
  if ((report.memory_mib["llama-server"] || 0) > MEMORY_ALERT_MIB) report.alerts.push(`LLAMA_MEMORY_ABOVE_${MEMORY_ALERT_MIB}_MIB: between jobs, stop llama-server; the worker restarts it on the next job`);
  const drive = psJson(`Get-PSDrive -Name '${STATE.slice(0, 1)}' | Select-Object Free | ConvertTo-Json`);
  report.disk_free_gib = drive ? +(drive.Free / 2 ** 30).toFixed(1) : null;
  if (report.disk_free_gib !== null && report.disk_free_gib < DISK_ALERT_GIB) report.alerts.push(`DISK_FREE_BELOW_${DISK_ALERT_GIB}_GIB`);
  report.metrics = metrics();
  if (report.metrics.parked_tasks > 0) report.alerts.push(`PARKED_TASKS=${report.metrics.parked_tasks}: the client must resubmit; see runbook`);
  if (report.metrics.auth_replays > 0) report.alerts.push(`AUTH_REPLAYS=${report.metrics.auth_replays}: investigate the source`);
  report.healthy = report.alerts.length === 0;
  return report;
}

// ── request signing, independently of the Rust client ─────────────────────
function keyFromSeedFile(file) {
  const seed = Buffer.from(fs.readFileSync(file, "utf8").trim(), "hex");
  const key = crypto.createPrivateKey({ key: Buffer.concat([Buffer.from("302e020100300506032b657004220420", "hex"), seed]), format: "der", type: "pkcs8" });
  const address = crypto.createPublicKey(key).export({ format: "der", type: "spki" }).subarray(-32).toString("hex");
  return { key, address };
}
// ── smoke ─────────────────────────────────────────────────────────────────
async function smoke() {
  const out = path.join(STATE, "smoke");
  fs.mkdirSync(out, { recursive: true });
  const report = { at: new Date().toISOString(), head: git("rev-parse", "HEAD") };
  const h0 = await health();
  if (!h0.healthy && !(h0.alerts.length === 1 && h0.alerts[0].startsWith("AUTH_REPLAYS=")))
    die(`environment not healthy before the smoke: ${JSON.stringify(h0.alerts)}`, 1);
  const job = async (name, text) => {
    const t = Date.now();
    const r = await run(BIN("mbongo-mvp"), ["submit", "-", "--wait", "--json"], { stdin: text });
    fs.writeFileSync(path.join(out, `${name}.stderr`), r.stderr);
    fs.writeFileSync(path.join(out, `${name}.stdout`), r.stdout);
    let rec = null;
    try {
      rec = JSON.parse(r.stdout.trim().split(/\r?\n/).pop());
    } catch {}
    const res = rec ? await run(BIN("mbongo-mvp"), ["result", rec.task_id, "--out", path.join(out, `${name}.result.json`)]) : null;
    return { exit: r.code, wall_ms: Date.now() - t, rec, result_exit: res?.code };
  };
  // 1. Synthetic launch job.
  const a = await job("launch", fs.readFileSync(FIXTURE, "utf8"));
  report.launch = {
    exit: a.exit,
    task_id: a.rec?.task_id,
    receipt_integrity: a.rec?.receipt_integrity?.status,
    result_integrity: a.rec?.result_integrity?.status,
    schema_conformity: a.rec?.schema_conformity?.status,
    business_accuracy: a.rec?.business_accuracy?.status,
    injection_screen: a.rec?.injection_screen?.status,
    total_ms: a.rec?.timings?.total_ms,
    receipt_height: a.rec?.receipt_height,
  };
  report.launch.pass = a.exit === 0 && report.launch.receipt_integrity === "VERIFIED" && report.launch.result_integrity === "VERIFIED" && report.launch.schema_conformity === "VERIFIED" && report.launch.business_accuracy === "NOT_ASSESSED";
  // 2. Adversarial job: the MVP-D D18 injection, verbatim.
  const e11 = JSON.parse(fs.readFileSync(INJECTION_CORPUS, "utf8")).cases.find((c) => c.id === "E-P11");
  const b = await job("adversarial", e11.cv_text);
  const resText = fs.existsSync(path.join(out, "adversarial.result.json")) ? fs.readFileSync(path.join(out, "adversarial.result.json"), "utf8") : "";
  const parsed = resText ? JSON.parse(resText) : {};
  report.adversarial = {
    case: "E-P11 (MVP-D D18 verbatim)",
    exit: b.exit,
    task_id: b.rec?.task_id,
    receipt_integrity: b.rec?.receipt_integrity?.status,
    result_integrity: b.rec?.result_integrity?.status,
    schema_conformity: b.rec?.schema_conformity?.status,
    business_accuracy: b.rec?.business_accuracy?.status,
    injection_screen: b.rec?.injection_screen?.status,
    forced_sentinel_present: /hacked/i.test(resText),
    forced_seniority: parsed.seniority === "executive",
    seniority: parsed.seniority,
  };
  report.adversarial.pass = b.exit === 0 && report.adversarial.receipt_integrity === "VERIFIED" && report.adversarial.result_integrity === "VERIFIED" && report.adversarial.schema_conformity === "VERIFIED" && !report.adversarial.forced_sentinel_present && !report.adversarial.forced_seniority && report.adversarial.injection_screen === "CLEAR";
  // 3. Real auth probes against the running front: replay and tamper.
  report.auth = await authProbes(a.rec?.task_id);
  // 4. Health after, privacy scan.
  report.health_after = await health();
  report.privacy = await privacyScan(out, [fs.readFileSync(FIXTURE, "utf8"), e11.cv_text]);
  // The smoke's own replay probe is one expected AUTH_REPLAYED; any other
  // alert, or any other replay, fails the smoke.
  const replaysBySmoke = report.health_after.metrics.auth_replays - h0.metrics.auth_replays;
  report.health_after.unexpected_alerts = report.health_after.alerts.filter((a) => !(a.startsWith("AUTH_REPLAYS=") && replaysBySmoke === 1 && h0.metrics.auth_replays === 0));
  report.pass = report.launch.pass && report.adversarial.pass && report.auth.pass && report.privacy.pass && replaysBySmoke === 1 && report.health_after.unexpected_alerts.length === 0;
  fs.writeFileSync(path.join(STATE, "smoke-report.json"), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  process.exit(report.pass ? 0 : 1);
}

// The probes sign requests with node:crypto, independently of the Rust
// client. node:crypto has no BLAKE3, so every probe signs the empty body
// (whose BLAKE3 digest is the published constant below); the body-tamper
// probe sends a non-empty body under that signature, which the front must
// refuse before reading it as an upload.
const BLAKE3_EMPTY = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
async function authProbes(taskId) {
  const k = keyFromSeedFile(cfg.MBONGO_MVP_KEY_FILE);
  const send = async (method, p, body, signedBody, nonce, tsOffset = 0) => {
    const ts = Math.floor(Date.now() / 1000) + tsOffset;
    const n = nonce.toString("hex");
    if (signedBody !== "") throw new Error("only empty signed bodies are supported");
    const msg = `mbongo-mvp-http:v1\n${method}\n${p}\n${ts}\n${n}\n${BLAKE3_EMPTY}`;
    const sig = crypto.sign(null, Buffer.from(msg), k.key).toString("hex");
    const r = await fetch(`${FRONT}${p}`, { method, headers: { "x-mbongo-address": k.address, "x-mbongo-timestamp": String(ts), "x-mbongo-nonce": n, "x-mbongo-signature": sig }, body: body === "" ? undefined : body });
    return { status: r.status, body: await r.text() };
  };
  const p = `/v1/tasks/${taskId}`;
  const nonce = crypto.randomBytes(16);
  const first = await send("GET", p, "", "", nonce);
  const replay = await send("GET", p, "", "", nonce);
  const otherTask = await send("GET", `/v1/tasks/${"77".repeat(32)}`, "", "", crypto.randomBytes(16));
  const stale = await send("GET", p, "", "", crypto.randomBytes(16), -1000);
  const tamperedBody = await (async () => {
    const ts = Math.floor(Date.now() / 1000);
    const n = crypto.randomBytes(16).toString("hex");
    const msg = `mbongo-mvp-http:v1\nPUT\n/v1/inputs\n${ts}\n${n}\n${BLAKE3_EMPTY}`;
    const sig = crypto.sign(null, Buffer.from(msg), k.key).toString("hex");
    const r = await fetch(`${FRONT}/v1/inputs`, { method: "PUT", headers: { "content-type": "application/json", "x-mbongo-address": k.address, "x-mbongo-timestamp": String(ts), "x-mbongo-nonce": n, "x-mbongo-signature": sig }, body: '{"task_id":"00","input":{"contract":"kaayu-cv-extract-input:v1","cv_text":"tampered"}}' });
    return { status: r.status, body: await r.text() };
  })();
  const res = {
    signer_address_prefix: k.address.slice(0, 16),
    genuine: first.status,
    replay: `${replay.status} ${replay.body}`,
    tampered_body: `${tamperedBody.status} ${tamperedBody.body}`,
    stale_timestamp: `${stale.status} ${stale.body}`,
    unknown_task_signed_by_owner: `${otherTask.status} ${otherTask.body}`,
  };
  res.pass = first.status === 200 && replay.body === '{"error":"AUTH_REPLAYED"}' && tamperedBody.body === '{"error":"AUTH_BAD_SIGNATURE"}' && stale.body === '{"error":"AUTH_STALE"}' && otherTask.status === 404;
  return res;
}

async function privacyScan(out, cvTexts) {
  const needles = new Set();
  for (const t of cvTexts) for (const l of t.split("\n")) if (l.trim().length >= 12) needles.add(l.trim());
  for (const f of fs.readdirSync(out).filter((f) => f.endsWith(".result.json"))) {
    const t = fs.readFileSync(path.join(out, f), "utf8");
    needles.add(t);
    for (const piece of t.split(/","|":"|\{|\}|\[|\]/)) if (piece.trim().length >= 16) needles.add(piece.trim());
  }
  const seeds = [cfg.MBONGO_MVP_KEY_FILE, cfg.MBONGO_MVP_EXECUTOR_KEY_FILE].filter((f) => f && fs.existsSync(f)).map((f) => fs.readFileSync(f, "utf8").trim());
  const prompt = JSON.parse(fs.readFileSync(ACTIVE_VECTOR, "utf8")).prompt_template_path;
  for (const l of fs.readFileSync(path.join(ROOT, prompt), "utf8").split("\n")) if (l.trim().length >= 16) needles.add(l.trim());
  const forms = (n) => [n, Buffer.from(n).toString("hex"), Array.from(Buffer.from(n)).join(","), JSON.stringify(n).slice(1, -1)].filter((f) => f.length >= 12);
  const files = [...["mvp_service.out", "mvp_service.err", "producer.err", "producer.out", "follower1.err", "follower2.err"].map((f) => path.join(STATE, f)), path.join(INFER_HOME, "logs", "llama-server.log"), ...fs.readdirSync(out).filter((f) => f.endsWith(".stderr") || f.endsWith(".stdout")).map((f) => path.join(out, f))].filter((f) => fs.existsSync(f));
  let logHits = 0;
  let secretHits = 0;
  for (const f of files) {
    const t = fs.readFileSync(f, "utf8");
    for (const n of needles) for (const x of forms(n)) if (t.includes(x)) logHits++;
    for (const s of seeds) if (t.includes(s)) secretHits++;
  }
  let chainHits = 0;
  let blocks = 0;
  const latest = await rpc(NODES[0].rpc, "get_block_height");
  for (let h = 0; h <= latest; h++) {
    const b = JSON.stringify(await rpc(NODES[0].rpc, "get_block_by_height", { height: h }));
    blocks++;
    for (const n of needles) for (const x of forms(n)) if (b.includes(x)) chainHits++;
    for (const s of seeds) if (b.includes(s)) secretHits++;
  }
  return { files_scanned: files.length, blocks_scanned: blocks, needles: needles.size, log_hits: logHits, chain_hits: chainHits, secret_hits: secretHits, pass: logHits === 0 && chainHits === 0 && secretHits === 0 };
}

// ── stop ──────────────────────────────────────────────────────────────────
function stop() {
  const st = readState();
  if (!st) die("no launch state");
  console.log("Stopping: uploaded inputs, results, registrations, parked tasks and replay history held by mvp_service are lost now.");
  const kill = (pid) => {
    try {
      process.kill(pid);
    } catch {}
  };
  if (st.service) kill(st.service);
  ps("Get-Process llama-server -ErrorAction SilentlyContinue | Stop-Process -Force");
  for (const pid of Object.values(st.nodes)) kill(pid);
  fs.rmSync(statePath);
  log("stopped");
}

if (cmd === "verify") {
  const v = await verifyArtifacts();
  console.log(JSON.stringify(v, null, 2));
  process.exit(v.ok ? 0 : 1);
} else if (cmd === "start") await start();
else if (cmd === "health") {
  const h = await health();
  console.log(JSON.stringify(h, null, 2));
  process.exit(h.healthy ? 0 : 1);
} else if (cmd === "smoke") await smoke();
else stop();
