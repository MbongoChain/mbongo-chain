// TypeScript side of the shared cross-language ComputeTask vectors.
//
// Every expected value is read from test-vectors/compute-task/compute-task-v1
// .json and test-vectors/rpc/compute-task-rpc-v1.json. Nothing is copied into
// this file: a copied constant would only prove the copy was faithful. Rust
// reads the same files, so agreement here is real interoperability.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  COMPUTE_TASK_DOMAIN,
  COMPUTE_TASK_PAYLOAD_PREFIX_BYTES,
  COMPUTE_TASK_VERSION,
  MAX_EXECUTION_SPEC_BYTES,
  MbongoClient,
  MbongoComputeTaskError,
  MbongoTransportError,
  computeTaskId,
  computeTaskIdPreimage,
  computeTaskSigningPayload,
  computeTaskToWire,
  computeTaskTransactionHash,
  computeTaskTransactionToWire,
  computeTasksInBlock,
  encodeComputeTask,
  receiptsInBlock,
  signComputeTaskTransaction,
  submitComputeTask,
  wireComputeTaskToComputeTask,
} from "../dist/index.js";
import { stringifyExact } from "../dist/json-exact.js";

const load = (rel) => JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));

const CT = load("../../../test-vectors/compute-task/compute-task-v1.json");
const RPC = load("../../../test-vectors/rpc/compute-task-rpc-v1.json");
const RX = load("../../../test-vectors/receipt/receipt-v1.json");
for (const [name, doc] of [["compute-task", CT], ["rpc", RPC], ["receipt", RX]]) {
  assert.equal(doc.fixture_version, 1, `${name} fixture: unsupported schema version`);
}

const unhex = (s) => {
  assert.ok(!s.startsWith("0x"), "fixture hex must not carry an 0x prefix");
  assert.ok(/^[0-9a-f]*$/.test(s), "fixture hex must be lowercase");
  return Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));
};
const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, "0")).join("");

/** Expands the execution_spec patterns the fixture defines. */
function spec(s) {
  if (s.pattern === "repeat") return new Uint8Array(s.length).fill(unhex(s.byte)[0]);
  if (s.pattern === "literal") return unhex(s.hex);
  assert.fail(`unsupported execution_spec pattern ${s.pattern}`);
}

/** Resolves a task vector by name. Exactly one match is required. */
function taskVector(name) {
  const matches = CT.tasks.filter((v) => v.name === name);
  assert.equal(matches.length, 1, `expected exactly one task vector named ${name}`);
  return matches[0];
}

function taskFrom(entry) {
  const t = entry.task;
  return {
    version: t.version,
    submitter: unhex(t.submitter),
    executor: unhex(t.executor),
    salt: unhex(t.salt),
    inputCommitment: unhex(t.input_commitment),
    executionSpec: spec(t.execution_spec),
  };
}

const SUBMITTER_SEED = unhex(RX.test_key.ed25519_seed);
const CANONICAL = taskFrom(taskVector("canonical"));

// ── constants ───────────────────────────────────────────────────────────

test("the protocol constants match the fixture", () => {
  assert.equal(COMPUTE_TASK_VERSION, CT.envelope.version);
  assert.equal(MAX_EXECUTION_SPEC_BYTES, CT.execution_spec_max_bytes);
  assert.equal(COMPUTE_TASK_DOMAIN, CT.domain_task.ascii);
  assert.equal(COMPUTE_TASK_DOMAIN.length, CT.domain_task.length);
  assert.equal(CT.signing_formula.fixed_bytes_before_task, COMPUTE_TASK_PAYLOAD_PREFIX_BYTES);
  assert.equal(CT.envelope.task_id_is_not_a_field, true);
});

// ── canonical bytes and task_id ─────────────────────────────────────────

