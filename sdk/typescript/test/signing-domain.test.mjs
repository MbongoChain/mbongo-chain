// The transaction signing domain of RFC 0006 section 2, through the SDK's
// signers: what is signed, for which network, and that a signature for one
// network, or a v0.4 signature, is refused on another. Expected values come
// from test-vectors/transaction/anchor-receipt-v2.json and its v1 file; Rust
// reads the same files.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { ed25519 } from "@noble/curves/ed25519.js";

import {
  CHAIN_ID_BYTES,
  MbongoAnchorError,
  MbongoComputeTaskError,
  TX_SIGNING_DOMAIN,
  anchorReceiptSigningPayload,
  anchorReceiptTransactionFields,
  computeTaskSigningPayload,
  computeTaskTransactionFields,
  signAnchorReceiptTransaction,
  signComputeTaskTransaction,
  transactionSigningPayload,
} from "../dist/index.js";

const load = (rel) => JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));
const TX = load("../../../test-vectors/transaction/anchor-receipt-v2.json");
const TX_V1 = load("../../../test-vectors/transaction/anchor-receipt-v1.json");
const CT = load("../../../test-vectors/compute-task/compute-task-v2.json");
const RX = load("../../../test-vectors/receipt/receipt-v1.json");
assert.equal(TX.fixture_version, 2);
assert.equal(TX_V1.fixture_version, 1);
assert.equal(CT.fixture_version, 2);

const unhex = (s) => Uint8Array.from(Buffer.from(s, "hex"));
const hex = (b) => Buffer.from(b).toString("hex");
const CHAIN = unhex(TX.chain.chain_id);
const OTHER = unhex(TX.chain.other_chain_id);
const SEED = unhex(RX.test_key.ed25519_seed);
const PUB = unhex(RX.test_key.public_key);

function receiptOf(name) {
  const v = RX.valid.find((r) => r.name === name);
  const meta = v.receipt.metadata;
  return {
    version: v.receipt.version,
    taskId: unhex(v.receipt.task_id),
    inputCommitment: unhex(v.receipt.input_commitment),
    outputCommitment: unhex(v.receipt.output_commitment),
    executor: unhex(v.receipt.executor),
    metadata: typeof meta === "string" ? unhex(meta) : new Uint8Array(0),
    signature: unhex(v.expected.executor_signature),
  };
}

test("the domain is mbongo-tx:v2 and the chain_id is the dev chain's", () => {
  assert.equal(Buffer.from(TX_SIGNING_DOMAIN).toString("ascii"), "mbongo-tx:v2");
  assert.equal(hex(TX_SIGNING_DOMAIN), TX.chain.signing_domain.hex);
  assert.equal(CHAIN_ID_BYTES, 32);
  // The dev genesis file's chain_id, as RFC 0006 section 1 defines it.
  assert.equal(TX.chain.chain_id, "2fd700acc09b3d1752ca6fe21e7d6f4844917c5a9fcd41a34fe1d13d7deed1a4");
});

test("the signed bytes are the domain, the chain_id, then the fields", () => {
  assert.equal(TX.valid.length, 2);
  for (const v of TX.valid) {
    const receipt = receiptOf(v.transaction.receipt_vector);
    const fields = anchorReceiptTransactionFields(receipt, v.transaction.nonce);
    const signed = anchorReceiptSigningPayload(receipt, v.transaction.nonce, CHAIN);
    assert.equal(hex(fields), v.expected.transaction_fields, v.name);
    assert.equal(hex(signed), v.expected.signing_payload, v.name);
    assert.equal(hex(signed), hex(TX_SIGNING_DOMAIN) + hex(CHAIN) + hex(fields), v.name);
    assert.equal(hex(transactionSigningPayload(fields, CHAIN)), hex(signed), v.name);
  }
});

