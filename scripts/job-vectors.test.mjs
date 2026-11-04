// mbongo-job:v1 vectors, recomputed independently of the Rust code.
//   npm ci --prefix sdk/typescript && node --test scripts/job-vectors.test.mjs
//   node scripts/job-vectors.test.mjs --write   (regenerates the vector file)
import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

const ROOT = path.join(import.meta.dirname, "..");
const FILE = path.join(ROOT, "test-vectors/job/job-contract-v1.json");
const { blake3 } = await import(pathToFileURL(path.join(ROOT, "sdk/typescript/node_modules/@noble/hashes/blake3.js")));

const utf8 = (s) => Buffer.from(s, "utf8");
const hex = (b) => Buffer.from(b).toString("hex");
const domainHash = (domain, bytes) => hex(blake3(Buffer.concat([utf8(domain), bytes])));
const sha256 = (b) => crypto.createHash("sha256").update(b).digest("hex");

// The canonical JSON of the contract: sorted keys, no whitespace, non-negative integers.
function canonical(v) {
  if (typeof v === "string") return JSON.stringify(v);
  if (Number.isSafeInteger(v) && v >= 0) return String(v);
  if (Array.isArray(v)) return `[${v.map(canonical).join(",")}]`;
  if (v && typeof v === "object") {
    const keys = Object.keys(v).sort((a, b) => Buffer.compare(utf8(a), utf8(b)));
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonical(v[k])}`).join(",")}}`;
  }
  throw new Error(`outside the canonical model: ${v}`);
}

const TABLE = Buffer.from(Array.from({ length: 256 }, (_, i) => (i * 7 + 3) % 256));
const base = (workload, type, artifacts) => ({
  workload,
  runtime: { family: "test-fixture", revision: "1", accelerator_class: "cpu" },
  artifacts,
  input: { type, max_bytes: 64 },
  output: { type, max_bytes: 64 },
  limits: { max_wall_ms: 5000, max_memory_mib: 16 },
  determinism: "BIT_EXACT",
  confidentiality: ["PUBLIC"],
  params: {},
});
const SPECS = {
  "fixture-byte-table": base("fixture-byte-table", "bytes", [{ role: "table", sha256: sha256(TABLE), size_bytes: TABLE.length }]),
  "fixture-sum": base("fixture-sum", "canonical-json", []),
  "fixture-future-runtime": {
    ...base("fixture-future-runtime", "canonical-json", []),
    runtime: { family: "test-future", revision: "1", accelerator_class: "gpu:test" },
    determinism: "TOLERANCE",
  },
};

const byteTable = (input) => Buffer.from([...input].map((b) => TABLE[b]));
const sum = (input) => {
  const list = JSON.parse(input.toString("utf8"));
  return utf8(canonical({ count: list.length, sum: list.reduce((a, b) => a + b, 0) }));
};
const CASES = [
  ["fixture-byte-table", Buffer.from([]), byteTable],
  ["fixture-byte-table", Buffer.from([0, 1, 2, 255]), byteTable],
  ["fixture-byte-table", utf8("mbongo"), byteTable],
  ["fixture-sum", utf8("[]"), sum],
  ["fixture-sum", utf8("[7]"), sum],
  ["fixture-sum", utf8("[1,2,3,4,5]"), sum],
];

function build() {
  const specs = Object.entries(SPECS).map(([name, spec]) => {
    const text = canonical(spec);
    const digest = domainHash("mbongo:job-spec:v1", utf8(text));
    return { name, spec, canonical: text, digest, execution_spec: hex(utf8("mbongo-job:v1")) + digest };
  });
  const cases = CASES.map(([workload, input, run]) => {
    const output = run(input);
    return {
      workload,
      input: hex(input),
      output: hex(output),
      input_commitment: domainHash("mbongo:compute-input:v1", input),
      output_commitment: domainHash("mbongo:compute-output:v1", output),
    };
  });
  return {
    $comment: "mbongo-job:v1 conformance vectors, computed by scripts/job-vectors.test.mjs, independently of the Rust code. The fixtures test the infrastructure and are not workloads. Synthetic only.",
    version: "job-contract-vectors:v1",
    table: hex(TABLE),
    specs,
    cases,
  };
}

if (process.argv.includes("--write")) {
  fs.mkdirSync(path.dirname(FILE), { recursive: true });
  fs.writeFileSync(FILE, `${JSON.stringify(build(), null, 1)}\n`);
} else {
  test("the committed vectors are what this independent implementation computes", () => {
    const committed = JSON.parse(fs.readFileSync(FILE, "utf8"));
    assert.equal(committed.specs.length, 3);
    assert.equal(committed.cases.length, 6);
    assert.deepEqual(committed, build());
  });

  test("the execution_spec is 45 bytes and the two job-spec domains differ", () => {
    for (const s of build().specs) assert.equal(s.execution_spec.length / 2, 45);
    const bytes = utf8("{}");
    assert.notEqual(domainHash("mbongo:job-spec:v1", bytes), domainHash("mbongo:infer-job-spec:v1", bytes));
  });
}
