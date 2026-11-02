#!/usr/bin/env node
// MVP-D — 20-job Kaayu acceptance run (docs/product/MVP_D_ACCEPTANCE.md).
//
// Measures the frozen MVP path; changes nothing about it. It drives the
// shipped binaries only: a three-node native devnet (`mbongo-node`), one
// long-lived `mvp_service serve` (front + control plane + data plane + MVP-B
// worker + pinned llama-server), and the `mbongo-mvp` CLI exactly as a
// client runs it: `mbongo-mvp submit - --wait --json`, sequentially, one job
// per corpus case. One job (default D07) carries the controlled recovery
// case: llama-server is killed mid-inference and the job must complete on a
// retry with exactly one anchored receipt.
//
// Afterwards it re-verifies every task, audits every receipt on all three
// nodes, scans chain blocks and every log for the corpus' private text and
// for the results, and writes sanitized evidence (no CV text, no result
// content) to --evidence. Private material (CV texts, result bytes, logs)
// stays in --run-dir, outside the repository.
//
// Requirements: a release build (`--build` runs it), the pinned runtime and
// model from scripts/setup-mvp-inference-runtime.sh, Windows on AC power
// (the reference host). Never run in CI: it needs the 2.5 GB model.
//
//   node scripts/mvp-d-acceptance.mjs --build --evidence docs/product/evidence/mvp-d
//
// Options: --run-dir <dir> (default: a fresh directory under the OS temp
// directory) · --recovery-job <Dnn> (default D07; "none" disables) ·
// --recovery-delay-ms <ms> (default 10000) · --jobs <n> (default 20) ·
// --profile-vector <file> (the golden vector of the profile the service runs;
// default the v2 vector MVP-D used) ·
// --rate <usd/h> (displayed reference cost rate, default 0.50 = the MVP
// infrastructure ceiling of MVP_v0.1.md §12) · --build · --allow-battery
// (records the run as invalid for acceptance).

import { spawn, spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const EXE = process.platform === "win32" ? ".exe" : "";
const BIN = (n) => path.join(ROOT, "target", "release", n + EXE);
const CORPUS = path.join(ROOT, "test-vectors", "mvp", "mvp-d-corpus-v1.json");
// The golden vector of the profile the service runs: v2 for MVP-D, the active
// profile for a later revalidation (--profile-vector). Its prompt and schema
// paths are verified and its prompt is a privacy-scan needle.
const DEFAULT_PROFILE_VECTOR = path.join(ROOT, "test-vectors", "inference", "kaayu-cv-extract-v2.json");
const INFER_HOME = path.join(os.homedir(), ".cache", "mbongo", "inference");
const RUNTIME_EXE = path.join(INFER_HOME, "runtime", "b11669-win-cpu-x64", "llama-server.exe");
const PINNED_RUNTIME_SHA256 = "6a4635aeb35656153d4bfeeddf761d24ed84aada9c9108d580737cbed465141b";
// Harness-profile ports of DEVNET_V0.4_OPERATIONS.md.
const NODES = [
  { name: "producer", rpc: 39944, rest: 38080, p2p: 40333, producer: true },
  { name: "follower1", rpc: 39945, rest: 38081, p2p: 40334, producer: false },
  { name: "follower2", rpc: 39946, rest: 38082, p2p: 40335, producer: false },
];
const FRONT = "127.0.0.1:8740";
// Documented DEV / TEST keys (DEVNET_V0.4_OPERATIONS.md "Identities and funding").
const CLIENT_SEED = "aa".repeat(32);
const EXECUTOR_SEED = "e1".repeat(32);

// ── arguments ─────────────────────────────────────────────────────────────
const argv = process.argv.slice(2);
const opt = (name, def) => {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : def;
};
const flag = (name) => argv.includes(name);
const RUN = path.resolve(opt("--run-dir", path.join(os.tmpdir(), `mbongo-mvp-d-${Date.now()}`)));
const EVIDENCE = opt("--evidence") && path.resolve(opt("--evidence"));
const RECOVERY_JOB = opt("--recovery-job", "D07");
const RECOVERY_DELAY_MS = Number(opt("--recovery-delay-ms", "10000"));
const JOBS = Number(opt("--jobs", "20"));
const RATE = opt("--rate", "0.50");
const PROFILE_VECTOR = path.resolve(opt("--profile-vector", DEFAULT_PROFILE_VECTOR));
if (!EVIDENCE) die("--evidence <dir> is required");
if (RUN.startsWith(ROOT + path.sep)) die("--run-dir must be outside the repository: it holds private material");
fs.mkdirSync(RUN, { recursive: true });
fs.mkdirSync(EVIDENCE, { recursive: true });

// ── small helpers ─────────────────────────────────────────────────────────
function die(msg) {
  console.error(`MVP-D: ${msg}`);
  process.exit(2);
}
const now = () => Date.now();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const sha256 = (buf) => crypto.createHash("sha256").update(buf).digest("hex");
const log = (...a) => console.log(new Date().toISOString(), ...a);
function sha256File(p) {
  return new Promise((resolve, reject) => {
    const h = crypto.createHash("sha256");
    fs.createReadStream(p).on("data", (d) => h.update(d)).on("end", () => resolve(h.digest("hex"))).on("error", reject);
  });
}
function ps(script) {
  const r = spawnSync("powershell", ["-NoProfile", "-Command", script], { encoding: "utf8" });
  return (r.stdout || "").trim();
}
function psJson(script) {
  const t = ps(script);
  try {
    return t ? JSON.parse(t) : null;
  } catch {
    return null;
  }
}
function git(...args) {
  return spawnSync("git", args, { cwd: ROOT, encoding: "utf8" }).stdout.trim();
}
async function rpc(port, method, params) {
  const r = await fetch(`http://127.0.0.1:${port}/rpc`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
  });
  const j = await r.json();
  if (j.error) throw new Error(`${method}: ${j.error.message}`);
  return j.result;
}
const hexOf = (a) => (Array.isArray(a) ? Buffer.from(a).toString("hex") : String(a).replace(/^0x/, ""));

