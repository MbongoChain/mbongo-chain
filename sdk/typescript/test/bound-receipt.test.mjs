// The receipt that answers a task, bound the way RFC 0005 rules (q)–(s)
// require, and the AnchorReceipt that carries it.
//
// Every expected value is read from test-vectors/compute-task/anchor-binding-v1
// .json and the task and key fixtures it references. Rust drives the same file
// through consensus; agreement here is the client half of the same proof.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  MbongoAnchorError,
  MbongoClient,
  MbongoComputeTaskError,
  MbongoReceiptBindingError,
  anchorReceiptSigningPayload,
  anchorReceiptTransactionHash,
  anchorReceiptTransactionToWire,
  assertReceiptBoundToTask,
  computeTaskId,
  encodeReceipt,
  receiptHash,
  signAnchorReceiptTransaction,
  signBoundReceipt,
  submitAnchorReceipt,
  verifyReceiptSignature,
} from "../dist/index.js";

const load = (rel) => JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));

const BIND = load("../../../test-vectors/compute-task/anchor-binding-v1.json");
const CT = load("../../../test-vectors/compute-task/compute-task-v1.json");
const RX = load("../../../test-vectors/receipt/receipt-v1.json");
const RPC = load("../../../test-vectors/rpc/compute-task-rpc-v1.json");
for (const [name, doc] of [["binding", BIND], ["compute-task", CT], ["receipt", RX], ["rpc", RPC]]) {
  assert.equal(doc.fixture_version, 1, `${name} fixture: unsupported schema version`);
}

const unhex = (s) => Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));
const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, "0")).join("");

function taskFrom(name) {
  const matches = CT.tasks.filter((v) => v.name === name);
  assert.equal(matches.length, 1, `expected exactly one task vector named ${name}`);
  const t = matches[0].task;
  const s = t.execution_spec;
  return {
    version: t.version,
    submitter: unhex(t.submitter),
    executor: unhex(t.executor),
    salt: unhex(t.salt),
    inputCommitment: unhex(t.input_commitment),
    executionSpec: s.pattern === "repeat" ? new Uint8Array(s.length).fill(unhex(s.byte)[0]) : unhex(s.hex),
  };
}

function vector(name) {
  const matches = BIND.vectors.filter((v) => v.name === name);
  assert.equal(matches.length, 1, `expected exactly one binding vector named ${name}`);
  return matches[0];
}

const EXECUTOR_SEED = unhex(CT.test_keys.executor.ed25519_seed);
const SUBMITTER_SEED = unhex(RX.test_key.ed25519_seed);
const CANONICAL = taskFrom("canonical");

/** The receipt a binding vector describes, exactly as pinned. */
function receiptFrom(v) {
  const r = v.receipt;
  assert.equal(r.metadata.length, 0, "binding vectors carry empty metadata");
  return {
    version: r.version,
    taskId: unhex(r.task_id),
    inputCommitment: unhex(r.input_commitment),
    outputCommitment: unhex(r.output_commitment),
    executor: unhex(r.executor),
    metadata: new Uint8Array(0),
    signature: unhex(v.expected.executor_signature),
  };
}

// ── the bound receipt ───────────────────────────────────────────────────

test("signBoundReceipt reproduces the bound vectors byte for byte", () => {
  const bound = BIND.vectors.filter((v) => v.consensus.valid);
  assert.equal(bound.length, 2, "two bound vectors");
  for (const v of bound) {
    const task = taskFrom(v.task_vector);
    const receipt = signBoundReceipt(
      task,
      { outputCommitment: unhex(v.receipt.output_commitment) },
      EXECUTOR_SEED,
    );
    assert.equal(hex(receipt.taskId), v.receipt.task_id, `${v.name}: taskId`);
    assert.equal(hex(receipt.taskId), hex(computeTaskId(task)), `${v.name}: derived`);
    assert.equal(hex(receipt.inputCommitment), v.receipt.input_commitment, `${v.name}: inputCommitment`);
    assert.equal(hex(receipt.executor), v.receipt.executor, `${v.name}: executor`);
    assert.equal(hex(receiptHash(receipt)), v.expected.receipt_hash, `${v.name}: hash`);
    assert.equal(hex(receipt.signature), v.expected.executor_signature, `${v.name}: signature`);
    assert.equal(hex(encodeReceipt(receipt)), v.expected.receipt_full_encoding, `${v.name}: encoding`);
    assert.ok(verifyReceiptSignature(receipt));
    assertReceiptBoundToTask(receipt, task);

    // Anchored by the executor: the existing anchor builder, unchanged,
    // reproduces the pinned transaction.
    const tx = signAnchorReceiptTransaction(receipt, v.transaction.nonce, EXECUTOR_SEED);
    assert.equal(hex(anchorReceiptSigningPayload(receipt, v.transaction.nonce)), v.expected.signing_payload, `${v.name}: signing payload`);
    assert.equal(hex(tx.signature), v.expected.transaction_signature, `${v.name}: tx signature`);
    assert.equal(hex(tx.sender), hex(task.executor), `${v.name}: sender is the named executor`);
    assert.equal(hex(anchorReceiptTransactionHash(tx)), v.expected.transaction_hash, `${v.name}: tx hash`);
  }
});

