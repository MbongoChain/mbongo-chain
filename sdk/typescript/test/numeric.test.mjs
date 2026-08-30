/**
 * Numeric safety tests.
 *
 * The SDK represents `u128` and `u64` wire fields as JavaScript numbers,
 * which are integer-exact only to 2^53 − 1. These prove the SDK refuses to
 * transmit or return a value it cannot vouch for, and that the refusal
 * happens before any network call on the outbound path.
 *
 * They assert an SDK restriction, not a protocol rule: the node accepts the
 * full Rust domain and `rpc_v0.2.md` is unchanged.
 */

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  MAX_SAFE_RPC_INTEGER,
  MbongoClient,
  MbongoNumericRangeError,
  assertSafeUnsignedInteger,
} from "../dist/index.js";

const URL = "http://localhost:8080/rpc";
const SAFE_MAX = 9007199254740991; // 2^53 - 1
const TWO_POW_53 = 9007199254740992; // 2^53, first unsafe

/** A client whose fetch counts calls, so "rejected before the network" is provable. */
function countingClient(result) {
  const calls = { n: 0 };
  const client = new MbongoClient(URL, {
    fetch: async () => {
      calls.n += 1;
      return {
        status: 200,
        text: async () => JSON.stringify({ jsonrpc: "2.0", id: 1, result }),
      };
    },
  });
  return { client, calls };
}

function tx(overrides) {
  return {
    tx_type: "Transfer",
    sender: "0x11",
    receiver: "0x22",
    amount: 100,
    nonce: 0,
    payload: "None",
    signature: "0x00",
    ...overrides,
  };
}

function block(overrides = {}, txs = []) {
  return {
    header: {
      parent_hash: "0x00",
      state_root: "0x00",
      transactions_root: "0x00",
      timestamp: 0,
      height: 0,
      ...overrides,
    },
    body: { transactions: txs },
  };
}

// ── The primitive ────────────────────────────────────────────────────

test("the safe boundary is accepted exactly", () => {
  assert.equal(MAX_SAFE_RPC_INTEGER, SAFE_MAX);
  assert.doesNotThrow(() => assertSafeUnsignedInteger("f", SAFE_MAX));
  assert.doesNotThrow(() => assertSafeUnsignedInteger("f", 0));
});

test("2^53 and beyond are rejected", () => {
  for (const v of [TWO_POW_53, TWO_POW_53 + 2, Number.MAX_VALUE]) {
    assert.throws(
      () => assertSafeUnsignedInteger("f", v),
      MbongoNumericRangeError,
      `${v} must be rejected`,
    );
  }
});

test("the 2^53+1 literal is already rounded, and the rounded value is rejected", () => {
  // The corruption happens when JavaScript parses the literal, before the
  // SDK sees anything. The intent cannot be recovered — but the value the
  // SDK holds is detectably unsafe, and that is what it refuses.
  const literal = 9007199254740993;
  assert.equal(literal, TWO_POW_53, "JavaScript rounds 2^53+1 down to 2^53");
  assert.throws(
    () => assertSafeUnsignedInteger("f", literal),
    MbongoNumericRangeError,
  );
});

test("negative, fractional, NaN and Infinity are rejected", () => {
  for (const v of [-1, -0.5, 1.5, NaN, Infinity, -Infinity]) {
    assert.throws(
      () => assertSafeUnsignedInteger("f", v),
      MbongoNumericRangeError,
      `${String(v)} must be rejected`,
    );
  }
});

test("a bigint is rejected rather than silently converted", () => {
  assert.throws(
    () => assertSafeUnsignedInteger("f", 1n),
    MbongoNumericRangeError,
  );
});

test("the error names the offending field", () => {
  try {
    assertSafeUnsignedInteger("transaction.amount", TWO_POW_53);
    assert.fail("expected a throw");
  } catch (err) {
    assert.ok(err instanceof MbongoNumericRangeError);
    assert.equal(err.field, "transaction.amount");
    assert.equal(err.value, TWO_POW_53);
    assert.match(err.message, /transaction\.amount/);
  }
});

// ── Outbound: rejected before the network ────────────────────────────

test("submitTransaction rejects an unsafe amount without calling the network", async () => {
  const { client, calls } = countingClient("0xhash");
  await assert.rejects(
    () => client.submitTransaction(tx({ amount: TWO_POW_53 })),
    MbongoNumericRangeError,
  );
  assert.equal(calls.n, 0, "no request may be issued");
});

test("submitTransaction rejects an unsafe nonce without calling the network", async () => {
  const { client, calls } = countingClient("0xhash");
  await assert.rejects(
    () => client.submitTransaction(tx({ nonce: TWO_POW_53 })),
    MbongoNumericRangeError,
  );
  assert.equal(calls.n, 0);
});

test("getBlockByHeight rejects an unsafe height without calling the network", async () => {
  const { client, calls } = countingClient(block());
  await assert.rejects(
    () => client.getBlockByHeight(TWO_POW_53),
    MbongoNumericRangeError,
  );
  assert.equal(calls.n, 0);
});

