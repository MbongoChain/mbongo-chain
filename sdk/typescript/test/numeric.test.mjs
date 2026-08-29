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
        json: async () => ({ jsonrpc: "2.0", id: 1, result }),
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

test("the safe maximum still reaches the network", async () => {
  const { client, calls } = countingClient(block({ height: SAFE_MAX }));
  const got = await client.getBlockByHeight(SAFE_MAX);
  assert.equal(got.header.height, SAFE_MAX, "the boundary survives exactly");
  assert.equal(calls.n, 1);
});

// ── Inbound: never returned as trustworthy ───────────────────────────

test("an unsafe get_block_height result is not returned", async () => {
  const { client } = countingClient(TWO_POW_53);
  await assert.rejects(() => client.getBlockHeight(), MbongoNumericRangeError);
});

test("an unsafe block height is not returned", async () => {
  const { client } = countingClient(block({ height: TWO_POW_53 }));
  await assert.rejects(
    () => client.getBlockByHeight(0),
    (err) => {
      assert.ok(err instanceof MbongoNumericRangeError);
      assert.equal(err.field, "block.header.height");
      return true;
    },
  );
});

test("an unsafe block timestamp is not returned", async () => {
  const { client } = countingClient(block({ timestamp: TWO_POW_53 }));
  await assert.rejects(
    () => client.getBlockByHeight(0),
    (err) => {
      assert.equal(err.field, "block.header.timestamp");
      return true;
    },
  );
});

test("an unsafe amount inside a block transaction is not returned", async () => {
  const { client } = countingClient(
    block({}, [tx({ amount: TWO_POW_53 })]),
  );
  await assert.rejects(
    () => client.getBlockByHeight(0),
    (err) => {
      assert.equal(err.field, "block.body.transactions[0].amount");
      return true;
    },
  );
});

test("an unsafe nonce inside a block transaction is not returned", async () => {
  const { client } = countingClient(block({}, [tx(), tx({ nonce: TWO_POW_53 })]));
  await assert.rejects(
    () => client.getBlockByHeight(0),
    (err) => {
      assert.equal(
        err.field,
        "block.body.transactions[1].nonce",
        "every transaction in the body is walked, not just the first",
      );
      return true;
    },
  );
});

test("a well-formed block with safe values passes through untouched", async () => {
  const b = block({ height: 7, timestamp: 12345 }, [tx({ amount: SAFE_MAX })]);
  const { client } = countingClient(b);
  const got = await client.getBlockByHeight(7);
  assert.deepEqual(got, b);
});
