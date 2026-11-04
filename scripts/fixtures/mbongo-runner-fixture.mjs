#!/usr/bin/env node
// A conformance fixture for mbongo-runner:v1, written independently of the
// Rust core: it speaks only the process contract. Not a workload.
// `params.mode` asks for misbehaviour, so that tests can reach each failure.
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const CONTRACT = "mbongo-runner:v1";
const dir = process.env.MBONGO_JOB_DIR;
const job = JSON.parse(fs.readFileSync(path.join(dir, "job.json"), "utf8"));
const params = job.params;

function readRequest() {
  const all = fs.readFileSync(0);
  const end = all.indexOf(10);
  const [contract, digest, length] = all.subarray(0, end).toString("ascii").split(" ");
  const input = all.subarray(end + 1);
  if (contract !== CONTRACT || digest.length !== 64 || Number(length) !== input.length) process.exit(70);
  return input;
}

const write = (bytes) => fs.writeSync(1, bytes);
function respond(output) {
  write(Buffer.from(`${CONTRACT} ok ${output.length}\n`, "ascii"));
  write(output);
}

function compute(input) {
  switch (job.workload) {
    case "fixture-byte-table": {
      const table = fs.readFileSync(path.join(dir, "artifacts", job.artifacts[0].sha256));
      return Buffer.from([...input].map((b) => table[b]));
    }
    case "fixture-sum": {
      const list = JSON.parse(input.toString("utf8"));
      return Buffer.from(`{"count":${list.length},"sum":${list.reduce((a, b) => a + b, 0)}}`);
    }
    default:
      process.exit(71);
  }
}

const pause = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
const attempt = (action) => {
  try {
    action();
    return "OK";
  } catch (e) {
    return e.code ?? "ERROR";
  }
};
const session = (pid) => fs.readFileSync(`/proc/${pid}/stat`, "utf8").split(") ")[1].split(" ")[3];

function sorted(value) {
  if (Array.isArray(value)) return value.map(sorted);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(Object.keys(value).sort().map((k) => [k, sorted(value[k])]));
}

// What a compromised runner would try. Each path is tried, and the outcome
// reported as an errno name: never what was read.
function attack({ targets, service_pid: pid, cgroup_root: root }) {
  const own = fs.readFileSync("/proc/self/cgroup", "utf8").trim().replace(/^0::/, "");
  const status = fs.readFileSync("/proc/self/status", "utf8");
  const line = (name) => status.split("\n").find((l) => l.startsWith(`${name}:`)).slice(name.length + 1).trim().replace(/\s+/g, " ");
  return {
    read: targets.map((t) => attempt(() => fs.readFileSync(t))),
    write: targets.map((t) => attempt(() => fs.closeSync(fs.openSync(t, "a")))),
    unlink: targets.map((t) => attempt(() => fs.unlinkSync(t))),
    service: {
      environ: attempt(() => fs.readFileSync(`/proc/${pid}/environ`)),
      mem: attempt(() => fs.closeSync(fs.openSync(`/proc/${pid}/mem`, "r"))),
      signal: attempt(() => process.kill(pid, 0)),
    },
    cgroup: {
      into_parent: attempt(() => fs.writeFileSync(`${root}/cgroup.procs`, String(process.pid))),
      into_service: attempt(() => fs.writeFileSync(`${root}/service/cgroup.procs`, String(process.pid))),
      raise_memory: attempt(() => fs.writeFileSync(`/sys/fs/cgroup${own}/memory.max`, "max")),
      new_child: attempt(() => fs.mkdirSync(`/sys/fs/cgroup${own}/escape`)),
    },
    setuid_root: attempt(() => process.setuid(0)),
    job_spec: attempt(() => fs.readFileSync(path.join(dir, "job.json"))),
    job_dir_write: attempt(() => fs.writeFileSync(path.join(dir, "left-behind"), "x")),
    identity: {
      uid: line("Uid"),
      gid: line("Gid"),
      groups: line("Groups"),
      cap_eff: line("CapEff"),
      cap_prm: line("CapPrm"),
      cap_inh: line("CapInh"),
      cap_amb: line("CapAmb"),
      no_new_privs: line("NoNewPrivs"),
      session_leader: session(process.pid) === String(process.pid) ? "YES" : "NO",
      cgroup: own,
    },
    descriptors: fs.readdirSync("/proc/self/fd").flatMap((fd) => {
      try {
        return [fs.readlinkSync(`/proc/self/fd/${fd}`)];
      } catch {
        return [];
      }
    }),
  };
}

// A descendant in its own session, which outlives its parent unless something
// kills it. Returns once it has proven it is running.
function daemon(heartbeat) {
  const script = `while :; do echo $$ >> ${heartbeat}; sleep 0.05; done`;
  const child = spawn("/bin/sh", ["-c", script], { detached: true, stdio: "ignore" });
  child.unref();
  while (!fs.existsSync(heartbeat) || fs.readFileSync(heartbeat, "utf8").split("\n").length < 4) pause(10);
  return child.pid;
}

const input = readRequest();
switch (params.mode) {
  case undefined:
    respond(compute(input));
    break;
  case "exit-1":
    respond(compute(input));
    process.exit(1);
    break;
  case "no-frame":
    write(compute(input));
    break;
  case "wrong-version":
    write(Buffer.from("mbongo-runner:v2 ok 0\n"));
    break;
  case "short":
    write(Buffer.from(`${CONTRACT} ok 10\nabc`));
    break;
  case "extra":
    write(Buffer.from(`${CONTRACT} ok 1\nabc`));
    break;
  case "not-canonical":
    respond(Buffer.from("{ }"));
    break;
  case "declare-huge":
    write(Buffer.from(`${CONTRACT} ok 999999999\n`));
    break;
  case "flood":
    write(Buffer.from(`${CONTRACT} ok 1\n`));
    for (;;) write(Buffer.alloc(65536, 0x41));
  case "sleep":
    for (;;) {
      fs.appendFileSync(params.heartbeat, "x");
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 20);
    }
  case "slow":
    fs.appendFileSync(params.marker, "started\n");
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, params.delay_ms);
    respond(compute(input));
    break;
  case "attack":
    respond(Buffer.from(JSON.stringify(sorted(attack(params)))));
    break;
  case "daemon": {
    const pid = daemon(params.heartbeat);
    if (params.hang) for (;;) pause(1000);
    respond(Buffer.from(JSON.stringify({ descendant_session: session(pid), own_session: session(process.pid), pid })));
    break;
  }
  case "alloc": {
    const block = Buffer.alloc(params.mib * 1048576, 1);
    respond(compute(input));
    process.exitCode = block[0] === 1 ? 0 : 1;
    break;
  }
  case "report":
    respond(Buffer.from(JSON.stringify({
      env: Object.keys(process.env).sort(),
      files: fs.readdirSync(dir).sort(),
      cwd_is_job_dir: fs.realpathSync(process.cwd()) === fs.realpathSync(dir),
    })));
    break;
  case "stderr":
    fs.writeSync(2, Buffer.concat([Buffer.from("runner log: "), input]));
    process.exit(1);
    break;
  default:
    process.exit(72);
}
