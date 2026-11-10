// Fixed harness ports must stay outside the Linux ephemeral range: a listener
// cannot bind a port held by a recent outbound connection, or one in TIME_WAIT.
//   node scripts/check-harness-ports.mjs
import fs from "node:fs";
import path from "node:path";

const ROOT = path.join(import.meta.dirname, "..");
const CONST = /const ([A-Z0-9_]*(?:RPC|REST|P2P)(?:_PORT)?): u16 = (\d+);/g;
const FIELD = /\b(rpc|rest|p2p): (\d+),/g;

// Every harness that CI runs, plus the ignored convergence test, with the
// number of fixed ports each must yield.
const HARNESSES = [
  { file: "crates/mbongo-node/src/bin/replay_harness.rs", pattern: CONST, count: 3 },
  { file: "crates/mbongo-compute/src/bin/compute_harness.rs", pattern: CONST, count: 3 },
  { file: "crates/mbongo-compute/src/bin/provider_harness.rs", pattern: CONST, count: 3 },
  { file: "crates/mbongo-node/src/bin/compute_vertical.rs", pattern: FIELD, count: 9 },
  { file: "crates/mbongo-node/src/bin/devnet_harness.rs", pattern: CONST, count: 9 },
  { file: "crates/mbongo-node/tests/devnet_convergence.rs", pattern: CONST, count: 9 },
  { file: "crates/mbongo-network/tests/reconnect.rs", pattern: CONST, count: 4 },
  { file: "crates/mbongo-network/tests/a1_limits.rs", pattern: CONST, count: 2 },
  { file: "crates/mbongo-network/tests/a2_3a_rate.rs", pattern: CONST, count: 1 },
  { file: "crates/mbongo-network/tests/a2_3b_rate.rs", pattern: CONST, count: 1 },
  { file: "crates/mbongo-node/tests/a1_flood_convergence.rs", pattern: CONST, count: 6 },
];

const ports = [];
for (const { file, pattern, count } of HARNESSES) {
  const text = fs.readFileSync(path.join(ROOT, file), "utf8");
  const found = [...text.matchAll(pattern)].map((m) => ({ file, name: m[1], port: Number(m[2]) }));
  if (found.length !== count) throw new Error(`${file}: expected ${count} ports, found ${found.length}`);
  ports.push(...found);
}
const expected = HARNESSES.reduce((n, h) => n + h.count, 0);
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