test("every task vector reproduces the pinned canonical bytes and task_id", () => {
  assert.equal(CT.tasks.length, 4, "task vector cardinality");
  for (const v of CT.tasks) {
    const task = taskFrom(v);
    const bytes = encodeComputeTask(task);
    assert.equal(hex(bytes), v.expected.canonical_task, `${v.name}: canonical bytes`);
    assert.equal(bytes.length, v.expected.canonical_task_length, `${v.name}: length`);
    // The compact prefix sits right after the four 32-byte fields.
    const prefix = unhex(v.expected.execution_spec_compact_prefix);
    assert.equal(hex(bytes.subarray(129, 129 + prefix.length)), hex(prefix), `${v.name}: prefix`);
    const preimage = computeTaskIdPreimage(task);
    assert.equal(preimage.length, v.expected.task_id_preimage_length, `${v.name}: preimage length`);
    assert.equal(hex(preimage.subarray(0, 22)), CT.domain_task.hex, `${v.name}: raw domain tag`);
    assert.equal(hex(preimage.subarray(22)), v.expected.canonical_task, `${v.name}: tag then bytes`);
    assert.equal(hex(computeTaskId(task)), v.expected.task_id, `${v.name}: task_id`);
  }
});

test("the domain tag is prepended raw: no NUL, no length prefix, no hex text", () => {
  const preimage = computeTaskIdPreimage(CANONICAL);
  // Byte 22 is the version byte of the task, not a NUL and not a prefix.
  assert.equal(preimage[22], COMPUTE_TASK_VERSION);
  assert.ok(!preimage.subarray(0, 22).includes(0), "no NUL inside the tag");
  // And the diagnostics the fixture pins for each wrong tagging are all
  // different from what this package produces.
  const id = hex(computeTaskId(CANONICAL));
  for (const key of ["tag_scale_encoded", "tag_nul_terminated", "no_tag", "hex_rendering_hashed"]) {
    assert.notEqual(id, CT.wrong_tag_diagnostics[key], key);
  }
});

test("task_id commits to every field, and not to the nonce", () => {
  const base = taskVector(CT.identity.base_vector);
  assert.equal(hex(computeTaskId(taskFrom(base))), base.expected.task_id);
  assert.equal(CT.identity.variants.length, 5);
  const seen = new Set();
  for (const v of CT.identity.variants) {
    const id = hex(computeTaskId(taskFrom(v)));
    assert.equal(id, v.expected.task_id, `variant ${v.changed_field}`);
    assert.notEqual(id, base.expected.task_id, `variant ${v.changed_field} must change task_id`);
    assert.ok(!seen.has(id), `variant ${v.changed_field} collides`);
    seen.add(id);
  }
  // The two canonical transaction vectors carry the same task under
  // different nonces: same task_id, different transaction hash.
  const [a, b] = ["canonical", "canonical-nonce-zero"].map((n) => CT.valid.find((v) => v.name === n));
  assert.equal(a.task_vector, b.task_vector);
  assert.notEqual(a.transaction.nonce, b.transaction.nonce);
  assert.notEqual(a.expected.transaction_hash, b.expected.transaction_hash);
  assert.equal(CT.identity.nonce_changes_task_id, false);
});

// ── SCALE compact-length boundaries ─────────────────────────────────────

/** SCALE compact, written from the rule rather than the implementation. */
function compactFromRule(n) {
  if (n < 64) return [n << 2];
  const v = (n << 2) | 1;
  return [v & 0xff, v >> 8];
}

test("the execution_spec length prefix follows SCALE across every boundary", () => {
  for (const n of [0, 1, 63, 64, 255, 256, 1023, 1024]) {
    const task = { ...CANONICAL, executionSpec: new Uint8Array(n).fill(0x5a) };
    const bytes = encodeComputeTask(task);
    const expected = compactFromRule(n);
    assert.deepEqual(Array.from(bytes.subarray(129, 129 + expected.length)), expected, `n=${n}`);
    assert.equal(bytes.length, 129 + expected.length + n, `n=${n}: total length`);
  }
});