test("a signature for one network does not verify on another", () => {
  for (const v of TX.valid) {
    const receipt = receiptOf(v.transaction.receipt_vector);
    const ours = signAnchorReceiptTransaction(receipt, v.transaction.nonce, SEED, CHAIN);
    const theirs = signAnchorReceiptTransaction(receipt, v.transaction.nonce, SEED, OTHER);
    assert.equal(hex(ours.signature), v.expected.transaction_signature, v.name);
    assert.notEqual(hex(theirs.signature), hex(ours.signature), v.name);
    const here = anchorReceiptSigningPayload(receipt, v.transaction.nonce, CHAIN);
    const there = anchorReceiptSigningPayload(receipt, v.transaction.nonce, OTHER);
    assert.ok(ed25519.verify(ours.signature, here, PUB), v.name);
    assert.equal(ed25519.verify(ours.signature, there, PUB), false, `${v.name}: ours there`);
    assert.equal(ed25519.verify(theirs.signature, here, PUB), false, `${v.name}: theirs here`);
  }
});

test("a v0.4 signature, over the bare fields, verifies on no network", () => {
  assert.equal(TX_V1.valid.length, 2);
  for (const v of TX_V1.valid) {
    const receipt = receiptOf(v.transaction.receipt_vector);
    const sig = unhex(v.expected.transaction_signature);
    const fields = anchorReceiptTransactionFields(receipt, v.transaction.nonce);
    assert.equal(hex(fields), v.expected.signing_payload, `${v.name}: v1 signed the fields`);
    assert.ok(ed25519.verify(sig, fields, PUB), `${v.name}: a sound v1 signature`);
    for (const chain of [CHAIN, OTHER]) {
      const signed = anchorReceiptSigningPayload(receipt, v.transaction.nonce, chain);
      assert.equal(ed25519.verify(sig, signed, PUB), false, v.name);
    }
  }
});

test("compute-task signatures carry the domain too", () => {
  const entry = CT.valid.find((v) => v.name === "canonical");
  assert.ok(entry);
  const t = CT.tasks.find((x) => x.name === entry.task_vector).task;
  const task = {
    version: t.version,
    submitter: unhex(t.submitter),
    executor: unhex(t.executor),
    salt: unhex(t.salt),
    inputCommitment: unhex(t.input_commitment),
    executionSpec: unhex(t.execution_spec.hex),
  };
  const fields = computeTaskTransactionFields(task, entry.transaction.nonce);
  assert.equal(hex(fields), entry.expected.transaction_fields);
  assert.equal(hex(computeTaskSigningPayload(task, entry.transaction.nonce, CHAIN)), entry.expected.signing_payload);
  const tx = signComputeTaskTransaction(task, entry.transaction.nonce, SEED, CHAIN);
  assert.equal(hex(tx.signature), entry.expected.transaction_signature);
  const there = computeTaskSigningPayload(task, entry.transaction.nonce, OTHER);
  assert.equal(ed25519.verify(tx.signature, there, PUB), false);
});

test("a chainId is required and is 32 bytes", () => {
  const receipt = receiptOf("empty-metadata");
  for (const bad of [undefined, null, new Uint8Array(31), new Uint8Array(33), "00".repeat(32)]) {
    assert.throws(() => signAnchorReceiptTransaction(receipt, 0, SEED, bad), MbongoAnchorError);
    assert.throws(() => anchorReceiptSigningPayload(receipt, 0, bad), MbongoAnchorError);
    assert.throws(() => transactionSigningPayload(new Uint8Array(0), bad), RangeError);
  }
  const t = CT.tasks.find((x) => x.name === "canonical").task;
  const task = {
    version: t.version,
    submitter: unhex(t.submitter),
    executor: unhex(t.executor),
    salt: unhex(t.salt),
    inputCommitment: unhex(t.input_commitment),
    executionSpec: unhex(t.execution_spec.hex),
  };
  assert.throws(() => signComputeTaskTransaction(task, 0, SEED, new Uint8Array(16)), MbongoComputeTaskError);
  assert.throws(() => computeTaskSigningPayload(task, 0, undefined), MbongoComputeTaskError);
});