// Runs a binary; feeds `stdin`; resolves {code, stdout, stderr, ms}.
function run(bin, args, { env = {}, stdin = null, onStderrLine = null } = {}) {
  return new Promise((resolve) => {
    const t = now();
    const child = spawn(bin, args, { cwd: ROOT, env: { ...process.env, ...env }, stdio: ["pipe", "pipe", "pipe"] });
    let out = "";
    let err = "";
    let partial = "";
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => {
      err += d;
      if (onStderrLine) {
        partial += d;
        const lines = partial.split(/\r?\n/);
        partial = lines.pop();
        lines.forEach(onStderrLine);
      }
    });
    child.on("close", (code) => resolve({ code, stdout: out, stderr: err, ms: now() - t }));
    if (stdin !== null) child.stdin.end(stdin);
    else child.stdin.end();
  });
}

// ── 1. baseline and environment ───────────────────────────────────────────
const env = {};
env.git_head = git("rev-parse", "HEAD");
env.git_branch = git("branch", "--show-current");
env.git_dirty_files = git("status", "--short").split("\n").filter(Boolean).length;
env.started_at = new Date().toISOString();
if (process.platform === "win32") {
  const cpu = psJson("Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed,CurrentClockSpeed,LoadPercentage | ConvertTo-Json");
  const cs = psJson("Get-CimInstance Win32_ComputerSystem | Select-Object TotalPhysicalMemory | ConvertTo-Json");
  const osi = psJson("Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber | ConvertTo-Json");
  const bat = psJson("Get-CimInstance Win32_Battery | Select-Object BatteryStatus,EstimatedChargeRemaining | ConvertTo-Json");
  env.cpu = cpu;
  env.ram_gib = cs ? +(cs.TotalPhysicalMemory / 2 ** 30).toFixed(1) : null;
  env.os = osi;
  env.battery = bat;
  env.power_plan = ps("powercfg /getactivescheme");
}
env.power_state = env.battery && env.battery.BatteryStatus === 2 ? "AC" : "NOT_AC";
env.cpu_threads = Number(process.env.MBONGO_INFER_THREADS || 6);
env.gpu_offload = 0;
log(`baseline ${env.git_head} (${env.git_branch}, ${env.git_dirty_files} dirty) power=${env.power_state}`);
if (env.power_state !== "AC" && !flag("--allow-battery")) die("not on AC power (Win32_Battery.BatteryStatus != 2)");
if (env.power_state !== "AC") env.acceptance_invalid = "battery";

if (flag("--build")) {
  for (const [pkg, bins] of [["mbongo-compute", ["mvp_service", "mbongo-mvp"]], ["mbongo-node", ["mbongo-node"]]]) {
    const r = spawnSync("cargo", ["build", "--release", "--locked", "-p", pkg, ...bins.flatMap((b) => ["--bin", b])], { cwd: ROOT, stdio: "inherit" });
    if (r.status !== 0) die(`build of ${pkg} failed`);
  }
}
env.binaries = {};
for (const b of ["mvp_service", "mbongo-mvp", "mbongo-node"]) {
  if (!fs.existsSync(BIN(b))) die(`missing ${BIN(b)}; run with --build`);
  env.binaries[b] = { sha256: await sha256File(BIN(b)), mtime: fs.statSync(BIN(b)).mtime.toISOString() };
}

// ── 2. corpus, profile, runtime and model, verified once ─────────────────
const corpus = JSON.parse(fs.readFileSync(CORPUS, "utf8")).cases.slice(0, JOBS);
for (const c of corpus) {
  if (sha256(Buffer.from(c.cv_text, "utf8")) !== c.cv_text_sha256) die(`${c.id}: fixture changed (sha256)`);
}
const v2 = JSON.parse(fs.readFileSync(PROFILE_VECTOR, "utf8"));
const PROMPT_PATH = path.join(ROOT, v2.prompt_template_path);
const SCHEMA_PATH = path.join(ROOT, v2.output_schema_path);
env.profile = v2.profile;
env.profile_vector = path.relative(ROOT, PROFILE_VECTOR).split(path.sep).join("/");
const verify = {};
verify.profile = v2.profile;
verify.prompt_template_sha256 = sha256(fs.readFileSync(PROMPT_PATH));
verify.output_schema_sha256 = sha256(fs.readFileSync(SCHEMA_PATH));
// Field name kept from MVP-D for comparable evidence: "the profile's assets
// match its golden vector" (v2 for MVP-D, `verify.profile` otherwise).
verify.profile_v2_verified =
  verify.prompt_template_sha256 === v2.prompt_template_sha256 && verify.output_schema_sha256 === v2.output_schema_sha256;
const modelPath = path.join(INFER_HOME, "models", v2.job_spec.model.artifact_name);
verify.runtime_sha256 = fs.existsSync(RUNTIME_EXE) ? await sha256File(RUNTIME_EXE) : null;
verify.runtime_verified = verify.runtime_sha256 === PINNED_RUNTIME_SHA256;
verify.runtime_revision = `${v2.job_spec.runtime.family} ${v2.job_spec.runtime.revision} (b11669 @ ${v2.job_spec.runtime.commit.slice(0, 8)})`;
verify.model_size_bytes = fs.existsSync(modelPath) ? fs.statSync(modelPath).size : null;
log("hashing the model (once)…");
verify.model_sha256 = fs.existsSync(modelPath) ? await sha256File(modelPath) : null;
verify.model_verified =
  verify.model_sha256 === v2.job_spec.model.artifact_sha256 && verify.model_size_bytes === v2.job_spec.model.artifact_size_bytes;
