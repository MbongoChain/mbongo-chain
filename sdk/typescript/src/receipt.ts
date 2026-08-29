/**
 * Canonical receipt primitives.
 *
 * Pure, synchronous and offline: nothing here touches the network. These
 * functions reproduce, in TypeScript, the encoding and hashing the node
 * performs — so a caller can compute a `receipt_hash` and check an executor
 * signature without trusting a node to do it.
 *
 * Authority:
 * - `docs/specs/RECEIPT_SPEC_v0.1.md` — structure, canonical encoding, hash
 * - `docs/rfcs/0002-receipt-anchoring-v0.3.md` — the activated v0.3 rules and
 *   the 4096-byte metadata bound
 * - `docs/specs/PROTOCOL_LOCK_v0.3.md` — FROZEN
 *
 * Correctness is checked against `test-vectors/receipt/receipt-v1.json`, the
 * shared fixture Rust also reads. That file is the source of truth; no
 * expected value is duplicated here.
 */

import { blake3 } from "@noble/hashes/blake3.js";
import { ed25519 } from "@noble/curves/ed25519.js";

import { MbongoReceiptError } from "./errors.js";

/** The only receipt version these primitives understand. */
export const RECEIPT_VERSION = 1;

/**
 * Maximum `metadata` length in bytes.
 *
 * Normative through RFC 0002 §3 and frozen by `PROTOCOL_LOCK_v0.3` rule (e),
 * even though `RECEIPT_SPEC_v0.1` omits it. A receipt above this bound cannot
 * be anchored, so it is rejected before anything canonical-looking is
 * produced for it.
 */
export const MAX_RECEIPT_METADATA_BYTES = 4096;

const HASH_BYTES = 32;
const SIGNATURE_BYTES = 64;

/**
 * A receipt, in its canonical byte form.
 *
 * Fields are `Uint8Array` rather than the hex strings the RPC types use: these
 * values are hashed and signed, and carrying them as text invites signing the
 * text instead of the bytes. The RPC layer keeps hex because that is its wire
 * form; this layer keeps bytes because that is its canonical form.
 */
export interface Receipt {
  /** Protocol version. Must be {@link RECEIPT_VERSION}. */
  version: number;
  /** 32 bytes. Opaque to the chain. */
  taskId: Uint8Array;
  /** 32 bytes. Opaque to the chain. */
  inputCommitment: Uint8Array;
  /** 32 bytes. Opaque to the chain. */
  outputCommitment: Uint8Array;
  /** 32 bytes: the executor's Ed25519 public key. */
  executor: Uint8Array;
  /** Opaque bytes, at most {@link MAX_RECEIPT_METADATA_BYTES}. */
  metadata: Uint8Array;
  /** 64 bytes: Ed25519 over the raw 32-byte receipt hash. */
  signature: Uint8Array;
}

function requireBytes(field: string, value: unknown, length: number): Uint8Array {
  if (!(value instanceof Uint8Array)) {
    throw new MbongoReceiptError(field, `expected a Uint8Array, got ${typeof value}`);
  }
  if (value.length !== length) {
    throw new MbongoReceiptError(
      field,
      `expected exactly ${length} bytes, got ${value.length}`,
    );
  }
  return value;
}

/**
 * Validates everything the canonical encoding depends on.
 *
 * `signature` is checked for width here even though it is excluded from the
 * signing payload. A receipt whose signature is the wrong size is malformed
 * whatever you intend to do with it, and one contract for one type is easier
 * to reason about than a function that accepts a receipt its sibling rejects.
 */
function assertCanonical(receipt: Receipt): void {
  if (!Number.isInteger(receipt.version)) {
    throw new MbongoReceiptError("version", "must be an integer");
  }
  if (receipt.version < 0 || receipt.version > 0xff) {
    throw new MbongoReceiptError("version", "must fit in a u8");
  }
  if (receipt.version !== RECEIPT_VERSION) {
    // Fail closed. Hashing an unrecognised version would produce a
    // canonical-looking digest for rules we do not know.
    throw new MbongoReceiptError(
      "version",
      `unsupported receipt version ${receipt.version}; this package implements version ${RECEIPT_VERSION}`,
    );
  }

  requireBytes("taskId", receipt.taskId, HASH_BYTES);
  requireBytes("inputCommitment", receipt.inputCommitment, HASH_BYTES);
  requireBytes("outputCommitment", receipt.outputCommitment, HASH_BYTES);
  requireBytes("executor", receipt.executor, HASH_BYTES);
  requireBytes("signature", receipt.signature, SIGNATURE_BYTES);

  if (!(receipt.metadata instanceof Uint8Array)) {
    throw new MbongoReceiptError(
      "metadata",
      `expected a Uint8Array, got ${typeof receipt.metadata}`,
    );
  }
  if (receipt.metadata.length > MAX_RECEIPT_METADATA_BYTES) {
    throw new MbongoReceiptError(
      "metadata",
      `${receipt.metadata.length} bytes exceeds the ${MAX_RECEIPT_METADATA_BYTES}-byte consensus maximum; ` +
        "a receipt this large cannot be anchored",
    );
  }
}

