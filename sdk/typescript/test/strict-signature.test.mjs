// TypeScript side of the shared strict Ed25519 vectors
// (test-vectors/signature/ed25519-strict-v2.json). Rust reads the same file.
//
// Every case goes through the SDK's strict verifier. The receipt cases also go
// through the public verifyReceiptSignature, the check a client runs before
// anchoring. Every valid receipt of receipt-v1.json must stay valid.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { verifyReceiptSignature } from "../dist/index.js";
import { verifyEd25519Strict } from "../dist/ed25519-strict.js";

const load = (rel) => JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));
const SX = load("../../../test-vectors/signature/ed25519-strict-v2.json");
const RX = load("../../../test-vectors/receipt/receipt-v1.json");
for (const [name, doc] of [["strict", SX], ["receipt", RX]]) {
  assert.equal(doc.fixture_version, doc.supersedes ? 2 : 1, `${name} fixture: unsupported schema version`);
}

const unhex = (s) => {
  assert.ok(/^[0-9a-f]*$/.test(s) && s.length % 2 === 0, "lowercase even-length hex");
  return Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));
};

function receiptFrom(name, overrides = {}) {
  const matches = RX.valid.filter((v) => v.name === name);
  assert.equal(matches.length, 1, `exactly one receipt vector named ${name}`);
  const v = matches[0];
  assert.equal(v.receipt.metadata.pattern, "repeat");
  return {
    version: v.receipt.version,
    taskId: unhex(v.receipt.task_id),
    inputCommitment: unhex(v.receipt.input_commitment),
    outputCommitment: unhex(v.receipt.output_commitment),
    executor: unhex(v.receipt.executor),
    metadata: new Uint8Array(v.receipt.metadata.length).fill(unhex(v.receipt.metadata.byte)[0]),
    signature: unhex(v.expected.executor_signature),
    ...overrides,
  };
}

test("every case has the strict verdict", () => {
  assert.equal(SX.cases.length, 19, "expected 19 cases");
  let valid = 0;
  for (const c of SX.cases) {
    assert.ok(SX.rules.includes(c.rule), `${c.name}: unknown rule`);
    const verdict = verifyEd25519Strict(unhex(c.signature), unhex(c.message), unhex(c.public_key));
    assert.equal(verdict, c.expected === "valid", `${c.name} (${c.rule})`);
    if (verdict) valid += 1;
  }
  assert.equal(valid, 4, "expected 4 valid cases");
});

test("receipt cases through verifyReceiptSignature", () => {
  const cases = SX.cases.filter((c) => c.kind === "receipt");
  assert.equal(cases.length, 5, "expected 5 receipt cases");
  for (const c of cases) {
    const receipt = receiptFrom(c.receipt.receipt_vector, {
      executor: unhex(c.receipt.executor),
      signature: unhex(c.signature),
    });
    assert.equal(verifyReceiptSignature(receipt), c.expected === "valid", `${c.name} (${c.rule})`);
  }
});

test("malformed lengths are invalid, never an exception", () => {
  assert.equal(SX.malformed_lengths.length, 3);
  for (const m of SX.malformed_lengths) {
    assert.equal(verifyEd25519Strict(unhex(m.signature), unhex(m.message), unhex(m.public_key)), false, m.name);
  }
  assert.equal(verifyEd25519Strict(null, new Uint8Array(0), new Uint8Array(32)), false);
});

test("every valid receipt vector still verifies", () => {
  assert.ok(RX.valid.length > 0);
  for (const v of RX.valid) assert.ok(verifyReceiptSignature(receiptFrom(v.name)), v.name);
});