log(`profile_v2=${verify.profile_v2_verified} runtime=${verify.runtime_verified} model=${verify.model_verified}`);
if (!verify.profile_v2_verified || !verify.runtime_verified || !verify.model_verified) die("runtime, model or profile verification failed");

// ── 3. keys, three-node devnet ────────────────────────────────────────────
const keyDir = path.join(RUN, "keys");
fs.mkdirSync(keyDir, { recursive: true });
fs.writeFileSync(path.join(keyDir, "client.key"), CLIENT_SEED + "\n");
fs.writeFileSync(path.join(keyDir, "executor.key"), EXECUTOR_SEED + "\n");
const CLI_ENV = {
  MBONGO_MVP_RPC_URL: `http://127.0.0.1:${NODES[0].rpc}/rpc`,
  MBONGO_MVP_REST_URL: `http://127.0.0.1:${NODES[0].rest}`,
  MBONGO_MVP_FRONT: `http://${FRONT}`,
  MBONGO_MVP_KEY_FILE: path.join(keyDir, "client.key"),
  MBONGO_MVP_EXECUTOR_KEY_FILE: path.join(keyDir, "executor.key"),
  MBONGO_MVP_FUNDER_KEY_FILE: path.join(keyDir, "client.key"),
  MBONGO_MVP_REFERENCE_USD_PER_HOUR: RATE,
};
const children = [];
function cleanup() {
  for (const c of children) {
    try {
      c.kill();
    } catch {}
  }
  if (process.platform === "win32") ps("Get-Process llama-server -ErrorAction SilentlyContinue | Stop-Process -Force");
}
process.on("exit", cleanup);
process.on("SIGINT", () => process.exit(130));

if (process.platform === "win32") ps("Get-Process llama-server,mvp_service,mbongo-node -ErrorAction SilentlyContinue | Stop-Process -Force");
const llamaLog = path.join(INFER_HOME, "logs", "llama-server.log");
try {
  fs.rmSync(llamaLog);
} catch {}

async function spawnNode(n, bootnode) {
  const dir = path.join(RUN, "nodes", n.name);
  fs.mkdirSync(dir, { recursive: true });
  const args = ["--rpc-port", String(n.rpc), "--rest-port", String(n.rest), "--p2p-port", String(n.p2p), "--data-dir", dir];
  if (n.producer) args.push("--producer", "--block-time", "1");
  if (bootnode) args.push("--bootnodes", bootnode);
  const out = fs.createWriteStream(path.join(RUN, `${n.name}.log`));
  const child = spawn(BIN("mbongo-node"), args, { cwd: ROOT, env: { ...process.env, RUST_LOG: "info" } });
  children.push(child);
  child.stderr.pipe(out);
  const peer = await new Promise((resolve, reject) => {
    let buf = "";
    const timer = setTimeout(() => reject(new Error(`${n.name}: no PeerId within 20 s`)), 20000);
    child.stdout.on("data", (d) => {
      out.write(d);
      buf += d;
      const m = buf.match(/PeerId:\s*(\S+)/);
      if (m) {
        clearTimeout(timer);
        resolve(m[1]);
      }
    });
  });
  return { ...n, child, dir, multiaddr: `/ip4/127.0.0.1/tcp/${n.p2p}/p2p/${peer}` };
}
log("starting the three-node devnet");
const producer = await spawnNode(NODES[0], null);
const followers = [await spawnNode(NODES[1], producer.multiaddr), await spawnNode(NODES[2], producer.multiaddr)];
const nodes = [producer, ...followers];
async function converged(minHeight) {
  const hs = await Promise.all(nodes.map((n) => rpc(n.rpc, "get_block_height").catch(() => -1)));
  const h = Math.min(...hs);
  if (h < minHeight) return null;
  const blocks = await Promise.all(nodes.map((n) => rpc(n.rpc, "get_block_by_height", { height: h })));
  return blocks.every((b) => JSON.stringify(b) === JSON.stringify(blocks[0])) ? h : null;
}
let conv = null;
for (let i = 0; i < 120 && conv === null; i++) {
  await sleep(500);
  conv = await converged(3).catch(() => null);
}
if (conv === null) die("devnet did not converge (three nodes, same block at height >= 3)");
env.devnet = { nodes: 3, converged_at_height: conv, profile: "native harness profile (DEVNET_V0.4_OPERATIONS.md)" };
log(`devnet converged at height ${conv}`);

// ── 4. fund the executor, start the one long-lived service ───────────────
const fund = await run(BIN("mvp_service"), ["fund-executor", "--amount", "1000"], { env: CLI_ENV });
fs.writeFileSync(path.join(RUN, "fund.out"), fund.stdout);
fs.writeFileSync(path.join(RUN, "fund.err"), fund.stderr);
if (fund.code !== 0) die("funding the executor failed");
const serviceOut = path.join(RUN, "service.out");
const serviceErr = path.join(RUN, "service.err");
const svcStart = now();
const service = spawn(BIN("mvp_service"), ["serve", "--bind", FRONT], {
  cwd: ROOT,
  env: { ...process.env, ...CLI_ENV, RUST_LOG: "info", MBONGO_INFER_THREADS: String(env.cpu_threads) },
});
children.push(service);
fs.writeFileSync(serviceOut, "");
fs.writeFileSync(serviceErr, "");
service.stdout.pipe(fs.createWriteStream(serviceOut, { flags: "a" }));
service.stderr.pipe(fs.createWriteStream(serviceErr, { flags: "a" }));
let serviceExited = null;
service.on("exit", (code) => (serviceExited = { code, at: new Date().toISOString() }));
for (let i = 0; i < 600 && !fs.readFileSync(serviceOut, "utf8").includes("FRONT_BIND="); i++) {
  if (serviceExited) die("mvp_service exited during start-up");
  await sleep(250);
}
const svcOut = fs.readFileSync(serviceOut, "utf8");
if (!svcOut.includes("FRONT_BIND=")) die("mvp_service did not start");
env.service = Object.fromEntries(svcOut.split(/\r?\n/).filter((l) => l.includes("=")).map((l) => l.split(/=(.*)/s).slice(0, 2)));
env.service.ready_after_ms = now() - svcStart;
env.service_pid = service.pid;
CLI_ENV.MBONGO_MVP_EXECUTOR = env.service.EXECUTOR;
const llamaPid = () => {
  const p = psJson("Get-Process llama-server -ErrorAction SilentlyContinue | Select-Object Id | ConvertTo-Json");
  return p ? [].concat(p).map((x) => x.Id) : [];
};
env.llama_pids_at_start = llamaPid();
log(`service ready: executor ${env.service.EXECUTOR}, cold start ${env.service.WORKER_COLD_START_MS} ms, llama-server ${env.llama_pids_at_start}`);

