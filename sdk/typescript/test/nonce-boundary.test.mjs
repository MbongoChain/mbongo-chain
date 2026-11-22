// TypeScript side of the shared nonce-boundary and account vectors
// (test-vectors/transaction/nonce-boundary-v2.json,
// test-vectors/account/account-v1.json).
//
// Every expected value is read from those files. Rust reads the same files.
//
// Two kinds of check, kept apart on purpose:
// - AnchorReceipt vectors go through the SDK's own builders, signer and hash,
//   at the nonce bound and beyond it.
// - The SDK has no Transfer or Stake codec and no account codec. Those vectors
//   are read here by a small fixed-layout reader and checked with the same
//   Ed25519 and BLAKE3 the SDK uses, so the two languages are shown to read
//   the same bytes the same way. No SDK behaviour changes.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { blake3 } from "@noble/hashes/blake3.js";
import { ed25519 } from "@noble/curves/ed25519.js";

import {
  anchorReceiptSigningPayload,
  anchorReceiptTransactionFields,
  transactionSigningPayload,
  signAnchorReceiptTransaction,
  anchorReceiptTransactionHash,
  MbongoNumericRangeError,
  U64_MAX,
  U128_MAX,
} from "../dist/index.js";

const load = (rel) => JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));
const NX = load("../../../test-vectors/transaction/nonce-boundary-v2.json");
const AX = load("../../../test-vectors/account/account-v1.json");
// The network the v2 fixtures sign for (RFC 0006 section 2).
const CHAIN = Uint8Array.from(Buffer.from(NX.chain.chain_id, "hex"));
const RX = load("../../../test-vectors/receipt/receipt-v1.json");

for (const [name, doc] of [["nonce", NX], ["account", AX], ["receipt", RX]]) {
  assert.equal(doc.fixture_version, doc.supersedes ? 2 : 1, `${name} fixture: unsupported schema version`);
}

const unhex = (s) => {
  assert.ok(/^[0-9a-f]*$/.test(s) && s.length % 2 === 0, "lowercase even-length hex");
  return Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));
};
const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, "0")).join("");
const leToBigInt = (bytes) => bytes.reduceRight((acc, b) => (acc << 8n) | BigInt(b), 0n);

const SEED = unhex(RX.test_key.ed25519_seed);
const PUB = unhex(RX.test_key.public_key);
const resolve = (v) => (v === "<test_key>" || v === "<receipt.executor>" ? PUB : unhex(v));

const MAX_TRANSACTION_NONCE = BigInt(NX.constants.max_transaction_nonce);
const TYPE_BY_BYTE = { 0x00: "Transfer", 0x02: "Stake" };

/**
 * Fixed-layout reader for a signed Transfer or Stake transaction:
 * tx_type[1] sender[32] receiver[32] amount[16] nonce[8] payload[1] signature[64].
 * Returns null for anything that is not exactly that.
 */
function readPlain(bytes) {
  const o = NX.signing_formula.offsets_in_the_full_transaction;
  if (bytes.length !== NX.signing_formula.plain_full_transaction_length) return null;
  const txType = TYPE_BY_BYTE[bytes[o.tx_type]];
  if (txType === undefined || bytes[o.payload] !== 0x00) return null;
  return {
    txType,
    sender: bytes.subarray(o.sender, o.receiver),
    receiver: bytes.subarray(o.receiver, o.amount),
    amount: leToBigInt(bytes.subarray(o.amount, o.nonce)),
    nonce: leToBigInt(bytes.subarray(o.nonce, o.payload)),
    payload: bytes.subarray(0, o.signature),
    signature: bytes.subarray(o.signature),
  };
}

test("constants agree with the SDK's integer bounds", () => {
  assert.equal(BigInt(NX.constants.u64_max), U64_MAX);
  assert.equal(BigInt(NX.constants.u128_max), U128_MAX);
  assert.equal(MAX_TRANSACTION_NONCE, U64_MAX - 1n);
});

test("plain transactions: every field, signature and hash read back as derived", () => {
  assert.equal(NX.plain.length, 9, "expected 9 plain vectors");
  let beyond = 0;
  for (const v of NX.plain) {
    const full = unhex(v.expected.full_transaction);
    const tx = readPlain(full);
    assert.ok(tx, `${v.name}: must read as a plain transaction`);
    assert.equal(tx.txType, v.transaction.tx_type, v.name);
    assert.deepEqual(tx.sender, resolve(v.transaction.sender), v.name);
    assert.deepEqual(tx.receiver, resolve(v.transaction.receiver), v.name);
    assert.equal(tx.amount, BigInt(v.transaction.amount), `${v.name}: amount`);
    assert.equal(tx.nonce, BigInt(v.transaction.nonce), `${v.name}: nonce`);
    assert.equal(hex(tx.payload), v.expected.transaction_fields, `${v.name}: signing payload`);
    assert.equal(hex(tx.signature), v.expected.transaction_signature, `${v.name}: signature`);
    const signed = transactionSigningPayload(tx.payload, CHAIN);
    assert.equal(hex(signed), v.expected.signing_payload, `${v.name}: signed bytes`);
    assert.ok(ed25519.verify(tx.signature, signed, PUB), `${v.name}: signature verifies`);
    assert.equal(hex(ed25519.sign(signed, SEED)), v.expected.transaction_signature, `${v.name}: re-signed`);
    assert.equal(ed25519.verify(tx.signature, tx.payload, PUB), false, `${v.name}: not over the bare fields`);
    assert.equal(hex(blake3(full)), v.expected.transaction_hash, `${v.name}: hash`);
    if (tx.nonce > MAX_TRANSACTION_NONCE) beyond += 1;
  }
  assert.equal(beyond, 2, "expected 2 plain vectors beyond the bound");
});

