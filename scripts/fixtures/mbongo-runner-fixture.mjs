#!/usr/bin/env node
// A conformance fixture for mbongo-runner:v1, written independently of the
// Rust core: it speaks only the process contract. Not a workload.
// `params.mode` asks for misbehaviour, so that tests can reach each failure.
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