// ── 5. observation: memory every 30 s ─────────────────────────────────────
const memory = [];
const sampleMemory = () => {
  const p = psJson("Get-Process mvp_service,llama-server -ErrorAction SilentlyContinue | Select-Object Name,Id,WorkingSet64,PrivateMemorySize64 | ConvertTo-Json");
  for (const x of [].concat(p || [])) memory.push({ at: new Date().toISOString(), name: x.Name, pid: x.Id, working_set_mib: Math.round(x.WorkingSet64 / 2 ** 20), private_mib: Math.round(x.PrivateMemorySize64 / 2 ** 20) });
};
sampleMemory();
const memTimer = setInterval(sampleMemory, 30000);
const diskBefore = nodes.map((n) => dirSize(n.dir));
function dirSize(d) {
  let s = 0;
  for (const e of fs.readdirSync(d, { withFileTypes: true })) s += e.isDirectory() ? dirSize(path.join(d, e.name)) : fs.statSync(path.join(d, e.name)).size;
  return s;
}

// ── 6. the campaign ───────────────────────────────────────────────────────
const cli = (args, extra = {}) => run(BIN("mbongo-mvp"), args, { env: CLI_ENV, ...extra });
const jobs = [];
const continuity = [];
let previousTask = null;
let lastLlama = env.llama_pids_at_start;
const STAT_TIMEOUT_MS = 5000;

function classify(code, rec, stderr) {
  if (code === 0 && rec && rec.outcome === "VERIFIED") return null;
  if (code === 3) return "CLIENT";
  if (code === 4) return /chain/i.test(stderr) ? "CHAIN" : "HTTP_FRONT";
  if (code === 5) return /ComputeTask|chain/i.test(stderr) ? "CHAIN" : "HTTP_FRONT";
  if (code === 6) return /timed out/.test(stderr) ? "TIMEOUT" : "RECEIPT";
  if (code === 7) {
    const f = rec?.failure_code || (stderr.match(/after \d+ attempts \(([^)]+)\)/) || [])[1] || "";
    if (/RUNTIME|INFERENCE|llama/i.test(f)) return "LLAMA_SERVER";
    if (/input/i.test(f)) return "DATA_PLANE";
    if (/persistence/i.test(f)) return "DATA_PLANE";
    if (/receipt/i.test(f)) return "RECEIPT";
    if (/OUTPUT|SCHEMA/i.test(f)) return "SCHEMA";
    return "WORKER";
  }
  if (code === 10) return rec?.receipt_integrity?.status !== "VERIFIED" ? "RECEIPT" : "RESULT";
  if (code === 11) return "SCHEMA";
  return "UNKNOWN";
}

