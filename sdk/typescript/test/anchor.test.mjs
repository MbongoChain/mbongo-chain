// TypeScript side of the shared cross-language AnchorReceipt transaction
// vectors.
//
// Every expected value is read from test-vectors/transaction/anchor-receipt-v1
// .json and the receipt vector it references. Nothing is copied into this file:
// a copied constant would only prove the copy was faithful. Rust reads the same
// two files, so agreement here is real interoperability.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  anchorReceiptSigningPayload,
  signAnchorReceiptTransaction,
  anchorReceiptTransactionHash,
  anchorReceiptTransactionToWire,
  submitAnchorReceipt,
  ANCHOR_RECEIPT_PAYLOAD_PREFIX_BYTES,
  MbongoAnchorError,
  MbongoNumericRangeError,
  MbongoReceiptError,
  MbongoClient,
  receiptHash,
  verifyReceiptSignature,
} from "../dist/index.js";

const load = (rel) =>
  JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));

const TX = load("../../../test-vectors/transaction/anchor-receipt-v1.json");
const RX = load("../../../test-vectors/receipt/receipt-v1.json");

const SUPPORTED_FIXTURE_VERSION = 1;
for (const [name, doc] of [["transaction", TX], ["receipt", RX]]) {
  assert.equal(
    doc.fixture_version,
    SUPPORTED_FIXTURE_VERSION,
    `${name} fixture: unsupported schema version`,
  );
}

const unhex = (s) => {
  assert.ok(!s.startsWith("0x"), "fixture hex must not carry an 0x prefix");
  assert.ok(/^[0-9a-f]*$/.test(s), "fixture hex must be lowercase");
  assert.equal(s.length % 2, 0, "fixture hex must have even length");
  return Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));
};
const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, "0")).join("");

/** Expands the {pattern:"repeat", byte, length} form the receipt fixture uses. */
function metadata(m) {
  assert.equal(m.pattern, "repeat", "unsupported metadata pattern");
  return new Uint8Array(m.length).fill(unhex(m.byte)[0]);
}

/** Resolves a receipt vector by name. Exactly one match is required. */
function receiptVector(name) {
  const matches = RX.valid.filter((v) => v.name === name);
  assert.equal(matches.length, 1, `expected exactly one receipt vector named ${name}`);
  return matches[0];
}

function receiptFrom(name) {
  const v = receiptVector(name);
  return {
    receipt: {
      version: v.receipt.version,
      taskId: unhex(v.receipt.task_id),
      inputCommitment: unhex(v.receipt.input_commitment),
      outputCommitment: unhex(v.receipt.output_commitment),
      executor: unhex(v.receipt.executor),
      metadata: metadata(v.receipt.metadata),
      signature: unhex(v.expected.executor_signature),
    },
    expected: v.expected,
  };
}

const SEED = unhex(RX.test_key.ed25519_seed);
const CANONICAL = TX.valid.find((v) => v.name === "canonical-diagnostic-nonce");

test("the referenced receipt vector resolves in the receipt fixture", () => {
  for (const v of TX.valid) {
    const name = v.transaction.receipt_vector;
    assert.equal(receiptVector(name).name, name);
  }
});

test("every valid vector reproduces the pinned signing payload", () => {
  assert.ok(TX.valid.length > 0, "no valid vectors");
  for (const v of TX.valid) {
    const { receipt } = receiptFrom(v.transaction.receipt_vector);
    const payload = anchorReceiptSigningPayload(receipt, v.transaction.nonce);
    assert.equal(hex(payload), v.expected.signing_payload, `${v.name}: bytes`);
    assert.equal(payload.length, v.expected.signing_payload_length, `${v.name}: length`);
  }
});

test("the receipt bytes sit contiguously at the pinned offset", () => {
  for (const v of TX.valid) {
    const { receipt, expected } = receiptFrom(v.transaction.receipt_vector);
    const payload = anchorReceiptSigningPayload(receipt, v.transaction.nonce);
    const offset = v.expected.receipt_offset;

    assert.equal(offset, ANCHOR_RECEIPT_PAYLOAD_PREFIX_BYTES, `${v.name}: offset constant`);
    assert.equal(offset, payload.length - expected.full_encoding_length, `${v.name}: computed`);
    // No Vec length prefix, no extra discriminant, no second encoding layer.
    assert.equal(hex(payload.subarray(offset)), expected.full_encoding, `${v.name}: suffix`);
    assert.equal(payload[0], 0x03, `${v.name}: tx_type discriminant`);
    assert.equal(payload[offset - 1], 0x01, `${v.name}: payload discriminant`);
  }
});