test("execution_spec: 0 and 1024 bytes are accepted, 1025 is rejected before anything is produced", () => {
  const empty = taskFrom(taskVector("empty-spec"));
  assert.equal(empty.executionSpec.length, 0);
  assert.equal(encodeComputeTask(empty).length, 130);
  const maximal = taskFrom(taskVector("spec-max-1024"));
  assert.equal(maximal.executionSpec.length, MAX_EXECUTION_SPEC_BYTES);
  assert.equal(encodeComputeTask(maximal).length, CT.maximal_sizes.canonical_task);
  assert.equal(computeTaskIdPreimage(maximal).length, CT.maximal_sizes.task_id_preimage);

  const over = taskFrom(CT.rejected.find((v) => v.name === "spec-1025"));
  assert.equal(over.executionSpec.length, MAX_EXECUTION_SPEC_BYTES + 1);
  for (const f of [encodeComputeTask, computeTaskId, computeTaskToWire]) {
    assert.throws(() => f(over), (e) => e instanceof MbongoComputeTaskError && e.field === "executionSpec", f.name);
  }
  assert.throws(() => signComputeTaskTransaction(over, 0, SUBMITTER_SEED), MbongoComputeTaskError);
});

// ── byte safety ─────────────────────────────────────────────────────────

test("every byte value survives the canonical encoding and the wire form", () => {
  const all = new Uint8Array(256);
  for (let i = 0; i < 256; i++) all[i] = i;
  // 0x00, 0xff, embedded zeros and every value in between, in one spec.
  const spec = new Uint8Array(1024);
  spec.set(all, 0);
  spec.set(all, 256);
  spec.set(all, 512);
  spec.set(all, 768);
  const task = { ...CANONICAL, salt: all.subarray(0, 32), inputCommitment: all.subarray(224, 256), executionSpec: spec };
  const bytes = encodeComputeTask(task);
  assert.equal(hex(bytes.subarray(131)), hex(spec), "spec bytes verbatim after the two-byte prefix");
  const wire = computeTaskToWire(task);
  assert.deepEqual(wire.execution_spec, Array.from(spec));
  assert.deepEqual(wire.salt, Array.from(task.salt));
  const back = wireComputeTaskToComputeTask(wire);
  assert.equal(hex(back.executionSpec), hex(spec));
  assert.equal(hex(back.salt), hex(task.salt));
  assert.equal(hex(back.inputCommitment), hex(task.inputCommitment));
  assert.equal(hex(computeTaskId(back)), hex(computeTaskId(task)), "identity survives the round trip");
  // Never mutated.
  assert.equal(spec[0], 0);
  assert.equal(spec[255], 255);
});

test("malformed tasks fail closed", () => {
  const cases = [
    ["version", { ...CANONICAL, version: 2 }],
    ["version", { ...CANONICAL, version: 1.5 }],
    ["submitter", { ...CANONICAL, submitter: new Uint8Array(31) }],
    ["executor", { ...CANONICAL, executor: "0x" + "22".repeat(32) }],
    ["salt", { ...CANONICAL, salt: new Uint8Array(33) }],
    ["inputCommitment", { ...CANONICAL, inputCommitment: new Uint8Array(0) }],
    ["executionSpec", { ...CANONICAL, executionSpec: [1, 2, 3] }],
  ];
  for (const [field, task] of cases) {
    assert.throws(
      () => encodeComputeTask(task),
      (e) => e instanceof MbongoComputeTaskError && e.field === field,
      field,
    );
  }
});

// ── the transaction ─────────────────────────────────────────────────────

test("every valid vector reproduces the signing payload, signature, encoding and hash", () => {
  assert.equal(CT.valid.length, 3, "transaction vector cardinality");
  for (const v of CT.valid) {
    const task = taskFrom(taskVector(v.task_vector));
    const payload = computeTaskSigningPayload(task, v.transaction.nonce);
    assert.equal(hex(payload), v.expected.signing_payload, `${v.name}: signing payload`);
    assert.equal(payload.length, v.expected.signing_payload_length, `${v.name}: length`);
    assert.equal(payload[0], 0x01, `${v.name}: TransactionType::ComputeTask`);
    assert.equal(payload[89], 0x02, `${v.name}: TransactionPayload::ComputeTask`);
    assert.equal(v.expected.task_offset, COMPUTE_TASK_PAYLOAD_PREFIX_BYTES);
    assert.equal(hex(payload.subarray(90)), hex(encodeComputeTask(task)), `${v.name}: task suffix`);
    assert.equal(hex(payload.subarray(81, 89)), v.expected.nonce_u64_le, `${v.name}: nonce LE`);

    const tx = signComputeTaskTransaction(task, v.transaction.nonce, SUBMITTER_SEED);
    assert.equal(hex(tx.signature), v.expected.transaction_signature, `${v.name}: signature`);
    assert.equal(hex(tx.sender), hex(task.submitter), `${v.name}: sender is the submitter`);
    assert.equal(tx.amount, 0n);
    assert.equal(hex(tx.receiver), "00".repeat(32));
    assert.equal(hex(computeTaskTransactionHash(tx)), v.expected.transaction_hash, `${v.name}: hash`);
  }
});