/** Builds the SDK receipt for a named receipt vector. */
function receiptFrom(name) {
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
  };
}

test("AnchorReceipt at and beyond the bound: the SDK builds exactly the derived bytes", () => {
  assert.equal(NX.anchor.length, 2, "expected 2 anchor vectors");
  for (const v of NX.anchor) {
    const receipt = receiptFrom(v.transaction.receipt_vector);
    const nonce = BigInt(v.transaction.nonce);
    const payload = anchorReceiptTransactionFields(receipt, nonce);
    assert.equal(payload.length, v.expected.transaction_fields_length, v.name);
    assert.equal(hex(payload), v.expected.transaction_fields, `${v.name}: signing payload`);
    const tx = signAnchorReceiptTransaction(receipt, nonce, SEED, CHAIN);
    assert.equal(tx.nonce, nonce, v.name);
    assert.equal(hex(tx.signature), v.expected.transaction_signature, `${v.name}: signature`);
    assert.equal(hex(anchorReceiptTransactionHash(tx)), v.expected.transaction_hash, `${v.name}: hash`);
  }
});

test("nonces outside u64 are refused before anything is encoded", () => {
  assert.equal(NX.sdk_input_domain.length, 2, "expected 2 input-domain cases");
  const receipt = receiptFrom("empty-metadata");
  for (const c of NX.sdk_input_domain) {
    assert.throws(() => anchorReceiptTransactionFields(receipt, BigInt(c.nonce)), MbongoNumericRangeError, c.name);
    assert.throws(() => signAnchorReceiptTransaction(receipt, BigInt(c.nonce, CHAIN), SEED), MbongoNumericRangeError, c.name);
  }
});

test("malformed encodings are not plain transactions", () => {
  assert.equal(NX.malformed_encodings.length, 6, "expected 6 malformed encodings");
  for (const m of NX.malformed_encodings) {
    const bytes = unhex(m.hex);
    assert.equal(bytes.length, m.hex_length, `${m.name}: length`);
    assert.equal(readPlain(bytes), null, `${m.name}: ${m.reason}`);
  }
});

test("signatures over the wrong message do not verify", () => {
  assert.equal(NX.invalid_signatures.length, 4, "expected 4 invalid signatures");
  for (const s of NX.invalid_signatures) {
    const base = NX.plain.filter((p) => p.name === s.base_vector);
    assert.equal(base.length, 1, `${s.name}: exactly one base vector`);
    const payload = unhex(base[0].expected.signing_payload); // the v2 signed bytes
    assert.ok(ed25519.verify(unhex(base[0].expected.transaction_signature), payload, PUB));
    assert.equal(ed25519.verify(unhex(s.transaction_signature), payload, PUB), false, `${s.name}: ${s.reason}`);
  }
});

test("state-transition cases reference only known vectors", () => {
  // Block application is a node concern; Rust consumes these through
  // apply_block. This checks the references resolve, so neither side can
  // read a case the other cannot.
  const names = new Set(NX.plain.map((p) => p.name));
  assert.equal(NX.state_transition.length, 7, "expected 7 state-transition cases");
  for (const c of NX.state_transition) {
    for (const b of c.blocks) for (const n of b.transactions) assert.ok(names.has(n), `${c.name}: ${n}`);
  }
});

test("accounts: SCALE(Account) reads back as derived", () => {
  const { offsets, length } = AX.layout;
  assert.equal(length, 56);
  assert.equal(AX.valid.length, 4, "expected 4 account vectors");
  for (const v of AX.valid) {
    const enc = unhex(v.expected.encoding);
    assert.equal(enc.length, length, `${v.name}: length`);
    assert.deepEqual(enc.subarray(offsets.address, offsets.balance), resolve(v.account.address), v.name);
    assert.equal(leToBigInt(enc.subarray(offsets.balance, offsets.nonce)), BigInt(v.account.balance), `${v.name}: balance`);
    assert.equal(leToBigInt(enc.subarray(offsets.nonce)), BigInt(v.account.nonce), `${v.name}: nonce`);
  }
  assert.equal(AX.malformed_encodings.length, 3, "expected 3 malformed accounts");
  for (const m of AX.malformed_encodings) {
    assert.equal(unhex(m.hex).length, m.hex_length, m.name);
    assert.notEqual(m.hex_length, length, `${m.name}: must not be an account`);
  }
});