test("the nonce is encoded little-endian and fixed-width", () => {
  for (const v of TX.valid) {
    const { receipt } = receiptFrom(v.transaction.receipt_vector);
    const payload = anchorReceiptSigningPayload(receipt, v.transaction.nonce);
    const at = 1 + 32 + 32 + 16;
    assert.equal(hex(payload.subarray(at, at + 8)), v.expected.nonce_u64_le, `${v.name}`);
  }
  // The canonical vector must stay asymmetric, or it proves nothing about
  // byte order.
  const le = CANONICAL.expected.nonce_u64_le;
  const reversed = le.match(/../g).reverse().join("");
  assert.notEqual(le, reversed, "the canonical nonce is palindromic");
});

test("the amount occupies sixteen bytes, not eight and not a compact prefix", () => {
  // Every consensus-valid AnchorReceipt carries amount 0, so this package
  // exposes no amount parameter at all and therefore no non-zero u128 path.
  // What is provable here is the width, which is where a naive implementation
  // goes wrong. The fixture's non-zero u128 diagnostic has no public surface
  // in this package and is exercised by the Rust consumer instead.
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const payload = anchorReceiptSigningPayload(receipt, CANONICAL.transaction.nonce);
  const at = 1 + 32 + 32;
  assert.equal(hex(payload.subarray(at, at + 16)), CANONICAL.expected.amount_u128_le);
  assert.equal(CANONICAL.expected.amount_u128_le, "0".repeat(32));
});

test("every valid vector reproduces the pinned transaction signature", () => {
  for (const v of TX.valid) {
    const { receipt } = receiptFrom(v.transaction.receipt_vector);
    const tx = signAnchorReceiptTransaction(receipt, v.transaction.nonce, SEED);
    assert.equal(hex(tx.signature), v.expected.transaction_signature, v.name);
    assert.equal(tx.amount, 0);
    assert.equal(tx.nonce, v.transaction.nonce);
    assert.equal(hex(tx.sender), hex(receipt.executor), "sender is the executor");
    assert.equal(hex(tx.receiver), "00".repeat(32), "receiver is the zero address");
  }
});

test("the full encoding and the transaction hash match", () => {
  for (const v of TX.valid) {
    const { receipt } = receiptFrom(v.transaction.receipt_vector);
    const tx = signAnchorReceiptTransaction(receipt, v.transaction.nonce, SEED);
    const payload = anchorReceiptSigningPayload(receipt, v.transaction.nonce);

    // The signature is the final SCALE field, so the payload is a strict prefix.
    assert.equal(
      hex(payload) + hex(tx.signature),
      v.expected.full_transaction,
      `${v.name}: full encoding`,
    );
    assert.equal(
      v.expected.full_transaction.length / 2,
      v.expected.full_transaction_length,
    );
    // Computed over the full signed encoding by the implementation itself.
    assert.equal(
      hex(anchorReceiptTransactionHash(tx)),
      v.expected.transaction_hash,
      `${v.name}: transaction hash`,
    );
  }
});

test("the two signature domains stay separate", () => {
  const { receipt, expected } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const tx = signAnchorReceiptTransaction(receipt, CANONICAL.transaction.nonce, SEED);

  // One key, because consensus requires sender == executor.
  assert.equal(hex(tx.sender), hex(receipt.executor));
  // Two messages, therefore two signatures.
  assert.notEqual(hex(tx.signature), hex(receipt.signature));
  assert.equal(hex(receipt.signature), expected.executor_signature);
  // The anchored receipt stays independently valid and untouched.
  assert.ok(verifyReceiptSignature(tx.receipt));
  assert.equal(hex(receiptHash(tx.receipt)), expected.receipt_hash);
  // Three distinct values, compared as values: two of them are 32 bytes.
  assert.notEqual(expected.receipt_hash, CANONICAL.expected.transaction_hash);
});

test("the transaction signature is over the raw payload, never a digest of it", () => {
  // The regression that matters most. If someone changes sign(payload) to
  // sign(blake3(payload)) — applying the receipt's scheme to a transaction —
  // the pinned signature stops matching and this fails.
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const tx = signAnchorReceiptTransaction(receipt, CANONICAL.transaction.nonce, SEED);
  const prehashed = TX.invalid.find(
    (v) => v.name === "transaction-signature-over-prehashed-payload",
  );
  assert.ok(prehashed, "the prehash vector is missing from the fixture");
  assert.notEqual(hex(tx.signature), prehashed.transaction_signature);
  assert.equal(hex(tx.signature), CANONICAL.expected.transaction_signature);
});