for (let i = 0; i < corpus.length; i++) {
  const c = corpus[i];
  // Continuity before the job: service process, front, llama-server.
  const cont = { before_job: c.id, at: new Date().toISOString(), service_alive: serviceExited === null };
  if (previousTask) {
    const t = now();
    const s = await Promise.race([cli(["status", previousTask, "--json"]), sleep(STAT_TIMEOUT_MS).then(() => ({ code: -1 }))]);
    cont.front_responsive = s.code === 0;
    cont.front_ms = now() - t;
  }
  const pids = llamaPid();
  cont.llama_server_pids = pids;
  cont.llama_server_alive = pids.length > 0;
  cont.llama_server_pid_changed = JSON.stringify(pids) !== JSON.stringify(lastLlama);
  lastLlama = pids;
  continuity.push(cont);
  if (!cont.service_alive) die(`mvp_service exited unexpectedly before ${c.id}: environment invalid`);

  const recovery = c.id === RECOVERY_JOB;
  let taskId = null;
  let injected = null;
  let jobDone = false;
  const errPath = path.join(RUN, "cli", `${c.id}.stderr`);
  fs.mkdirSync(path.dirname(errPath), { recursive: true });
  log(`${c.id} (${c.kind}${recovery ? ", RECOVERY CASE" : ""}) submitting`);
  const job = cli(["submit", "-", "--wait", "--json", "--timeout", "900"], {
    stdin: c.cv_text,
    onStderrLine: (l) => {
      const m = l.match(/^task_id ([0-9a-f]{64})/);
      if (m) taskId = m[1];
    },
  });
  if (recovery) {
    // Wait until the worker is mid-attempt, let the inference run, then
    // kill llama-server (the MVP-B in-flight failure). While the worker runs,
    // status is the snapshot taken before the attempt (attempts still 0)
    // with worker_busy = true; that flag is the signal.
    injected = (async () => {
      while (!jobDone) {
        await sleep(500);
        if (!taskId) continue;
        const s = await cli(["status", taskId, "--json"]);
        if (s.code !== 0) continue;
        const v = JSON.parse(s.stdout);
        if (v.worker_busy && v.state !== "completed") {
          await sleep(RECOVERY_DELAY_MS);
          const again = JSON.parse((await cli(["status", taskId, "--json"])).stdout);
          const victim = llamaPid();
          const killedAt = new Date().toISOString();
          ps("Stop-Process -Name llama-server -Force");
          return { fault: "llama-server process killed mid-inference (Stop-Process -Force)", killed_at: killedAt, killed_pids: victim, worker_busy_at_kill: again.worker_busy, attempts_at_kill: again.attempts };
        }
      }
      return { fault: "NOT INJECTED: the attempt was never observed in progress" };
    })();
  }
  const r = await job;
  jobDone = true;
  fs.writeFileSync(errPath, r.stderr);
  fs.writeFileSync(errPath.replace(/.stderr$/, ".stdout"), r.stdout);
  const inj = injected ? await injected : null;
  let rec = null;
  try {
    rec = JSON.parse(r.stdout.trim().split(/\r?\n/).pop());
  } catch {}
  const t = rec?.timings || {};
  const row = {
    job_index: i + 1,
    fixture_id: c.id,
    fixture_kind: c.kind,
    fixture_sha256: c.cv_text_sha256,
    task_id: rec?.task_id || taskId,
    exit_code: r.code,
    cli_wall_ms: r.ms,
    submitted_at: t.submitted_at_ms ?? null,
    included_at: t.task_included_at_ms ?? null,
    registered_at: t.registered_at_ms ?? null,
    inference_started_at: t.inference_started_at_ms ?? null,
    inference_completed_at: t.inference_completed_at_ms ?? null,
    // The result is persisted in the same attempt, immediately after the
    // inference completes; the front does not expose a separate timestamp.
    result_available_at: t.inference_completed_at_ms ?? null,
    completed_observed_at: t.completed_at_ms ?? null,
    verified_at: t.verified_at_ms ?? null,
    total_duration_ms: t.total_ms ?? null,
    inference_duration_ms: t.inference_ms ?? null,
    attempt_count: rec?.attempts ?? null,
    execution_status: rec?.execution_status ?? null,
    receipt_integrity: rec?.receipt_integrity?.status ?? "NOT_RUN",
    result_integrity: rec?.result_integrity?.status ?? "NOT_RUN",
    schema_conformity: rec?.schema_conformity?.status ?? "NOT_RUN",
    business_accuracy: rec?.business_accuracy?.status ?? "NOT_ASSESSED",
    outcome: rec?.outcome ?? null,
    failure_code: rec?.failure_code ?? null,
    task_height: rec?.task_height ?? null,
    receipt_height: rec?.receipt_height ?? null,
    input_tokens: rec?.input_tokens ?? null,
    output_tokens: rec?.output_tokens ?? null,
    result_bytes: rec?.result_bytes ?? null,
    reference_cost_usd: rec?.reference_cost_usd ?? null,
  };
  row.success =
    r.code === 0 &&
    row.outcome === "VERIFIED" &&
    row.receipt_integrity === "VERIFIED" &&
    row.result_integrity === "VERIFIED" &&
    row.schema_conformity === "VERIFIED" &&
    row.business_accuracy === "NOT_ASSESSED";
  row.failure_class = row.success ? null : classify(r.code, rec, r.stderr);
  if (recovery) row.recovery = inj;
  jobs.push(row);
  if (row.task_id) previousTask = row.task_id;
  log(`${c.id} ${row.success ? "SUCCESS" : "FAILED " + row.failure_class} exit=${r.code} attempts=${row.attempt_count} total=${row.total_duration_ms} ms inference=${row.inference_duration_ms} ms`);
  if (serviceExited) {
    log(`mvp_service exited (${JSON.stringify(serviceExited)}): environment invalid; stopping`);
    break;
  }
}
clearInterval(memTimer);
sampleMemory();
env.finished_at = new Date().toISOString();
env.battery_after = process.platform === "win32" ? psJson("Get-CimInstance Win32_Battery | Select-Object BatteryStatus,EstimatedChargeRemaining | ConvertTo-Json") : null;
env.power_state_after = env.battery_after && env.battery_after.BatteryStatus === 2 ? "AC" : "NOT_AC";
env.cpu_after = process.platform === "win32" ? psJson("Get-CimInstance Win32_Processor | Select-Object CurrentClockSpeed,LoadPercentage | ConvertTo-Json") : null;

