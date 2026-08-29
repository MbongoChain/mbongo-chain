/**
 * TypeScript side of the shared cross-language receipt vectors.
 *
 * Every expected value is read from `test-vectors/receipt/receipt-v1.json`,
 * the same file `crates/mbongo-core/tests/receipt_vectors.rs` reads. Nothing
 * is copied here: a duplicated constant would only prove the copy was
 * faithful.
 *
 * That is the point of this suite. The fixture's values were derived without
 * the Rust encoder; Rust then had to agree with them; and now an independent
 * stack — its own SCALE, its own BLAKE3, its own Ed25519 — has to agree too.
 * Two implementations meeting on the same pinned bytes is the interoperability
 * proof.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  MAX_RECEIPT_METADATA_BYTES,
  MbongoReceiptError,
  RECEIPT_VERSION,
  encodeReceipt,
  encodeReceiptSigningPayload,
  receiptHash,
  verifyReceiptSignature,
} from "../dist/index.js";

/** Resolved against this file, so it does not depend on the working directory. */
const FIXTURE_URL = new URL(
  "../../../test-vectors/receipt/receipt-v1.json",
  import.meta.url,
);

const fixture = JSON.parse(readFileSync(FIXTURE_URL, "utf8"));

const SUPPORTED_FIXTURE_VERSION = 1;
assert.equal(
  fixture.fixture_version,
  SUPPORTED_FIXTURE_VERSION,
  "unsupported fixture schema version",
);

