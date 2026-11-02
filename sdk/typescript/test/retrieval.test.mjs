// Reading anchored receipts back out of a block.
//
// The central proof is the round trip: the exact wire receipt pinned by
// test-vectors/transaction/anchor-receipt-v1.json must convert back to the
// canonical receipt pinned by test-vectors/receipt/receipt-v1.json, with its
// hash and signature intact. No expected value is copied into this file.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  wireReceiptToReceipt,
  receiptsInBlock,
  receiptHash,
  verifyReceiptSignature,
  MbongoReceiptError,
  MbongoClient,
  MAX_RECEIPT_METADATA_BYTES,
} from "../dist/index.js";

const load = (rel) =>
  JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));

const TX = load("../../../test-vectors/transaction/anchor-receipt-v1.json");
const RX = load("../../../test-vectors/receipt/receipt-v1.json");
for (const [name, doc] of [["transaction", TX], ["receipt", RX]]) {
  assert.equal(doc.fixture_version, 1, `${name} fixture: unsupported schema version`);
}

const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, "0")).join("");
const unhex = (s) => Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));

/** The wire receipt pinned by #94, inside the pinned Transaction object. */
const PINNED_WIRE = TX.serialized_transaction.object.payload.AnchorReceipt;

/** Resolves a receipt vector by name. Exactly one match is required. */
function receiptVector(name) {
  const matches = RX.valid.filter((v) => v.name === name);
  assert.equal(matches.length, 1, `expected exactly one receipt vector named ${name}`);
  return matches[0];
}

const REFERENCED = TX.valid.find(
  (v) => v.name === TX.serialized_transaction.vector,
).transaction.receipt_vector;

/** A deep clone, so a test that mutates its input cannot leak into another. */
const wire = (over = {}) => ({ ...structuredClone(PINNED_WIRE), ...over });

const blockWith = (...payloads) => ({
  header: { parent_hash: "0x00", state_root: "0x00", transactions_root: "0x00", timestamp: 0, height: 1 },
  body: {
    transactions: payloads.map((payload) => ({
      tx_type: payload === "None" ? "Transfer" : "AnchorReceipt",
      sender: "0x" + "11".repeat(32),
      receiver: "0x" + "00".repeat(32),
      amount: 0,
      nonce: 0,
      payload,
      signature: "0x" + "22".repeat(64),
    })),
  },
});

// ── the round trip ──────────────────────────────────────────────────────

test("the pinned wire receipt converts back to the pinned canonical receipt", () => {
  const v = receiptVector(REFERENCED);
  const got = wireReceiptToReceipt(PINNED_WIRE);

  assert.equal(got.version, v.receipt.version);
  assert.equal(hex(got.taskId), v.receipt.task_id);
  assert.equal(hex(got.inputCommitment), v.receipt.input_commitment);
  assert.equal(hex(got.outputCommitment), v.receipt.output_commitment);
  assert.equal(hex(got.executor), v.receipt.executor);
  assert.equal(hex(got.signature), v.expected.executor_signature);
  assert.equal(got.metadata.length, v.receipt.metadata.length);
});

test("conversion preserves the receipt hash and the signature", () => {
  const v = receiptVector(REFERENCED);
  const got = wireReceiptToReceipt(PINNED_WIRE);

  // The whole point: what came off the wire still means the same thing.
  assert.equal(hex(receiptHash(got)), v.expected.receipt_hash);
  assert.equal(verifyReceiptSignature(got), true);
});

test("every field is a fresh copy, and nothing is mutated", () => {
  const source = wire();
  const before = JSON.stringify(source);
  const got = wireReceiptToReceipt(source);

  assert.equal(JSON.stringify(source), before, "the wire object is not mutated");

  got.taskId[0] ^= 0xff;
  got.executor[0] ^= 0xff;
  assert.equal(JSON.stringify(source), before, "the result does not alias the input");

  source.task_id[0] = (source.task_id[0] + 1) % 256;
  const again = wireReceiptToReceipt(source);
  assert.notEqual(hex(again.taskId), hex(wireReceiptToReceipt(PINNED_WIRE).taskId));
});