// ── 7. audit: re-verify, retrieve, receipts on every node ─────────────────
log("audit: re-verifying every task and retrieving results");
const resultsDir = path.join(RUN, "results");
fs.mkdirSync(resultsDir, { recursive: true });
const audit = [];
for (const j of jobs) {
  if (!j.task_id) {
    audit.push({ fixture_id: j.fixture_id, task_id: null });
    continue;
  }
  const v = await cli(["verify", j.task_id, "--json"]);
  let rep = null;
  try {
    rep = JSON.parse(v.stdout);
  } catch {}
  const resFile = path.join(resultsDir, `${j.fixture_id}.bin`);
  const res = await cli(["result", j.task_id, "--out", resFile]);
  const m = res.stderr.match(/recomputed output commitment ([0-9a-f]{64})/);
  audit.push({
    fixture_id: j.fixture_id,
    task_id: j.task_id,
    reverify_exit: v.code,
    reverify_outcome: rep?.outcome ?? null,
    reverify_receipt: rep?.receipt_integrity?.status ?? null,
    reverify_result: rep?.result_integrity?.status ?? null,
    reverify_schema: rep?.schema_conformity?.status ?? null,
    reverify_business: rep?.business_accuracy?.status ?? null,
    result_exit: res.code,
    result_bytes: fs.existsSync(resFile) ? fs.statSync(resFile).size : null,
    recomputed_output_commitment: m ? m[1] : null,
  });
}
const latestHeights = await Promise.all(nodes.map((n) => rpc(n.rpc, "get_block_height")));
const scanTo = Math.min(...latestHeights);
const chainBlocks = {}; // node -> [block json text]
const receipts = {}; // node -> [{height, task_id, executor, output_commitment, input_commitment}]
const txCounts = {};
for (const n of nodes) {
  chainBlocks[n.name] = [];
  receipts[n.name] = [];
  txCounts[n.name] = { blocks: 0, transfer: 0, compute_task: 0, anchor_receipt: 0, other: 0 };
  for (let h = 0; h <= scanTo; h++) {
    const b = await rpc(n.rpc, "get_block_by_height", { height: h });
    if (!b) continue;
    txCounts[n.name].blocks++;
    chainBlocks[n.name].push(JSON.stringify(b));
    for (const tx of b.body?.transactions || []) {
      const p = tx.payload;
      if (p && typeof p === "object" && p.AnchorReceipt) {
        const r = p.AnchorReceipt;
        receipts[n.name].push({ height: h, task_id: hexOf(r.task_id), executor: r.executor, input_commitment: hexOf(r.input_commitment), output_commitment: hexOf(r.output_commitment) });
        txCounts[n.name].anchor_receipt++;
      } else if (p && typeof p === "object" && p.ComputeTask) txCounts[n.name].compute_task++;
      else if (p === "None") txCounts[n.name].transfer++;
      else txCounts[n.name].other++;
    }
  }
}
const sameChain = nodes.every((n) => chainBlocks[n.name].join("\n") === chainBlocks.producer.join("\n"));
const receiptAudit = jobs.map((j) => {
  const a = audit.find((x) => x.fixture_id === j.fixture_id) || {};
  const perNode = Object.fromEntries(nodes.map((n) => [n.name, receipts[n.name].filter((r) => r.task_id === j.task_id)]));
  const rs = perNode.producer;
  return {
    fixture_id: j.fixture_id,
    task_id: j.task_id,
    receipt_found: rs.length > 0,
    receipt_count: rs.length,
    receipt_count_by_node: Object.fromEntries(Object.entries(perNode).map(([k, v]) => [k, v.length])),
    receipt_height: rs[0]?.height ?? null,
    executor: rs[0]?.executor ?? null,
    executor_is_configured: rs[0] ? rs[0].executor === env.service.EXECUTOR : null,
    output_commitment: rs[0]?.output_commitment ?? null,
    retrieved_result_commitment: a.recomputed_output_commitment ?? null,
    output_commitment_match: rs[0] && a.recomputed_output_commitment ? rs[0].output_commitment === a.recomputed_output_commitment : null,
    receipt_valid: a.reverify_receipt === "VERIFIED",
  };
});
const jobTasks = new Set(jobs.map((j) => j.task_id).filter(Boolean));
const foreignReceipts = receipts.producer.filter((r) => !jobTasks.has(r.task_id)).length;

// ── 8. privacy and secret scans ───────────────────────────────────────────
log("privacy scans");
const promptFragments = [fs.readFileSync(PROMPT_PATH, "utf8")];
const cvNeedles = new Set();
for (const c of corpus) for (const l of c.cv_text.split("\n")) if (l.trim().length >= 12) cvNeedles.add(l.trim());
const resultNeedles = new Set();
for (const f of fs.readdirSync(resultsDir)) {
  const t = fs.readFileSync(path.join(resultsDir, f), "utf8");
  if (t.length) resultNeedles.add(t);
  for (const piece of t.split(/","|":"|\{|\}|\[|\]/)) if (piece.trim().length >= 16) resultNeedles.add(piece.trim());
}
const promptNeedles = new Set();
for (const p of promptFragments) for (const l of p.split("\n")) if (l.trim().length >= 16) promptNeedles.add(l.trim());
const secretNeedles = new Set([CLIENT_SEED, EXECUTOR_SEED, Buffer.from(CLIENT_SEED, "hex").toString("base64"), Buffer.from(EXECUTOR_SEED, "hex").toString("base64")]);
const forms = (n) => [n, Buffer.from(n, "utf8").toString("hex"), Array.from(Buffer.from(n, "utf8")).join(","), JSON.stringify(n).slice(1, -1)];
function scan(texts, needles) {
  let hits = 0;
  for (const t of texts) for (const n of needles) for (const f of forms(n)) if (f.length >= 12 && t.includes(f)) hits++;
  return hits;
}
try {
  fs.copyFileSync(llamaLog, path.join(RUN, "llama-server.log"));
} catch {}
const readIf = (p) => (fs.existsSync(p) ? fs.readFileSync(p, "utf8") : "");
const serviceLog = [readIf(serviceOut), readIf(serviceErr)];
const httpLog = [serviceLog[1].split(/\r?\n/).filter((l) => l.includes("mvp-front:")).join("\n")];
const workerLog = [serviceLog[1].split(/\r?\n/).filter((l) => !l.includes("mvp-front:")).join("\n")];
const llamaText = [readIf(path.join(RUN, "llama-server.log"))];
const cliDiag = fs.readdirSync(path.join(RUN, "cli")).map((f) => readIf(path.join(RUN, "cli", f)));
const cliStdout = [jobs.map((j) => JSON.stringify(j)).join("\n")];
const nodeLogs = nodes.map((n) => readIf(path.join(RUN, `${n.name}.log`)));
const fundLog = [readIf(path.join(RUN, "fund.out")), readIf(path.join(RUN, "fund.err"))];
const chainText = Object.values(chainBlocks).map((bs) => bs.join("\n"));
const privacy = {
  needles: { cv_lines: cvNeedles.size, result_fragments: resultNeedles.size, prompt_lines: promptNeedles.size, secrets: secretNeedles.size },
  scanned: {
    chain_blocks: Object.fromEntries(Object.entries(chainBlocks).map(([k, v]) => [k, v.length])),
    log_bytes: [...serviceLog, ...llamaText, ...cliDiag, ...nodeLogs, ...fundLog].reduce((s, t) => s + t.length, 0),
    cli_diagnostic_files: cliDiag.length,
  },
  CHAIN_PRIVATE_HITS: scan(chainText, cvNeedles) + scan(chainText, resultNeedles) + scan(chainText, promptNeedles),
  HTTP_LOG_PRIVATE_HITS: scan(httpLog, cvNeedles) + scan(httpLog, resultNeedles) + scan(httpLog, promptNeedles),
  WORKER_LOG_PRIVATE_HITS: scan(workerLog, cvNeedles) + scan(workerLog, resultNeedles) + scan(workerLog, promptNeedles),
  LLAMA_LOG_PRIVATE_HITS: scan(llamaText, cvNeedles) + scan(llamaText, resultNeedles) + scan(llamaText, promptNeedles),
  CLI_DIAGNOSTIC_PRIVATE_HITS: scan(cliDiag, cvNeedles) + scan(cliDiag, resultNeedles) + scan(cliStdout, cvNeedles) + scan(cliStdout, resultNeedles),
  NODE_AND_FUNDING_LOG_PRIVATE_HITS: scan([...nodeLogs, ...fundLog], cvNeedles) + scan([...nodeLogs, ...fundLog], resultNeedles),
  SECRET_HITS: scan([...chainText, ...serviceLog, ...llamaText, ...cliDiag, ...cliStdout, ...nodeLogs, ...fundLog], secretNeedles),
};