test("the implementation never produces a domain-confused signature", () => {
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const tx = signAnchorReceiptTransaction(receipt, CANONICAL.transaction.nonce, SEED);
  for (const bad of TX.invalid) {
    assert.equal(bad.expected.transaction_signature_verifies, false, bad.name);
    assert.notEqual(hex(tx.signature), bad.transaction_signature, bad.name);
  }
  // Signing the receipt hash reproduces the receipt's own signature, so
  // reusing either is the same mistake. The fixture says so; check it holds.
  const overReceiptHash = TX.invalid.find(
    (v) => v.name === "transaction-signature-over-receipt-hash",
  );
  assert.equal(overReceiptHash.transaction_signature, hex(receipt.signature));
  assert.notEqual(hex(tx.signature), hex(receipt.signature));
});

test("the wire object matches the pinned serde representation exactly", () => {
  const pinned = TX.serialized_transaction;
  const v = TX.valid.find((x) => x.name === pinned.vector);
  const { receipt } = receiptFrom(v.transaction.receipt_vector);
  const tx = signAnchorReceiptTransaction(receipt, v.transaction.nonce, SEED);
  const wire = anchorReceiptTransactionToWire(tx);

  assert.deepEqual(wire, pinned.object);

  // The mixed representation is the point, so assert it rather than trusting
  // one deep comparison.
  const r = wire.payload.AnchorReceipt;
  for (const f of ["sender", "receiver", "signature"]) {
    assert.match(wire[f], /^0x[0-9a-f]+$/, `transaction.${f} is lowercase 0x hex`);
  }
  for (const f of ["executor", "signature"]) {
    assert.match(r[f], /^0x[0-9a-f]+$/, `receipt.${f} is lowercase 0x hex`);
  }
  for (const f of ["task_id", "input_commitment", "output_commitment", "metadata"]) {
    assert.ok(Array.isArray(r[f]), `receipt.${f} is a number array, not hex`);
  }
  assert.equal(wire.tx_type, "AnchorReceipt");
  assert.deepEqual(Object.keys(wire.payload), ["AnchorReceipt"], "externally tagged");
  assert.equal(typeof wire.amount, "number");
  assert.equal(typeof wire.nonce, "number");
  // No JSON-RPC envelope leaks into the transaction object.
  for (const k of ["jsonrpc", "id", "method", "params"]) {
    assert.ok(!(k in wire), `${k} does not belong on a transaction`);
  }
});

test("a key that does not derive the receipt executor is refused", () => {
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const otherSeed = new Uint8Array(32).fill(0x99);
  assert.throws(
    () => signAnchorReceiptTransaction(receipt, 0, otherSeed),
    (err) => err instanceof MbongoAnchorError && err.field === "secretKey",
  );
  // Wrong width, too.
  for (const len of [0, 31, 33, 64]) {
    assert.throws(
      () => signAnchorReceiptTransaction(receipt, 0, new Uint8Array(len)),
      MbongoAnchorError,
      `${len}-byte key`,
    );
  }
  assert.throws(
    () => signAnchorReceiptTransaction(receipt, 0, "not bytes"),
    MbongoAnchorError,
  );
});

test("an unsafe nonce fails before anything is signed", () => {
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const bad = [-1, 1.5, NaN, Infinity, -Infinity, Number.MAX_SAFE_INTEGER + 1, "7", 7n];
  for (const nonce of bad) {
    assert.throws(
      () => anchorReceiptSigningPayload(receipt, nonce),
      MbongoNumericRangeError,
      `nonce ${String(nonce)}`,
    );
    assert.throws(
      () => signAnchorReceiptTransaction(receipt, nonce, SEED),
      MbongoNumericRangeError,
      `signing with nonce ${String(nonce)}`,
    );
  }
});

test("a receipt that cannot be canonically encoded is refused", () => {
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const cases = [
    ["version", { ...receipt, version: 2 }],
    ["metadata", { ...receipt, metadata: new Uint8Array(4097) }],
    ["taskId", { ...receipt, taskId: new Uint8Array(31) }],
    ["executor", { ...receipt, executor: new Uint8Array(33) }],
    ["signature", { ...receipt, signature: new Uint8Array(63) }],
  ];
  for (const [label, r] of cases) {
    assert.throws(
      () => anchorReceiptSigningPayload(r, 0),
      MbongoReceiptError,
      `receipt with bad ${label}`,
    );
  }
});