// ── strict decoding ─────────────────────────────────────────────────────

test("an unsupported version is refused", () => {
  for (const version of [0, 2, 255]) {
    assert.throws(() => wireReceiptToReceipt(wire({ version })), MbongoReceiptError, `v${version}`);
  }
  for (const version of [1.5, "1", null, undefined, NaN]) {
    assert.throws(() => wireReceiptToReceipt(wire({ version })), MbongoReceiptError, String(version));
  }
});

test("wrong field widths are refused", () => {
  const cases = [
    ["task_id", new Array(31).fill(0)],
    ["task_id", new Array(33).fill(0)],
    ["input_commitment", new Array(0)],
    ["output_commitment", new Array(31).fill(0)],
    ["executor", "0x" + "aa".repeat(31)],
    ["executor", "0x" + "aa".repeat(33)],
    ["signature", "0x" + "aa".repeat(63)],
    ["signature", "0x" + "aa".repeat(65)],
  ];
  for (const [field, value] of cases) {
    assert.throws(
      () => wireReceiptToReceipt(wire({ [field]: value })),
      MbongoReceiptError,
      `${field} width`,
    );
  }
});

test("a byte outside 0..=255 is refused rather than truncated", () => {
  // Uint8Array would silently wrap these, producing a receipt whose hash does
  // not match the chain's.
  for (const bad of [-1, 256, 300, 1.5, NaN, Infinity, "7", null, undefined, 7n]) {
    const task = new Array(32).fill(0);
    task[5] = bad;
    assert.throws(
      () => wireReceiptToReceipt(wire({ task_id: task })),
      MbongoReceiptError,
      `byte ${String(bad)}`,
    );
  }
});

test("malformed hex is refused", () => {
  const cases = [
    "aa".repeat(32),                       // no 0x prefix
    "0x" + "AA".repeat(32),                // uppercase
    "0x" + "zz".repeat(32),                // not hex
    "0x" + "a".repeat(63),                 // odd length
    "0x",                                  // empty
    123,                                   // not a string
    null,
  ];
  for (const executor of cases) {
    assert.throws(
      () => wireReceiptToReceipt(wire({ executor })),
      MbongoReceiptError,
      `executor ${String(executor)}`,
    );
  }
});

test("metadata is accepted up to the consensus bound and refused beyond it", () => {
  const at = wireReceiptToReceipt(wire({ metadata: new Array(MAX_RECEIPT_METADATA_BYTES).fill(0xab) }));
  assert.equal(at.metadata.length, MAX_RECEIPT_METADATA_BYTES);
  assert.equal(at.metadata[0], 0xab);

  assert.throws(
    () => wireReceiptToReceipt(wire({ metadata: new Array(MAX_RECEIPT_METADATA_BYTES + 1).fill(0) })),
    MbongoReceiptError,
    "4097",
  );
  // Empty metadata is ordinary.
  assert.equal(wireReceiptToReceipt(wire({ metadata: [] })).metadata.length, 0);
  // And it must still be an array.
  assert.throws(() => wireReceiptToReceipt(wire({ metadata: "0x" })), MbongoReceiptError);
});

test("a non-object is refused", () => {
  for (const bad of [null, undefined, 42, "receipt", []]) {
    assert.throws(() => wireReceiptToReceipt(bad), MbongoReceiptError, String(bad));
  }
});

// ── extraction ──────────────────────────────────────────────────────────

test("a block with no transactions yields no receipts", () => {
  assert.deepEqual(receiptsInBlock(blockWith()), []);
});

test("a block with transactions but no anchoring yields no receipts", () => {
  assert.deepEqual(receiptsInBlock(blockWith("None", "None")), []);
});

