/**
 * Exact-tier numeric validator tests.
 *
 * These cover the `bigint` tier: the validators that normalise `amount`,
 * `nonce`, `height` and `timestamp` inputs. The number tier
 * (`assertSafeUnsignedInteger`) keeps its own suite in `numeric.test.mjs`.
 *
 * The load-bearing case is the one at the bottom: an unsafe `number` must be
 * refused, never widened. `BigInt(9007199254740993)` is `9007199254740992n`,
 * so widening would turn a value that was already wrong into one that looks
 * precise.
 *
 * Every value above 2^53 - 1 is built from its decimal string and checked
 * against that string before use.
 */

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  MAX_TRANSACTION_AMOUNT,
  MbongoNumericRangeError,
  U128_MAX,
  U64_MAX,
  normalizeAmount,
  normalizeU64,
  normalizeUnsignedInput,
} from "../dist/index.js";

/** Builds a bigint from decimal digits, proving identity before any use. */
function exact(decimal) {
  const value = BigInt(decimal);
  assert.equal(value.toString(), decimal, "harness failure: intended value not preserved");
  return value;
}

const SAFE_MAX = 9007199254740991; // 2^53 - 1
const U64_MAX_TEXT = "18446744073709551615";
const U128_MAX_TEXT = "340282366920938463463374607431768211455";

test("the domain constants are the real type maxima", () => {
  assert.equal(U64_MAX.toString(), U64_MAX_TEXT);
  assert.equal(U128_MAX.toString(), U128_MAX_TEXT);
  assert.equal(U64_MAX, 2n ** 64n - 1n);
  assert.equal(U128_MAX, 2n ** 128n - 1n);
});

test("safe numbers are accepted and converted losslessly", () => {
  for (const value of [0, 1, 255, 4294967295, SAFE_MAX]) {
    const out = normalizeU64("v", value);
    assert.equal(typeof out, "bigint");
    assert.equal(out, BigInt(value));
    assert.equal(out.toString(), String(value));
  }
});

test("bigint inputs are accepted across the whole u64 domain", () => {
  for (const decimal of ["0", "1", "9007199254740992", "9007199254740993", U64_MAX_TEXT]) {
    const intended = exact(decimal);
    const out = normalizeU64("v", intended);
    assert.equal(out, intended);
    assert.equal(out.toString(), decimal);
  }
});

test("u64 overflow is rejected", () => {
  const overflow = exact("18446744073709551616"); // u64::MAX + 1
  assert.throws(() => normalizeU64("nonce", overflow), MbongoNumericRangeError);
  // The boundary itself is accepted, so the limit is exactly the limit.
  assert.equal(normalizeU64("nonce", exact(U64_MAX_TEXT)), U64_MAX);
});

test("u128 validator accepts its whole domain and rejects past it", () => {
  assert.equal(normalizeUnsignedInput("v", U128_MAX, U128_MAX), U128_MAX);
  assert.throws(
    () => normalizeUnsignedInput("v", U128_MAX + 1n, U128_MAX),
    MbongoNumericRangeError,
  );
});

test("negative inputs are rejected in both representations", () => {
  assert.throws(() => normalizeU64("v", -1n), MbongoNumericRangeError);
  assert.throws(() => normalizeU64("v", -1), MbongoNumericRangeError);
});

test("non-integer numbers are rejected", () => {
  for (const bad of [1.5, NaN, Infinity, -Infinity]) {
    assert.throws(() => normalizeU64("v", bad), MbongoNumericRangeError);
  }
});

test("an unsafe number is refused, never widened", () => {
  // Written as a literal on purpose: this is what a caller would type, and it
  // is ALREADY rounded before the SDK is entered. The point is that the SDK
  // must not launder it.
  const alreadyRounded = 9007199254740993;
  assert.equal(
    alreadyRounded,
    9007199254740992,
    "harness note: JavaScript rounded the literal, which is the premise",
  );

  assert.throws(
    () => normalizeU64("nonce", alreadyRounded),
    MbongoNumericRangeError,
    "an unsafe number must be rejected rather than converted",
  );

  // What laundering would have produced, asserted so the contrast is on record.
  assert.equal(BigInt(alreadyRounded).toString(), "9007199254740992");
});

test("2^53 is the first rejected number input", () => {
  assert.equal(normalizeU64("v", SAFE_MAX), 9007199254740991n);
  assert.throws(() => normalizeU64("v", SAFE_MAX + 1), MbongoNumericRangeError);
});

// ── amount: SDK end-to-end limit, not a protocol limit ───────────────────

test("amount is capped at u64::MAX, below the Rust u128 domain", () => {
  assert.equal(MAX_TRANSACTION_AMOUNT, U64_MAX);
  assert.ok(
    MAX_TRANSACTION_AMOUNT < U128_MAX,
    "the SDK cap is deliberately below the Rust domain",
  );

  assert.equal(normalizeAmount("amount", exact(U64_MAX_TEXT)), U64_MAX);

  const past = exact("18446744073709551616");
  let caught;
  try {
    normalizeAmount("amount", past);
  } catch (e) {
    caught = e;
  }
  assert.ok(caught instanceof MbongoNumericRangeError, "expected a range error");
  assert.ok(
    !/u128/i.test(String(caught.message)),
    "must not describe this as a u128 overflow: u128 would accept it",
  );
});

test("amount accepts ordinary safe numbers unchanged", () => {
  assert.equal(normalizeAmount("amount", 0), 0n);
  assert.equal(normalizeAmount("amount", 100), 100n);
});
