/**
 * Reading anchored receipts back out of a block.
 *
 * This is the boundary between the two representations the package keeps
 * deliberately separate. On the wire a receipt is a {@link WireReceipt}: hex
 * strings where the Rust type has a custom serializer, arrays of numbers where
 * it does not. In canonical form it is a {@link Receipt}, all `Uint8Array`,
 * which is what can be hashed and verified.
 *
 * Both functions here are **pure, synchronous and offline**. They perform no
 * network call and hold no client. Retrieval is composition:
 *
 * ```typescript
 * const block    = await client.getBlockByHeight(knownHeight); // 1 RPC call
 * const receipts = receiptsInBlock(block);                     // 0 calls
 * ```
 *
 * ## Known height only
 *
 * There is no `task_id` to height index anywhere in the chain, so this path
 * works only when the height is already known — typically because the caller
 * recorded it at submission time. Nothing here discovers a height, scans the
 * chain, or looks a receipt up by `task_id`. Filtering by `task_id` **within**
 * a block the caller already has is one line over the returned array and is
 * deliberately not an API: a function taking a bare `task_id` would read as a
 * chain-side lookup, which does not exist.
 */

import { MbongoReceiptError } from "./errors.js";
import { MAX_RECEIPT_METADATA_BYTES, RECEIPT_VERSION, type Receipt } from "./receipt.js";
import type { Block, WireReceipt } from "./types.js";

const HASH_BYTES = 32;
const SIGNATURE_BYTES = 64;

/**
 * Decodes a `0x`-prefixed lowercase hex string of an exact byte width.
 *
 * Private on purpose. The node emits hex only where a Rust type carries a
 * custom serializer, so a general-purpose hex utility would invite callers to
 * apply it to fields that are not hex at all.
 */
