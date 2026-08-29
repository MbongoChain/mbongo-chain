/**
 * Integer safety for values that cross the wire as JSON numbers.
 *
 * ## The invariant
 *
 * Every integer this package represents as a JavaScript `number` and sends
 * or returns through RPC v0.2 must be a **non-negative safe integer**:
 * `Number.isSafeInteger(value) && value >= 0`.
 *
 * ## Why
 *
 * `rpc_v0.2.md` (FROZEN) represents `Transaction.amount` (Rust `u128`) and
 * `nonce`, `height`, `timestamp` (Rust `u64`) as JSON numbers. JavaScript is
 * integer-exact only through `Number.MAX_SAFE_INTEGER` (2^53 − 1), so a
 * larger value is rounded — and the rounding happens when the literal is
 * parsed, before this package ever sees it. The original value cannot be
 * recovered.
 *
 * What can be detected is that the value the SDK holds is not a safe
 * integer. So the SDK fails closed: it refuses to transmit such a value, and
 * refuses to hand one back as if it were trustworthy.
 *
 * ## What this is not
 *
 * This is an **SDK restriction**, not a protocol rule. The node accepts the
 * full Rust domain, and `rpc_v0.2.md` is unchanged. Nothing here narrows the
 * protocol; it narrows what this client is willing to vouch for.
 */

import { MbongoNumericRangeError } from "./errors.js";

/** Largest integer this package will send or return: 2^53 − 1. */
export const MAX_SAFE_RPC_INTEGER = Number.MAX_SAFE_INTEGER;

/**
 * Throws unless `value` is a non-negative safe integer.
 *
 * Rejects `NaN`, `Infinity`, `-Infinity`, negatives, fractions, and anything
 * from 2^53 upward. Never rounds, and never accepts a `bigint` for silent
 * conversion — converting one to `number` would reintroduce the very loss
 * this guard exists to prevent.
 *
 * @throws {MbongoNumericRangeError}
 */
export function assertSafeUnsignedInteger(
  field: string,
  value: unknown,
): asserts value is number {
  if (typeof value !== "number") {
    throw new MbongoNumericRangeError(
      field,
      value,
      `expected a number, got ${typeof value}`,
    );
  }
  if (Number.isNaN(value)) {
    throw new MbongoNumericRangeError(field, value, "is NaN");
  }
  if (!Number.isFinite(value)) {
    throw new MbongoNumericRangeError(field, value, "is not finite");
  }
  if (!Number.isInteger(value)) {
    throw new MbongoNumericRangeError(field, value, "is not an integer");
  }
  if (value < 0) {
    throw new MbongoNumericRangeError(field, value, "is negative");
  }
  if (!Number.isSafeInteger(value)) {
    throw new MbongoNumericRangeError(
      field,
      value,
      `exceeds the JavaScript safe-integer range (max ${MAX_SAFE_RPC_INTEGER}); ` +
        "the value may already have been rounded and cannot be trusted",
    );
  }
}

/** Validates the numeric fields of one transaction. */
export function assertSafeTransaction(path: string, tx: unknown): void {
  if (tx === null || typeof tx !== "object") {
    throw new MbongoNumericRangeError(path, tx, "expected a transaction object");
  }
  const t = tx as Record<string, unknown>;
  assertSafeUnsignedInteger(`${path}.amount`, t.amount);
  assertSafeUnsignedInteger(`${path}.nonce`, t.nonce);
}

/**
 * Validates every numeric field of a block, including the transactions in
 * its body.
 *
 * Block bodies are the reason this walks: a transaction arriving inside a
 * block is inbound data the caller never constructed, and its `amount` is
 * the widest field on the wire.
 */
export function assertSafeBlock(path: string, block: unknown): void {
  if (block === null || typeof block !== "object") {
    throw new MbongoNumericRangeError(path, block, "expected a block object");
  }
  const b = block as Record<string, unknown>;

  const header = b.header;
  if (header === null || typeof header !== "object") {
    throw new MbongoNumericRangeError(
      `${path}.header`,
      header,
      "expected a block header object",
    );
  }
  const h = header as Record<string, unknown>;
  assertSafeUnsignedInteger(`${path}.header.height`, h.height);
  assertSafeUnsignedInteger(`${path}.header.timestamp`, h.timestamp);

  const body = b.body;
  if (body === null || typeof body !== "object") {
    throw new MbongoNumericRangeError(
      `${path}.body`,
      body,
      "expected a block body object",
    );
  }
  const txs = (body as Record<string, unknown>).transactions;
  if (!Array.isArray(txs)) {
    throw new MbongoNumericRangeError(
      `${path}.body.transactions`,
      txs,
      "expected an array",
    );
  }
  txs.forEach((tx, i) =>
    assertSafeTransaction(`${path}.body.transactions[${i}]`, tx),
  );
}