// ── Inbound: returned exactly, not refused ───────────────────────────
//
// This is what issue #91 changed. The SDK used to reject any inbound value
// above 2^53 - 1, because `response.json()` had already rounded it and the
// original could not be recovered. The client no longer parses that way, so
// these values arrive intact and are returned as exact bigint.

/** Serves a raw JSON-RPC body, so response digits are under test control. */
function rawClient(resultJson) {
  const calls = { n: 0 };
  const client = new MbongoClient(URL, {
    fetch: async () => {
      calls.n += 1;
      return {
        status: 200,
        text: async () => `{"jsonrpc":"2.0","id":1,"result":${resultJson}}`,
      };
    },
  });
  return { client, calls };
}

/** Builds a bigint from digits, proving identity before any use. */
function exact(decimal) {
  const value = BigInt(decimal);
  assert.equal(value.toString(), decimal, "harness failure: intended value not preserved");
  return value;
}

const U64_MAX_TEXT = "18446744073709551615";
const TWO_POW_53_PLUS_1_TEXT = "9007199254740993";

test("the safe maximum still reaches the network, now as bigint", async () => {
  const { client, calls } = rawClient(
    `{"header":{"parent_hash":"0x00","state_root":"0x00","transactions_root":"0x00","timestamp":0,"height":${SAFE_MAX}},"body":{"transactions":[]}}`,
  );
  const got = await client.getBlockByHeight(SAFE_MAX);
  assert.equal(got.header.height, BigInt(SAFE_MAX), "the boundary survives exactly");
  assert.equal(calls.n, 1);
});

test("get_block_height returns values above 2^53 exactly", async () => {
  for (const decimal of [TWO_POW_53_PLUS_1_TEXT, U64_MAX_TEXT]) {
    const intended = exact(decimal);
    const { client } = rawClient(decimal);
    const height = await client.getBlockHeight();
    assert.equal(typeof height, "bigint");
    assert.equal(height, intended);
    assert.equal(height.toString(), decimal, `${decimal} must survive intact`);
  }
});

test("2^53 and 2^53+1 do not alias through get_block_height", async () => {
  const a = await rawClient("9007199254740992").client.getBlockHeight();
  const b = await rawClient("9007199254740993").client.getBlockHeight();
  assert.notEqual(a, b, "distinct heights must stay distinct");
  assert.equal(a.toString(), "9007199254740992");
  assert.equal(b.toString(), "9007199254740993");
});

test("block header and transaction integers all arrive exactly", async () => {
  const height = "9007199254740993";
  const timestamp = "9007199254740995";
  const nonce = "9007199254740997";
  const amount = "9007199254740999";
  const { client } = rawClient(
    `{"header":{"parent_hash":"0x00","state_root":"0x00","transactions_root":"0x00",` +
      `"timestamp":${timestamp},"height":${height}},"body":{"transactions":[` +
      `{"tx_type":"Transfer","sender":"0x11","receiver":"0x22","amount":${amount},` +
      `"nonce":${nonce},"payload":"None","signature":"0x00"}]}}`,
  );

  const got = await client.getBlockByHeight(0);
  assert.equal(got.header.height.toString(), height);
  assert.equal(got.header.timestamp.toString(), timestamp);
  assert.equal(got.body.transactions[0].amount.toString(), amount);
  assert.equal(got.body.transactions[0].nonce.toString(), nonce);
  // None of the four may alias onto the same rounded double.
  const seen = new Set([height, timestamp, nonce, amount]);
  assert.equal(seen.size, 4, "the four values are distinct by construction");
});

test("u64::MAX arrives exactly in every u64 field", async () => {
  const m = U64_MAX_TEXT;
  const { client } = rawClient(
    `{"header":{"parent_hash":"0x00","state_root":"0x00","transactions_root":"0x00",` +
      `"timestamp":${m},"height":${m}},"body":{"transactions":[` +
      `{"tx_type":"Transfer","sender":"0x11","receiver":"0x22","amount":${m},` +
      `"nonce":${m},"payload":"None","signature":"0x00"}]}}`,
  );
  const got = await client.getBlockByHeight(0);
  const intended = exact(m);
  assert.equal(got.header.height, intended);
  assert.equal(got.header.timestamp, intended);
  assert.equal(got.body.transactions[0].amount, intended);
  assert.equal(got.body.transactions[0].nonce, intended);
});

test("a well-formed block with small values still passes through", async () => {
  const { client } = rawClient(
    `{"header":{"parent_hash":"0x00","state_root":"0x00","transactions_root":"0x00","timestamp":12345,"height":7},` +
      `"body":{"transactions":[{"tx_type":"Transfer","sender":"0x11","receiver":"0x22","amount":100,"nonce":0,"payload":"None","signature":"0x00"}]}}`,
  );
  const got = await client.getBlockByHeight(7);
  assert.deepEqual(got, {
    header: {
      parent_hash: "0x00",
      state_root: "0x00",
      transactions_root: "0x00",
      timestamp: 12345n,
      height: 7n,
    },
    body: {
      transactions: [
        {
          tx_type: "Transfer",
          sender: "0x11",
          receiver: "0x22",
          amount: 100n,
          nonce: 0n,
          payload: "None",
          signature: "0x00",
        },
      ],
    },
  });
});