function hexBytes(field: string, value: unknown, length: number): Uint8Array {
  if (typeof value !== "string") {
    throw new MbongoReceiptError(field, `expected a hex string, got ${typeof value}`);
  }
  if (!value.startsWith("0x")) {
    throw new MbongoReceiptError(field, "expected a 0x-prefixed hex string");
  }
  const body = value.slice(2);
  if (body.length !== length * 2) {
    throw new MbongoReceiptError(
      field,
      `expected exactly ${length} bytes (${length * 2} hex characters), got ${body.length / 2}`,
    );
  }
  if (!/^[0-9a-f]*$/.test(body)) {
    throw new MbongoReceiptError(field, "expected lowercase hexadecimal characters");
  }
  const out = new Uint8Array(length);
  for (let i = 0; i < length; i++) {
    out[i] = Number.parseInt(body.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

/**
 * Copies a JSON array of byte values into a fresh `Uint8Array`.
 *
 * Every element must be an integer in `0..=255`. A value outside that range
 * would be silently truncated by `Uint8Array`, producing a receipt whose hash
 * does not match the chain's — which looks correct and is not.
 */
function byteArray(
  field: string,
  value: unknown,
  exactLength?: number,
): Uint8Array {
  if (!Array.isArray(value)) {
    throw new MbongoReceiptError(
      field,
      `expected an array of byte values, got ${typeof value}`,
    );
  }
  if (exactLength !== undefined && value.length !== exactLength) {
    throw new MbongoReceiptError(
      field,
      `expected exactly ${exactLength} bytes, got ${value.length}`,
    );
  }
  const out = new Uint8Array(value.length);
  for (let i = 0; i < value.length; i++) {
    const b: unknown = value[i];
    if (typeof b !== "number" || !Number.isInteger(b) || b < 0 || b > 255) {
      throw new MbongoReceiptError(
        `${field}[${i}]`,
        `expected an integer in 0..=255, got ${String(b)}`,
      );
    }
    out[i] = b;
  }
  return out;
}

/**
 * Converts a receipt from its JSON wire form to canonical bytes.
 *
 * Representation only: it decodes, and does nothing else. It performs no
 * network call, computes no hash, and **does not verify the signature** —
 * pass the result to `verifyReceiptSignature` for that. Nothing is mutated,
 * and every returned array is a fresh copy, so the result does not alias the
 * input.
 *
 * Fails closed on every field. A wire receipt that cannot be decoded exactly
 * is an error, never a partially populated receipt.
 *
 * @throws {MbongoReceiptError} the version is unsupported, a field has the
 * wrong width or type, a byte is out of range, the hex is malformed, or the
 * metadata exceeds the consensus bound.
 */
export function wireReceiptToReceipt(wire: WireReceipt): Receipt {
  if (wire === null || typeof wire !== "object") {
    throw new MbongoReceiptError("receipt", `expected an object, got ${typeof wire}`);
  }
  if (!Number.isInteger(wire.version)) {
    throw new MbongoReceiptError("version", "must be an integer");
  }
  if (wire.version !== RECEIPT_VERSION) {
    // Fail closed, as the canonical primitives do: decoding an unrecognised
    // version would produce a receipt shaped by rules we do not know.
    throw new MbongoReceiptError(
      "version",
      `unsupported receipt version ${wire.version}; this package implements version ${RECEIPT_VERSION}`,
    );
  }

  const metadata = byteArray("metadata", wire.metadata);
  if (metadata.length > MAX_RECEIPT_METADATA_BYTES) {
    throw new MbongoReceiptError(
      "metadata",
      `${metadata.length} bytes exceeds the ${MAX_RECEIPT_METADATA_BYTES}-byte consensus maximum`,
    );
  }

  return {
    version: wire.version,
    taskId: byteArray("taskId", wire.task_id, HASH_BYTES),
    inputCommitment: byteArray("inputCommitment", wire.input_commitment, HASH_BYTES),
    outputCommitment: byteArray("outputCommitment", wire.output_commitment, HASH_BYTES),
    executor: hexBytes("executor", wire.executor, HASH_BYTES),
    metadata,
    signature: hexBytes("signature", wire.signature, SIGNATURE_BYTES),
  };
}

/**
 * Returns the canonical receipts anchored in a block, in transaction order.
 *
 * Pure and offline: **no network call**, and no client. Give it a block you
 * already fetched, typically with `getBlockByHeight` for a height you already
 * know.
 *
 * A block may carry **0, 1 or many** `AnchorReceipt` transactions — consensus
 * only forbids repeating one `task_id` within a block — so this always returns
 * an array. An empty array means the block anchored nothing, which is ordinary
 * and not an error.
 *
 * A transaction that claims to carry a receipt but whose payload cannot be
 * decoded **throws**. Skipping it would silently under-report what a block
 * contains, which is worse than failing.
 *
 * The receipts are returned as decoded, not as verified. A block returned by a
 * node has passed that node's consensus validation, which already checked each
 * receipt's version, metadata bound, executor identity and signature — but
 * this package verified none of it. Call `verifyReceiptSignature` yourself if
 * you need your own proof:
 *
 * ```typescript
 * const receipts = receiptsInBlock(block);
 * const verified = receipts.filter(verifyReceiptSignature);
 * ```
 *
 * @throws {MbongoReceiptError} the block shape is wrong, or an anchored
 * receipt cannot be decoded.
 */
export function receiptsInBlock(block: Block): Receipt[] {
  if (block === null || typeof block !== "object") {
    throw new MbongoReceiptError("block", `expected a block object, got ${typeof block}`);
  }
  const body: unknown = block.body;
  if (body === null || typeof body !== "object") {
    throw new MbongoReceiptError("block.body", "expected a block body object");
  }
  const transactions: unknown = (body as { transactions?: unknown }).transactions;
  if (!Array.isArray(transactions)) {
    throw new MbongoReceiptError("block.body.transactions", "expected an array");
  }

  const receipts: Receipt[] = [];
  for (let i = 0; i < transactions.length; i++) {
    const payload: unknown = transactions[i]?.payload;
    if (payload === "None" || payload === undefined) continue;
    if (payload === null || typeof payload !== "object") {
      throw new MbongoReceiptError(
        `block.body.transactions[${i}].payload`,
        `expected "None" or an AnchorReceipt object, got ${typeof payload}`,
      );
    }
    if (!("AnchorReceipt" in payload)) continue;
    try {
      receipts.push(wireReceiptToReceipt((payload as { AnchorReceipt: WireReceipt }).AnchorReceipt));
    } catch (err) {
      if (err instanceof MbongoReceiptError) {
        throw new MbongoReceiptError(
          `block.body.transactions[${i}].payload.AnchorReceipt.${err.field}`,
          err.message.slice(err.field.length + 2),
        );
      }
      throw err;
    }
  }
  return receipts;
}