/**
 * SCALE compact encoding of a length.
 *
 * Only the two modes reachable under the metadata bound are implemented:
 *
 * ```
 * n < 64          -> one byte:  n << 2                 (mode 0b00)
 * 64 <= n < 16384 -> two bytes: LE16((n << 2) | 0b01)  (mode 0b01)
 * ```
 *
 * The four-byte mode begins at 16384, far above the 4096 bound, so it is
 * unreachable for a valid receipt and deliberately absent — code that cannot
 * run cannot be tested, and untested encoding code is a liability.
 *
 * The width change at 64 is the mistake this package most needs to avoid: at
 * the 4096 bound the prefix is two bytes, not one.
 */
function compactLength(n: number): Uint8Array {
  if (n < 64) {
    return new Uint8Array([n << 2]);
  }
  const encoded = (n << 2) | 0b01;
  return new Uint8Array([encoded & 0xff, (encoded >>> 8) & 0xff]);
}

/**
 * SCALE encoding of the signing payload: every field **except** `signature`.
 *
 * Never mutates the receipt or its arrays.
 *
 * @throws {MbongoReceiptError} the receipt is malformed, its version is
 * unsupported, or its metadata exceeds the consensus bound.
 */
export function encodeReceiptSigningPayload(receipt: Receipt): Uint8Array {
  assertCanonical(receipt);

  const prefix = compactLength(receipt.metadata.length);
  const out = new Uint8Array(
    1 + HASH_BYTES * 4 + prefix.length + receipt.metadata.length,
  );

  let at = 0;
  out[at++] = receipt.version;
  out.set(receipt.taskId, at);
  at += HASH_BYTES;
  out.set(receipt.inputCommitment, at);
  at += HASH_BYTES;
  out.set(receipt.outputCommitment, at);
  at += HASH_BYTES;
  out.set(receipt.executor, at);
  at += HASH_BYTES;
  out.set(prefix, at);
  at += prefix.length;
  out.set(receipt.metadata, at);

  return out;
}

/**
 * Full canonical SCALE encoding: the signing payload followed by the 64-byte
 * signature. The payload is therefore a strict prefix of this.
 *
 * @throws {MbongoReceiptError} as {@link encodeReceiptSigningPayload}.
 */
export function encodeReceipt(receipt: Receipt): Uint8Array {
  const payload = encodeReceiptSigningPayload(receipt);
  const out = new Uint8Array(payload.length + SIGNATURE_BYTES);
  out.set(payload, 0);
  out.set(receipt.signature, payload.length);
  return out;
}

/**
 * The receipt hash: `BLAKE3` over the signing payload.
 *
 * Because the signature is excluded from that payload, changing it leaves
 * this hash untouched.
 *
 * @throws {MbongoReceiptError} as {@link encodeReceiptSigningPayload}.
 */
export function receiptHash(receipt: Receipt): Uint8Array {
  return blake3(encodeReceiptSigningPayload(receipt));
}

/**
 * Verifies the executor's Ed25519 signature over the **raw 32 bytes** of the
 * receipt hash — never over its hex text.
 *
 * ## What a `true` result means
 *
 * The receipt is structurally canonical, its version is supported, its
 * metadata is within bound, and the key in `executor` signed this exact
 * receipt.
 *
 * ## What it does not mean
 *
 * It says nothing about whether the computation was performed correctly,
 * whether the receipt is anchored on chain, whether the task exists, whether
 * the executor was authorised to run it, or whether anything was settled.
 * The chain itself validates structure, signature and uniqueness — and
 * nothing about the work.
 *
 * @returns `false` when the signature does not verify. A well-formed receipt
 * with a wrong signature is not malformed data, so it is not an exception.
 * @throws {MbongoReceiptError} when the receipt is malformed, its version is
 * unsupported, or its metadata exceeds the bound — none of which can be
 * expressed as a signature verdict.
 */
export function verifyReceiptSignature(receipt: Receipt): boolean {
  const hash = receiptHash(receipt);
  try {
    return ed25519.verify(receipt.signature, hash, receipt.executor);
  } catch {
    // A malformed key or signature point is a verification failure, not a
    // structural error: the widths were already checked above.
    return false;
  }
}