test("a key that does not derive the submitter cannot sign the task", () => {
  const other = new Uint8Array(32).fill(0x2b);
  assert.throws(
    () => signComputeTaskTransaction(CANONICAL, 0, other),
    (e) => e instanceof MbongoComputeTaskError && e.field === "secretKey",
  );
  assert.throws(() => signComputeTaskTransaction(CANONICAL, 0, new Uint8Array(31)), MbongoComputeTaskError);
});

test("the wire object is exactly what the RPC fixture pins, and it round-trips", () => {
  assert.equal(RPC.transactions.length, 3);
  for (const entry of RPC.transactions) {
    const task = taskFrom(taskVector(entry.task_vector));
    const tx = signComputeTaskTransaction(task, entry.object.nonce, SUBMITTER_SEED);
    const wire = computeTaskTransactionToWire(tx);
    // Compared through the exact serialiser: nonce is a bigint here and a
    // JSON integer on the wire.
    assert.equal(stringifyExact(wire), JSON.stringify(entry.object), `${entry.name}: wire object`);
    assert.equal(hex(computeTaskId(task)), entry.expected.task_id, `${entry.name}: task_id`);
    assert.equal(hex(computeTaskTransactionHash(tx)), entry.expected.transaction_hash, `${entry.name}: hash`);
    assert.ok(!("task_id" in wire.payload.ComputeTask), `${entry.name}: task_id is not a wire field`);
    const back = wireComputeTaskToComputeTask(entry.object.payload.ComputeTask);
    assert.equal(hex(encodeComputeTask(back)), hex(encodeComputeTask(task)), `${entry.name}: decode`);
  }
});

test("submitComputeTask sends the pinned object through submit_transaction", async () => {
  const entry = RPC.transactions.find((t) => t.name === "maximal");
  const task = taskFrom(taskVector(entry.task_vector));
  const tx = signComputeTaskTransaction(task, entry.object.nonce, SUBMITTER_SEED);
  const sent = [];
  const fetchImpl = async (_url, init) => {
    sent.push(init.body);
    return { status: 200, text: async () => JSON.stringify({ jsonrpc: "2.0", id: 1, result: "0xabc" }) };
  };
  const client = new MbongoClient("http://localhost:8080/rpc", { fetch: fetchImpl });
  assert.equal(await submitComputeTask(client, tx), "0xabc");
  const request = JSON.parse(sent[0]);
  assert.equal(request.method, "submit_transaction");
  assert.deepEqual(request.params, entry.object);
  assert.equal(request.params.payload.ComputeTask.execution_spec.length, 1024);
});

test("a node rejection becomes a typed reason", async () => {
  const reject = (message) => async () => ({
    status: 500,
    text: async () => JSON.stringify({ jsonrpc: "2.0", id: 1, error: { code: -32603, message } }),
  });
  const tx = signComputeTaskTransaction(CANONICAL, 0, SUBMITTER_SEED);
  for (const [message, reason] of [
    ["internal backend error: task_id already registered", "duplicate-task"],
    ["internal backend error: compute task already pending", "task-pending"],
    ["internal backend error: sender must equal task submitter", "sender-submitter-mismatch"],
    ["internal backend error: compute task execution_spec too large", "execution-spec-too-large"],
    ["internal backend error: invalid nonce: expected 3", "invalid-nonce"],
    ["internal backend error: something new", "unknown"],
  ]) {
    const client = new MbongoClient("http://localhost:8080/rpc", { fetch: reject(message) });
    await assert.rejects(
      submitComputeTask(client, tx),
      (e) => e instanceof MbongoComputeTaskError && e.reason === reason && e.field === "submit",
      message,
    );
  }
});