// ── 9. summary ────────────────────────────────────────────────────────────
const ok = jobs.filter((j) => j.success);
const nearestRank = (xs, p) => {
  const s = [...xs].sort((a, b) => a - b);
  return s.length ? s[Math.max(1, Math.ceil((p / 100) * s.length)) - 1] : null;
};
const totals = ok.map((j) => j.total_duration_ms);
const infer = ok.map((j) => j.inference_duration_ms);
const restartsLogged = (serviceLog[1].match(/llama-server not alive; restarting/g) || []).length;
// Every inference attempt the worker logged, in order: its contract outcome
// and duration (no content). The failed attempts and the control plane's
// failure reports are the recovery reason.
const attemptOutcomes = [...serviceLog[1].matchAll(/^\[(\S+) .*cpu-inference: input ([0-9a-f]+) .*duration_ms (\d+) .*outcome ([A-Z_]+)/gm)].map((m) => ({ at: m[1], input: m[2], duration_ms: Number(m[3]), outcome: m[4] }));
const failureReports = [...serviceLog[1].matchAll(/^\[(\S+) .*control-plane: attempt \S+ on task ([0-9a-f]+) reports (Failed\(\w+\))/gm)].map((m) => ({ at: m[1], task: m[2], report: m[3] }));
const rec = jobs.find((j) => j.fixture_id === RECOVERY_JOB);
const recAudit = rec ? receiptAudit.find((r) => r.fixture_id === rec.fixture_id) : null;
const pidChangesOutsideRecovery = continuity.filter((c, idx) => c.llama_server_pid_changed && (idx === 0 || jobs[idx - 1]?.fixture_id !== RECOVERY_JOB)).length;
const memByName = (n) => memory.filter((m) => m.name === n).map((m) => m.working_set_mib);
const summary = {
  baseline_sha: env.git_head,
  power_state: env.power_state,
  power_state_after: env.power_state_after,
  total_jobs: jobs.length,
  successful_jobs: ok.length,
  failed_jobs: jobs.length - ok.length,
  success_rate_pct: jobs.length ? +((100 * ok.length) / jobs.length).toFixed(1) : 0,
  failure_classes: Object.fromEntries(jobs.filter((j) => !j.success).map((j) => [j.fixture_id, j.failure_class])),
  percentile_method: "nearest-rank over successful jobs: the value at 1-based rank ceil(p/100 * n) of the ascending list (n = 20 → P50 = 10th, P90 = 18th, P95 = 19th)",
  latency_total_ms: { p50: nearestRank(totals, 50), p90: nearestRank(totals, 90), p95: nearestRank(totals, 95), max: totals.length ? Math.max(...totals) : null, min: totals.length ? Math.min(...totals) : null, n: totals.length },
  latency_inference_ms: { p50: nearestRank(infer, 50), p90: nearestRank(infer, 90), p95: nearestRank(infer, 95), max: infer.length ? Math.max(...infer) : null, n: infer.length },
  invalid_receipts: receiptAudit.filter((r) => r.receipt_found && !r.receipt_valid).length + jobs.filter((j) => j.receipt_integrity === "FAILED").length,
  result_commitment_mismatches: receiptAudit.filter((r) => r.output_commitment_match === false).length + jobs.filter((j) => j.result_integrity === "FAILED").length,
  // Tasks for which any node holds more than one anchored receipt.
  duplicate_anchored_receipts: receiptAudit.filter((r) => Object.values(r.receipt_count_by_node).some((k) => k > 1)).length,
  receipts_per_task: [...new Set(receiptAudit.map((r) => r.receipt_count))],
  foreign_receipts: foreignReceipts,
  tx_counts: txCounts,
  nodes_identical_chain: sameChain,
  schema_failures_on_successful_jobs: ok.filter((j) => j.schema_conformity !== "VERIFIED").length,
  reverify_all_verified: audit.filter((a) => a.task_id).every((a) => a.reverify_outcome === "VERIFIED"),
  recovery: rec
    ? {
        job: rec.fixture_id,
        attempts: rec.attempt_count,
        fault: rec.recovery?.fault,
        killed_at: rec.recovery?.killed_at,
        worker_busy_at_kill: rec.recovery?.worker_busy_at_kill,
        final_receipt_count: recAudit?.receipt_count,
        final_receipt_count_by_node: recAudit?.receipt_count_by_node,
        recovery_reason: {
          failed_attempts: attemptOutcomes.filter((a) => a.outcome !== "OK"),
          control_plane_reports: failureReports.filter((r) => rec.task_id && rec.task_id.startsWith(r.task)),
          llama_server_restarts_logged: restartsLogged,
  inference_attempts_logged: attemptOutcomes.length,
  inference_attempt_failures: attemptOutcomes.filter((a) => a.outcome !== "OK"),
  control_plane_failure_reports: failureReports,
        },
        result: rec.success ? "SUCCESS" : "FAILED",
        total_duration_ms: rec.total_duration_ms,
      }
    : null,
  privacy,
  service_unexpected_restarts: serviceExited ? 1 : 0,
  worker_unexpected_restarts: serviceExited ? 1 : 0,
  llama_server_restarts_logged: restartsLogged,
  llama_server_unexpected_restarts: Math.max(0, restartsLogged - (rec?.recovery?.killed_at ? 1 : 0)),
  llama_pid_changes_outside_recovery: pidChangesOutsideRecovery,
  front_responsive_between_jobs: continuity.filter((c) => "front_responsive" in c).every((c) => c.front_responsive),
  memory_working_set_mib: { mvp_service: memByName("mvp_service"), llama_server: memByName("llama-server") },
  disk_bytes: { node_data_before: diskBefore, node_data_after: nodes.map((n) => dirSize(n.dir)), service: "none (in-memory)" },
  reference_cost_rate_usd_per_hour: Number(RATE),
  reference_cost_usd_max: ok.length ? Math.max(...ok.map((j) => j.reference_cost_usd || 0)) : null,
  business_extraction_accuracy: "NOT_ASSESSED",
};
const p95ok = summary.latency_total_ms.p95 !== null && summary.latency_total_ms.p95 <= 120000;
const privOk = ["CHAIN_PRIVATE_HITS", "HTTP_LOG_PRIVATE_HITS", "WORKER_LOG_PRIVATE_HITS", "LLAMA_LOG_PRIVATE_HITS", "CLI_DIAGNOSTIC_PRIVATE_HITS", "NODE_AND_FUNDING_LOG_PRIVATE_HITS", "SECRET_HITS"].every((k) => privacy[k] === 0);
const recoveryOk = !!(rec && rec.success && rec.attempt_count > 1 && recAudit?.receipt_count === 1 && rec.recovery?.killed_at);
summary.checks = {
  twenty_jobs: jobs.length === 20,
  at_least_19_successes: ok.length >= 19,
  p95_le_120s: p95ok,
  zero_invalid_receipts: summary.invalid_receipts === 0,
  zero_output_mismatches: summary.result_commitment_mismatches === 0,
  zero_duplicate_receipts: summary.duplicate_anchored_receipts === 0,
  zero_private_leaks: privOk,
  zero_schema_failures: summary.schema_failures_on_successful_jobs === 0,
  recovery_case_passes: recoveryOk,
  ac_power_throughout: env.power_state === "AC" && env.power_state_after === "AC",
  service_continuous: !serviceExited,
};
summary.acceptance = Object.values(summary.checks).every(Boolean) ? "PASS" : "FAIL";

// ── 10. sanitized evidence ────────────────────────────────────────────────
const wr = (n, s) => fs.writeFileSync(path.join(EVIDENCE, n), s);
wr("jobs.jsonl", jobs.map((j) => JSON.stringify(j)).join("\n") + "\n");
const cols = ["job_index", "fixture_id", "fixture_kind", "task_id", "exit_code", "success", "failure_class", "attempt_count", "execution_status", "receipt_integrity", "result_integrity", "schema_conformity", "business_accuracy", "submitted_at", "included_at", "registered_at", "inference_started_at", "inference_completed_at", "result_available_at", "verified_at", "total_duration_ms", "inference_duration_ms", "input_tokens", "output_tokens", "task_height", "receipt_height", "failure_code", "reference_cost_usd"];
wr("jobs.csv", cols.join(",") + "\n" + jobs.map((j) => cols.map((k) => (j[k] === null || j[k] === undefined ? "" : String(j[k]))).join(",")).join("\n") + "\n");
wr("receipts.json", JSON.stringify({ scanned_to_height: scanTo, nodes: nodes.map((n) => n.name), tasks: receiptAudit, reverify: audit.map(({ recomputed_output_commitment, ...a }) => a) }, null, 2) + "\n");
wr("environment.json", JSON.stringify({ ...env, verification: verify, continuity, corpus: corpus.map(({ cv_text, ...m }) => m) }, null, 2) + "\n");
wr("memory.csv", "at,name,pid,working_set_mib,private_mib\n" + memory.map((m) => [m.at, m.name, m.pid, m.working_set_mib, m.private_mib].join(",")).join("\n") + "\n");
wr("summary.json", JSON.stringify(summary, null, 2) + "\n");
log(`MVP-D ${summary.acceptance}: ${ok.length}/${jobs.length} success, P95 total ${summary.latency_total_ms.p95} ms, receipts ${summary.receipts_per_task}, privacy ${privOk ? "clean" : "HITS"}`);
log(`evidence: ${EVIDENCE}  private run dir: ${RUN}`);
cleanup();
process.exit(summary.acceptance === "PASS" ? 0 : 1);