/** Decodes the fixture's lowercase, unprefixed hex, strictly. */
function bytes(hex) {
  assert.equal(typeof hex, "string", "expected a hex string");
  assert.ok(!hex.startsWith("0x"), "fixture hex must not carry an 0x prefix");
  assert.match(hex, /^[0-9a-f]*$/, "fixture hex must be lowercase");
  assert.equal(hex.length % 2, 0, "fixture hex must have an even length");
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

const hex = (u8) => [...u8].map((b) => b.toString(16).padStart(2, "0")).join("");

/** Expands the only metadata pattern the schema defines. */
function metadata(spec) {
  assert.equal(spec.pattern, "repeat", "unsupported metadata pattern");
  const byte = bytes(spec.byte);
  assert.equal(byte.length, 1, "metadata byte must be exactly one byte");
  return new Uint8Array(spec.length).fill(byte[0]);
}

/** Fixture entry -> Receipt. Test-only: production APIs know nothing of the schema. */
function toReceipt(entry, { signature } = {}) {
  const r = entry.receipt;
  return {
    version: r.version,
    taskId: bytes(r.task_id),
    inputCommitment: bytes(r.input_commitment),
    outputCommitment: bytes(r.output_commitment),
    executor: bytes(r.executor),
    metadata: metadata(r.metadata),
    signature:
      signature ??
      bytes(r.signature ?? entry.expected.executor_signature ?? "00".repeat(64)),
  };
}

const validVectors = fixture.valid;
const byMetadataLength = (n) =>
  validVectors.find((v) => v.receipt.metadata.length === n) ??
  assert.fail(`no vector with metadata length ${n}`);

// ── The five shared valid vectors ────────────────────────────────────

test("every shared valid vector matches this implementation", () => {
  assert.equal(validVectors.length, 5);

  for (const entry of validVectors) {
    const name = entry.name;
    const expected = entry.expected;
    const receipt = toReceipt(entry);

    const payload = encodeReceiptSigningPayload(receipt);
    const full = encodeReceipt(receipt);

    // Compact prefix, right after the five fixed-width fields.
    const head = 1 + 32 * 4;
    const prefix = bytes(expected.metadata_compact_prefix);
    assert.deepEqual(
      payload.slice(head, head + prefix.length),
      prefix,
      `${name}: compact metadata prefix`,
    );

    assert.equal(
      payload.length,
      expected.signing_payload_length,
      `${name}: signing payload length`,
    );
    assert.equal(
      full.length,
      expected.full_encoding_length,
      `${name}: full encoding length`,
    );

    assert.equal(
      hex(receiptHash(receipt)),
      expected.receipt_hash,
      `${name}: receipt hash`,
    );
    assert.equal(
      verifyReceiptSignature(receipt),
      true,
      `${name}: pinned signature must verify`,
    );

    // Full byte pinning where the fixture carries it.
    if (expected.signing_payload !== undefined) {
      assert.equal(hex(payload), expected.signing_payload, `${name}: payload bytes`);
    }
    if (expected.full_encoding !== undefined) {
      assert.equal(hex(full), expected.full_encoding, `${name}: full bytes`);
    }

    // The payload is a strict prefix of the full encoding.
    assert.deepEqual(
      full.slice(0, payload.length),
      payload,
      `${name}: full encoding starts with the signing payload`,
    );
  }
});

test("the 4096 vector reproduces the consensus-bound encoding", () => {
  const entry = byMetadataLength(MAX_RECEIPT_METADATA_BYTES);
  const receipt = toReceipt(entry);
  const payload = encodeReceiptSigningPayload(receipt);
  const head = 1 + 32 * 4;

  assert.deepEqual(
    payload.slice(head, head + 2),
    new Uint8Array([0x01, 0x40]),
    "at the bound the compact prefix is two bytes",
  );
  assert.equal(payload.length, 4227);
  assert.equal(encodeReceipt(receipt).length, 4291);
  assert.equal(payload[head + 2], 0xab, "metadata starts after a two-byte prefix");
  assert.equal(hex(receiptHash(receipt)), entry.expected.receipt_hash);
  assert.equal(verifyReceiptSignature(receipt), true);
});

// ── SCALE boundary, stated outright ──────────────────────────────────

test("the compact prefix widens between 63 and 64", () => {
  const head = 1 + 32 * 4;
  const p63 = encodeReceiptSigningPayload(toReceipt(byMetadataLength(63)));
  const p64 = encodeReceiptSigningPayload(toReceipt(byMetadataLength(64)));

  assert.deepEqual(p63.slice(head, head + 1), new Uint8Array([0xfc]));
  assert.deepEqual(p64.slice(head, head + 2), new Uint8Array([0x01, 0x01]));
  assert.equal(
    p64.length - p63.length,
    2,
    "one more byte of metadata, one more of prefix",
  );
});

test("metadata over the bound throws before anything canonical is produced", () => {
  const entry = byMetadataLength(3);
  const receipt = toReceipt(entry);
  receipt.metadata = new Uint8Array(MAX_RECEIPT_METADATA_BYTES + 1).fill(0xab);

  for (const fn of [encodeReceiptSigningPayload, encodeReceipt, receiptHash, verifyReceiptSignature]) {
    assert.throws(() => fn(receipt), MbongoReceiptError, `${fn.name} must refuse`);
  }
  // Exactly at the bound is fine.
  receipt.metadata = new Uint8Array(MAX_RECEIPT_METADATA_BYTES).fill(0xab);
  assert.doesNotThrow(() => receiptHash(receipt));
});

// ── The three shared invalid vectors ─────────────────────────────────

test("shared invalid vectors fail in the way the fixture states", () => {
  assert.equal(fixture.invalid.length, 3);

  for (const entry of fixture.invalid) {
    const name = entry.name;
    const reason = entry.expected.rejected_by;

    if (reason === "metadata_bound") {
      // Malformed by policy: it cannot be canonically encoded at all.
      assert.equal(entry.receipt.metadata.length, 4097);
      assert.throws(
        () => receiptHash(toReceipt(entry)),
        (err) => {
          assert.ok(err instanceof MbongoReceiptError);
          assert.equal(err.field, "metadata");
          return true;
        },
        `${name}: must throw`,
      );
    } else if (reason === "signature") {
      // Well-formed data, wrong signature: a verdict, not an exception.
      const receipt = toReceipt(entry);
      assert.doesNotThrow(
        () => receiptHash(receipt),
        `${name}: the receipt itself is structurally sound`,
      );
      assert.equal(
        verifyReceiptSignature(receipt),
        false,
        `${name}: signature must not verify`,
      );
      if (entry.expected.receipt_hash !== undefined) {
        assert.equal(
          hex(receiptHash(receipt)),
          entry.expected.receipt_hash,
          `${name}: the hash itself is still correct`,
        );
      }
    } else {
      assert.fail(`${name}: unknown rejection reason ${reason}`);
    }
  }
});

// ── The two cross-language traps ─────────────────────────────────────

test("the signature is over the raw hash, not its hex text", () => {
  // Signing the displayed hex string instead of the digest is the mistake
  // most available to another language. The pinned signature verifies over
  // the raw bytes; a receipt whose hash bytes are the ASCII of that hex does
  // not exist, so the check is that verification uses the digest itself.
  const entry = byMetadataLength(3);
  const receipt = toReceipt(entry);
  const digest = receiptHash(receipt);

  assert.equal(digest.length, 32, "the message is 32 raw bytes");
  assert.equal(hex(digest), entry.expected.receipt_hash);
  assert.equal(verifyReceiptSignature(receipt), true);

  // The hex text is 64 ASCII bytes — a different message entirely.
  const asHexText = new TextEncoder().encode(hex(digest));
  assert.equal(asHexText.length, 64);
  assert.notDeepEqual(asHexText.slice(0, 32), digest);
});

test("the signature is excluded from the hash but not from the full encoding", () => {
  const entry = byMetadataLength(3);
  const receipt = toReceipt(entry);

  const payloadBefore = encodeReceiptSigningPayload(receipt);
  const hashBefore = receiptHash(receipt);
  const fullBefore = encodeReceipt(receipt);
  assert.equal(verifyReceiptSignature(receipt), true);

  const mutated = { ...receipt, signature: new Uint8Array(64).fill(0xff) };

  assert.deepEqual(
    encodeReceiptSigningPayload(mutated),
    payloadBefore,
    "the signing payload ignores the signature",
  );
  assert.deepEqual(receiptHash(mutated), hashBefore, "so does the hash");
  assert.notDeepEqual(
    encodeReceipt(mutated),
    fullBefore,
    "but the full encoding carries it",
  );
  assert.equal(
    verifyReceiptSignature(mutated),
    false,
    "and verification notices",
  );
});

// ── Structural refusals ──────────────────────────────────────────────

test("wrong field widths are refused", () => {
  const base = toReceipt(byMetadataLength(3));
  const cases = [
    ["taskId", new Uint8Array(31)],
    ["taskId", new Uint8Array(33)],
    ["inputCommitment", new Uint8Array(31)],
    ["outputCommitment", new Uint8Array(33)],
    ["executor", new Uint8Array(31)],
    ["signature", new Uint8Array(63)],
    ["signature", new Uint8Array(65)],
  ];
  for (const [field, value] of cases) {
    const receipt = { ...base, [field]: value };
    assert.throws(
      () => encodeReceiptSigningPayload(receipt),
      (err) => {
        assert.ok(err instanceof MbongoReceiptError);
        assert.equal(err.field, field);
        return true;
      },
      `${field} at ${value.length} bytes must be refused`,
    );
  }
});

test("a non-Uint8Array field is refused", () => {
  const base = toReceipt(byMetadataLength(3));
  for (const field of ["taskId", "executor", "metadata", "signature"]) {
    assert.throws(
      () => encodeReceiptSigningPayload({ ...base, [field]: "00".repeat(32) }),
      MbongoReceiptError,
      `${field} as a string must be refused`,
    );
  }
});

test("unsupported versions fail closed", () => {
  const base = toReceipt(byMetadataLength(3));
  assert.equal(RECEIPT_VERSION, 1);

  for (const version of [0, 2, 255]) {
    assert.throws(
      () => receiptHash({ ...base, version }),
      (err) => {
        assert.ok(err instanceof MbongoReceiptError);
        assert.equal(err.field, "version");
        return true;
      },
      `version ${version} is u8-valid but unsupported`,
    );
  }
  for (const version of [-1, 256, 1.5, NaN]) {
    assert.throws(
      () => receiptHash({ ...base, version }),
      MbongoReceiptError,
      `version ${version} must be refused`,
    );
  }
});

// ── Caller data is not touched ───────────────────────────────────────

test("public operations do not mutate caller-owned arrays", () => {
  const receipt = toReceipt(byMetadataLength(64));
  const snapshot = {
    taskId: receipt.taskId.slice(),
    inputCommitment: receipt.inputCommitment.slice(),
    outputCommitment: receipt.outputCommitment.slice(),
    executor: receipt.executor.slice(),
    metadata: receipt.metadata.slice(),
    signature: receipt.signature.slice(),
  };

  encodeReceiptSigningPayload(receipt);
  encodeReceipt(receipt);
  receiptHash(receipt);
  verifyReceiptSignature(receipt);

  for (const [field, before] of Object.entries(snapshot)) {
    assert.deepEqual(receipt[field], before, `${field} was mutated`);
  }

  // And the returned buffers are independent of the input.
  const payload = encodeReceiptSigningPayload(receipt);
  payload.fill(0);
  assert.deepEqual(receipt.metadata, snapshot.metadata);
});