test("only the executor the task named can build the bound receipt", () => {
  const fields = { outputCommitment: new Uint8Array(32).fill(0x44) };
  // The submitter's key, when the task names someone else.
  assert.notEqual(hex(CANONICAL.submitter), hex(CANONICAL.executor));
  assert.throws(
    () => signBoundReceipt(CANONICAL, fields, SUBMITTER_SEED),
    (e) => e instanceof MbongoComputeTaskError && e.field === "executorSecretKey",
  );
  // Any other key.
  assert.throws(
    () => signBoundReceipt(CANONICAL, fields, new Uint8Array(32).fill(0xe9)),
    (e) => e instanceof MbongoComputeTaskError && e.field === "executorSecretKey",
  );
  // A malformed key.
  assert.throws(() => signBoundReceipt(CANONICAL, fields, new Uint8Array(16)), MbongoComputeTaskError);
  // The named executor succeeds, and the receipt names it — never the submitter.
  const receipt = signBoundReceipt(CANONICAL, fields, EXECUTOR_SEED);
  assert.equal(hex(receipt.executor), hex(CANONICAL.executor));
  assert.notEqual(hex(receipt.executor), hex(CANONICAL.submitter));
});

test("a task whose submitter is its own executor is answered by that one key", () => {
  // Legal under RFC 0005; the same key commits and answers.
  const self = { ...CANONICAL, executor: CANONICAL.submitter };
  const receipt = signBoundReceipt(self, { outputCommitment: new Uint8Array(32) }, SUBMITTER_SEED);
  assert.equal(hex(receipt.executor), hex(self.submitter));
  assert.equal(hex(receipt.taskId), hex(computeTaskId(self)));
  assert.notEqual(hex(receipt.taskId), hex(computeTaskId(CANONICAL)), "a different executor is a different task");
});

test("assertReceiptBoundToTask names the first failing binding, in rule order", () => {
  const good = receiptFrom(vector("bound-named-executor"));
  assertReceiptBoundToTask(good, CANONICAL);

  // (r): the fixture's mismatched-commitment receipt.
  const r = receiptFrom(vector("input-commitment-mismatch"));
  assert.throws(
    () => assertReceiptBoundToTask(r, CANONICAL),
    (e) => e instanceof MbongoReceiptBindingError && e.binding === "input-commitment",
  );
  // (s): the fixture's receipt from the submitter, who was not named.
  const s = receiptFrom(vector("executor-not-named"));
  assert.throws(
    () => assertReceiptBoundToTask(s, CANONICAL),
    (e) => e instanceof MbongoReceiptBindingError && e.binding === "executor",
  );
  // (q) locally: a receipt for an unregistered task_id is simply not this
  // task's receipt. The node decides registration; here it is identity.
  const q = receiptFrom(vector("unknown-task"));
  assert.throws(
    () => assertReceiptBoundToTask(q, CANONICAL),
    (e) => e instanceof MbongoReceiptBindingError && e.binding === "task-id",
  );
  // task_id of another registered task: judged against that task.
  const other = taskFrom("spec-max-1024");
  assert.throws(
    () => assertReceiptBoundToTask(good, other),
    (e) => e instanceof MbongoReceiptBindingError && e.binding === "task-id",
  );
  // One flipped bit in the commitment.
  const flipped = { ...good, inputCommitment: Uint8Array.from(good.inputCommitment) };
  flipped.inputCommitment[31] ^= 1;
  assert.throws(
    () => assertReceiptBoundToTask(flipped, CANONICAL),
    (e) => e instanceof MbongoReceiptBindingError && e.binding === "input-commitment",
  );
});

test("the executor-not-named receipt is well formed and still cannot be bound", () => {
  // The squatting receipt of RFC 0005 §9.1: valid signature, valid anchor
  // signature, and no way to satisfy rule (s). The low-level APIs will build
  // it — they describe any receipt — and the binding check refuses it.
  const v = vector("executor-not-named");
  const receipt = receiptFrom(v);
  assert.ok(verifyReceiptSignature(receipt));
  const tx = signAnchorReceiptTransaction(receipt, v.transaction.nonce, SUBMITTER_SEED);
  assert.equal(hex(tx.signature), v.expected.transaction_signature);
  assert.throws(() => assertReceiptBoundToTask(receipt, CANONICAL), MbongoReceiptBindingError);
  assert.equal(v.consensus.rule, "s");
});

test("node rejections for rules q, r and s become typed anchor reasons", async () => {
  const receipt = receiptFrom(vector("bound-named-executor"));
  const tx = signAnchorReceiptTransaction(receipt, 0, EXECUTOR_SEED);
  const reject = (message) => async () => ({
    status: 500,
    text: async () => JSON.stringify({ jsonrpc: "2.0", id: 1, error: { code: -32603, message } }),
  });
  for (const [message, reason] of [
    ["internal backend error: receipt task_id is not a registered task", "task-not-registered"],
    ["internal backend error: receipt input_commitment does not match the task", "input-commitment-mismatch"],
    ["internal backend error: receipt executor is not authorised by the task", "executor-not-authorised"],
    ["internal backend error: task_id already anchored", "duplicate-task-id"],
  ]) {
    const client = new MbongoClient("http://localhost:8080/rpc", { fetch: reject(message) });
    await assert.rejects(
      submitAnchorReceipt(client, tx),
      (e) => e instanceof MbongoAnchorError && e.reason === reason,
      message,
    );
  }
});

test("the anchored receipt in the RPC block is the bound vector, carried unchanged", () => {
  // The existing anchor wire shape is untouched: the bound receipt renders to
  // exactly the AnchorReceipt object the RPC fixture pins inside its block.
  const v = vector("bound-named-executor");
  const receipt = receiptFrom(v);
  const tx = signAnchorReceiptTransaction(receipt, v.transaction.nonce, EXECUTOR_SEED);
  const wire = anchorReceiptTransactionToWire(tx);
  const pinned = RPC.block.object.body.transactions[2];
  assert.equal(JSON.stringify({ ...wire, amount: 0, nonce: Number(wire.nonce) }), JSON.stringify(pinned));
});
