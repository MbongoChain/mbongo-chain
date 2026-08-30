/**
 * Integer validation for values that cross the wire as JSON numbers.
 *
 * ## Two tiers, deliberately
 *
 * `rpc_v0.2.md` (FROZEN) represents `Transaction.amount` (Rust `u128`) and
 * `nonce`, `height`, `timestamp` (Rust `u64`) as JSON numbers. Fields whose
 * domain fits inside `Number.MAX_SAFE_INTEGER` are exact as JavaScript
 * numbers; fields whose domain does not are carried as `bigint`.
 *
 * | tier | fields | validator |
 * |---|---|---|
 * | bounded `number` | `receipt.version`, byte elements, `error.code`, request id | {@link assertSafeUnsignedInteger} |
 * | exact `bigint` | `amount`, `nonce`, `height`, `timestamp` | {@link normalizeUnsignedInput} |
 *
 * ## Why an unsafe `number` is always refused
 *
 * JavaScript is integer-exact only through 2^53 − 1, and the rounding happens
 * when the literal is parsed — before this package sees the value. The
 * original cannot be recovered. So a `number` is accepted only while it is
 * still provably exact, and is never widened to `bigint` afterwards:
 * `BigInt(9007199254740993)` is `9007199254740992n`, which would launder a
 * value that was already wrong into one that merely looks precise.
 *
 * Callers who need the full domain pass a `bigint`, which was never rounded.
 *
 * ## What this is not
 *
 * These are **SDK rules**, not protocol rules. The node accepts the full Rust
 * domain and `rpc_v0.2.md` is unchanged. Nothing here narrows the protocol.
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

// ── Exact bigint tier ────────────────────────────────────────────────────

/** Largest `u64`: the domain of `nonce`, `height` and `timestamp`. */
export const U64_MAX = 18446744073709551615n;

/** Largest `u128`: the Rust domain of `Transaction.amount`. */
export const U128_MAX = 340282366920938463463374607431768211455n;

/**
 * Largest `amount` this SDK will accept.
 *
 * **This is an SDK end-to-end limit, not a protocol limit.** Rust's
 * `Transaction.amount` is a `u128` ({@link U128_MAX}) and the node accepts
 * that whole domain on submission. The cap exists on the *read* side: the
 * node serialises blocks for `get_block_by_height` through
 * `serde_json::to_value`, which fails with `number out of range` above
 * `u64::MAX`. An amount past that bound could be submitted and included, and
 * the block containing it would then be unreadable through that method.
 *
 * Accepting such a value would make this client complicit in producing a
 * block the chain cannot serve back, so it is refused here instead. Raising
 * this bound is a node-side change, tracked separately from #91.
 */
export const MAX_TRANSACTION_AMOUNT = U64_MAX;

/**
 * Normalises an unsigned integer input to `bigint`, or throws.
 *
 * A `number` is accepted only while it is still exact — non-negative and a
 * safe integer — and is then converted losslessly. A `bigint` is accepted
 * across `0n .. max`. An unsafe `number` is refused rather than widened: see
 * the module note on why that conversion cannot restore the intended value.
 *
 * @throws {MbongoNumericRangeError}
 */
export function normalizeUnsignedInput(
  field: string,
  value: number | bigint,
  max: bigint,
): bigint {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new MbongoNumericRangeError(field, value, "is negative");
    }
    if (value > max) {
      throw new MbongoNumericRangeError(
        field,
        value,
        `exceeds the maximum for this field (${max})`,
      );
    }
    return value;
  }

  // Reuses the number tier's rules, so "safe integer" means one thing in this
  // package rather than two subtly different things.
  assertSafeUnsignedInteger(field, value);
  const widened = BigInt(value);
  if (widened > max) {
    throw new MbongoNumericRangeError(
      field,
      value,
      `exceeds the maximum for this field (${max})`,
    );
  }
  return widened;
}

/** Normalises a `u64`-domain input (`nonce`, `height`, `timestamp`). */
export function normalizeU64(field: string, value: number | bigint): bigint {
  return normalizeUnsignedInput(field, value, U64_MAX);
}

/**
 * Normalises a `Transaction.amount` input.
 *
 * Bounded by {@link MAX_TRANSACTION_AMOUNT}, which is `u64::MAX` rather than
 * the Rust `u128` domain — for the read-path reason documented there.
 */
export function normalizeAmount(field: string, value: number | bigint): bigint {
  return normalizeUnsignedInput(field, value, MAX_TRANSACTION_AMOUNT);
}