test("a single anchored receipt is returned", () => {
  const got = receiptsInBlock(blockWith({ AnchorReceipt: PINNED_WIRE }));
  assert.equal(got.length, 1);
  assert.equal(hex(receiptHash(got[0])), receiptVector(REFERENCED).expected.receipt_hash);
});

test("many receipts are returned in transaction order", () => {
  // Consensus forbids repeating one task_id in a block, so give each a
  // different one — which also makes the ordering observable.
  const ids = [0x0a, 0x0b, 0x0c];
  const block = blockWith(
    "None",
    { AnchorReceipt: wire({ task_id: new Array(32).fill(ids[0]) }) },
    "None",
    { AnchorReceipt: wire({ task_id: new Array(32).fill(ids[1]) }) },
    { AnchorReceipt: wire({ task_id: new Array(32).fill(ids[2]) }) },
  );
  const got = receiptsInBlock(block);
  assert.equal(got.length, 3);
  assert.deepEqual(
    got.map((r) => r.taskId[0]),
    ids,
    "receipts follow the order of the transactions that carried them",
  );
});

test("a malformed anchored receipt fails closed instead of being skipped", () => {
  const block = blockWith(
    { AnchorReceipt: PINNED_WIRE },
    { AnchorReceipt: wire({ task_id: new Array(31).fill(0) }) },
  );
  assert.throws(
    () => receiptsInBlock(block),
    (err) => {
      assert.ok(err instanceof MbongoReceiptError);
      // The error names which transaction carried it.
      assert.match(err.field, /transactions\[1\]/);
      return true;
    },
  );
});

test("a malformed block shape is refused", () => {
  for (const bad of [null, undefined, 42, {}, { body: null }, { body: { transactions: "no" } }]) {
    assert.throws(() => receiptsInBlock(bad), MbongoReceiptError, JSON.stringify(bad));
  }
});

// ── boundaries ──────────────────────────────────────────────────────────

test("extraction performs no network call and needs no client", () => {
  // If either function reached for the network, this would throw: there is no
  // fetch available under this name and no client was supplied.
  const realFetch = globalThis.fetch;
  globalThis.fetch = () => {
    throw new Error("receiptsInBlock must not touch the network");
  };
  try {
    assert.equal(receiptsInBlock(blockWith({ AnchorReceipt: PINNED_WIRE })).length, 1);
    assert.ok(wireReceiptToReceipt(PINNED_WIRE));
  } finally {
    globalThis.fetch = realFetch;
  }
  // Neither takes a client argument.
  assert.equal(receiptsInBlock.length, 1);
  assert.equal(wireReceiptToReceipt.length, 1);
});

test("verification stays explicit", () => {
  const tampered = wireReceiptToReceipt(wire({ signature: "0x" + "11".repeat(64) }));
  // Extraction succeeded even though the signature is wrong: decoding is not
  // verification, and the caller decides.
  assert.equal(verifyReceiptSignature(tampered), false);
  const block = blockWith({ AnchorReceipt: wire({ signature: "0x" + "11".repeat(64) }) });
  assert.equal(receiptsInBlock(block).length, 1, "a bad signature does not stop decoding");
});

test("known-height retrieval costs exactly one RPC call", async () => {
  const requests = [];
  const client = new MbongoClient("http://node.invalid/rpc", {
    fetch: async (_url, init) => {
      const body = JSON.parse(init.body);
      requests.push(body.method);
      return new Response(
        JSON.stringify({
          jsonrpc: "2.0",
          id: body.id,
          result: blockWith({ AnchorReceipt: PINNED_WIRE }),
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    },
  });

  const block = await client.getBlockByHeight(1);
  const receipts = receiptsInBlock(block);

  assert.deepEqual(requests, ["get_block_by_height"], "one call, and only that one");
  assert.equal(receipts.length, 1);
  assert.equal(hex(receiptHash(receipts[0])), receiptVector(REFERENCED).expected.receipt_hash);
});