test("caller-owned buffers are never mutated and never aliased", () => {
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const before = {
    taskId: hex(receipt.taskId),
    executor: hex(receipt.executor),
    metadata: hex(receipt.metadata),
    signature: hex(receipt.signature),
  };
  const seedCopy = Uint8Array.from(SEED);

  const tx = signAnchorReceiptTransaction(receipt, CANONICAL.transaction.nonce, seedCopy);
  anchorReceiptTransactionHash(tx);
  anchorReceiptTransactionToWire(tx);

  assert.equal(hex(receipt.taskId), before.taskId);
  assert.equal(hex(receipt.executor), before.executor);
  assert.equal(hex(receipt.metadata), before.metadata);
  assert.equal(hex(receipt.signature), before.signature);
  assert.equal(hex(seedCopy), hex(SEED), "the secret key is not mutated");

  // sender must be an independent copy, not a view onto receipt.executor.
  tx.sender[0] ^= 0xff;
  assert.equal(hex(receipt.executor), before.executor);
});

test("submission reuses the existing client and types anchoring rejections", async () => {
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const tx = signAnchorReceiptTransaction(receipt, CANONICAL.transaction.nonce, SEED);

  // Success: the exact wire object reaches submit_transaction, and no second
  // RPC method is invented.
  let seen;
  const okClient = new MbongoClient("http://node.invalid/rpc", {
    fetch: async (_url, init) => {
      seen = JSON.parse(init.body);
      return new Response(
        JSON.stringify({ jsonrpc: "2.0", id: seen.id, result: "0xabc" }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    },
  });
  assert.equal(await submitAnchorReceipt(okClient, tx), "0xabc");
  assert.equal(seen.method, "submit_transaction");
  assert.deepEqual(seen.params, TX.serialized_transaction.object);

  // Every anchoring rule the node enforces maps to a distinct reason.
  const rejections = [
    ["task_id already anchored", "duplicate-task-id"],
    ["task_id already pending", "task-id-pending"],
    ["receipt metadata too large", "metadata-too-large"],
    ["unsupported receipt version", "unsupported-receipt-version"],
    ["sender must equal receipt executor", "sender-executor-mismatch"],
    ["invalid receipt signature", "invalid-receipt-signature"],
    ["invalid signature", "invalid-transaction-signature"],
    ["invalid nonce", "invalid-nonce"],
    ["anchor receipt requires amount 0 and zero receiver", "invalid-anchor-fields"],
    ["insufficient balance", "sender-account-unusable"],
    ["something nobody has seen", "unknown"],
  ];
  for (const [message, reason] of rejections) {
    const client = new MbongoClient("http://node.invalid/rpc", {
      fetch: async () =>
        new Response(
          JSON.stringify({ jsonrpc: "2.0", id: 1, error: { code: -32603, message } }),
          { status: 200, headers: { "content-type": "application/json" } },
        ),
    });
    await assert.rejects(
      () => submitAnchorReceipt(client, tx),
      (err) =>
        err instanceof MbongoAnchorError &&
        err.reason === reason &&
        err.message.includes(message),
      `${message} -> ${reason}`,
    );
  }
});

test("a duplicate task id is reported as such and cannot say who anchored it", async () => {
  const { receipt } = receiptFrom(CANONICAL.transaction.receipt_vector);
  const tx = signAnchorReceiptTransaction(receipt, CANONICAL.transaction.nonce, SEED);
  const client = new MbongoClient("http://node.invalid/rpc", {
    fetch: async () =>
      new Response(
        JSON.stringify({
          jsonrpc: "2.0",
          id: 1,
          error: { code: -32603, message: "task_id already anchored" },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      ),
  });

  await assert.rejects(
    () => submitAnchorReceipt(client, tx),
    (err) => {
      assert.ok(err instanceof MbongoAnchorError);
      assert.equal(err.reason, "duplicate-task-id");
      assert.equal(err.isDuplicateTaskId, true);
      // Deliberately carries no claim about *who* anchored it: nothing in the
      // response can distinguish that, and no public query API exists.
      return true;
    },
  );

  // Replaying the identical signed transaction is byte-identical, which is
  // what makes retry safe before the task is anchored.
  const again = signAnchorReceiptTransaction(receipt, CANONICAL.transaction.nonce, SEED);
  assert.equal(hex(again.signature), hex(tx.signature));
  assert.deepEqual(anchorReceiptTransactionToWire(again), anchorReceiptTransactionToWire(tx));
});
