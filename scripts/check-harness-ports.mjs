// Fixed harness ports must stay outside the Linux ephemeral range: a listener
// cannot bind a port held by a recent outbound connection in TIME_WAIT.
//   node scripts/check-harness-ports.mjs
import fs from "node:fs";
import path from "node:path";

const ROOT = path.join(import.meta.dirname, "..");
const HARNESSES = [
  "crates/mbongo-node/src/bin/replay_harness.rs",
  "crates/mbongo-compute/src/bin/compute_harness.rs",
  "crates/mbongo-compute/src/bin/provider_harness.rs",
];
const NAMES = ["RPC_PORT", "REST_PORT", "P2P_PORT"];

const ports = [];
for (const file of HARNESSES) {
  const text = fs.readFileSync(path.join(ROOT, file), "utf8");
  for (const name of NAMES) {
    const m = text.match(new RegExp(`const ${name}: u16 = (\\d+);`));
    if (!m) throw new Error(`${file}: no ${name}`);
    ports.push({ file, name, port: Number(m[1]) });
  }
}
const expected = HARNESSES.length * NAMES.length;
if (ports.length !== expected) throw new Error(`expected ${expected} ports, found ${ports.length}`);
if (new Set(ports.map((p) => p.port)).size !== expected) throw new Error("two harnesses share a port");

const rangeFile = "/proc/sys/net/ipv4/ip_local_port_range";
if (!fs.existsSync(rangeFile)) {
  console.log(`${expected} harness ports distinct; no ${rangeFile} here, range not checked`);
  process.exit(0);
}
const [low, high] = fs.readFileSync(rangeFile, "utf8").trim().split(/\s+/).map(Number);
console.log(`ephemeral range: ${low}-${high}`);
const inside = ports.filter((p) => p.port >= low && p.port <= high);
for (const p of inside) console.error(`inside the range: ${p.file} ${p.name} = ${p.port}`);
if (inside.length) process.exit(1);
console.log(`${expected} harness ports distinct and outside the range`);