// ── reading blocks ──────────────────────────────────────────────────────

const BLOCK = RPC.block.object;

test("getBlockByHeight decodes a block carrying Transfer, ComputeTask and AnchorReceipt", async () => {
  const fetchImpl = async () => ({
    status: 200,
    text: async () => JSON.stringify({ jsonrpc: "2.0", id: 1, result: BLOCK }),
  });
  const client = new MbongoClient("http://localhost:8080/rpc", { fetch: fetchImpl });
  const block = await client.getBlockByHeight(1);
  assert.equal(block.body.transactions.length, 3);
  assert.equal(block.body.transactions[0].payload, "None");
  const task = block.body.transactions[1].payload.ComputeTask;
  assert.ok(task, "the ComputeTask payload is returned");
  assert.deepEqual(task, BLOCK.body.transactions[1].payload.ComputeTask, "byte-exact wire object");
  assert.ok("AnchorReceipt" in block.body.transactions[2].payload);
  // And the task it carries is the pinned minimal task.
  const minimal = RPC.transactions.find((t) => t.name === "minimal");
  assert.equal(hex(computeTaskId(wireComputeTaskToComputeTask(task))), minimal.expected.task_id);
});

test("computeTasksInBlock and receiptsInBlock read the same mixed block", () => {
  const tasks = computeTasksInBlock(BLOCK);
  assert.equal(tasks.length, 1);
  const minimal = RPC.transactions.find((t) => t.name === "minimal");
  assert.equal(hex(computeTaskId(tasks[0])), minimal.expected.task_id);
  const receipts = receiptsInBlock(BLOCK);
  assert.equal(receipts.length, 1, "receipt extraction is unaffected by the task");
  // The anchored receipt answers the canonical task, by identity.
  const canonical = RPC.transactions.find((t) => t.name === "canonical");
  assert.equal(hex(receipts[0].taskId), canonical.expected.task_id);
  assert.equal(computeTasksInBlock({ body: { transactions: [] } }).length, 0);
});

test("unknown and malformed payloads still fail closed", async () => {
  const withPayload = (payload) => ({
    ...BLOCK,
    body: { transactions: [{ ...BLOCK.body.transactions[1], payload }] },
  });
  const clientFor = (block) =>
    new MbongoClient("http://localhost:8080/rpc", {
      fetch: async () => ({
        status: 200,
        text: async () => JSON.stringify({ jsonrpc: "2.0", id: 1, result: block }),
      }),
    });
  assert.equal(RPC.unknown_variant.examples.length, 3);
  for (const example of RPC.unknown_variant.examples) {
    await assert.rejects(
      clientFor(withPayload(example.payload)).getBlockByHeight(1),
      MbongoTransportError,
      JSON.stringify(example.payload),
    );
  }
  // A ComputeTask object with a byte out of range, or the wrong width, is
  // not carried either.
  const good = BLOCK.body.transactions[1].payload.ComputeTask;
  await assert.rejects(
    clientFor(withPayload({ ComputeTask: { ...good, salt: [...good.salt.slice(0, 31), 256] } })).getBlockByHeight(1),
    MbongoTransportError,
  );
  assert.throws(
    () => wireComputeTaskToComputeTask({ ...good, salt: good.salt.slice(0, 31) }),
    (e) => e instanceof MbongoComputeTaskError && e.field === "salt",
  );
  assert.throws(
    () => wireComputeTaskToComputeTask({ ...good, version: 2 }),
    (e) => e instanceof MbongoComputeTaskError && e.field === "version",
  );
  assert.throws(
    () => wireComputeTaskToComputeTask({ ...good, execution_spec: new Array(1025).fill(0) }),
    (e) => e instanceof MbongoComputeTaskError && e.field === "executionSpec",
  );
  // computeTasksInBlock throws rather than under-reports.
  assert.throws(
    () => computeTasksInBlock(withPayload({ ComputeTask: { ...good, executor: "0x00" } })),
    (e) => e instanceof MbongoComputeTaskError && e.field.startsWith("block.body.transactions[0]"),
  );
});
