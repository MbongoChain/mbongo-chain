/**
 * Wire-contract tests for the baseline client.
 *
 * These assert the JSON actually put on the wire, not the return value of a
 * mocked method. A stubbed `fetch` captures each request body, so a test
 * fails if the client ever emits a wrong method string, wraps params it
 * should omit, or reintroduces a form the node does not accept.
 *
 * Run with `npm test`, which builds first and exercises `dist/` — the same
 * artifact a consumer installs.
 */

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  MbongoClient,
  MbongoRpcError,
  MbongoTransportError,
  RPC_METHODS,
} from "../dist/index.js";

const URL = "http://localhost:8080/rpc";

/** A `fetch` stub that records requests and replays canned responses. */
function stub(responses) {
  const sent = [];
  const sentRaw = [];
  const queue = Array.isArray(responses) ? [...responses] : [responses];
  const fetchImpl = async (_url, init) => {
    // The raw body matters as much as the parsed one now: exactness is a
    // property of the bytes on the wire, not of what JSON.parse recovers.
    sentRaw.push(init.body);
    sent.push(JSON.parse(init.body));
    const next = queue.length > 1 ? queue.shift() : queue[0];
    return {
      status: next.status ?? 200,
      text: async () => {
        if (next.text !== undefined) return next.text;
        if (next.raw !== undefined) return JSON.stringify(next.raw);
        return "not json at all";
      },
    };
  };
  return { sent, sentRaw, fetchImpl };
}

/** Wraps a JSON-RPC result, echoing the id the client chose. */
const ok = (result) => ({
  raw: { jsonrpc: "2.0", id: 1, result },
});

function clientWith(responses) {
  const s = stub(responses);
  return {
    client: new MbongoClient(URL, { fetch: s.fetchImpl }),
    sent: s.sent,
    sentRaw: s.sentRaw,
  };
}

// ── Method strings and canonical params ──────────────────────────────

test("ping sends the exact method string and no params", async () => {
  const { client, sent } = clientWith(ok("pong"));
  assert.equal(await client.ping(), "pong");
  assert.equal(sent[0].method, "ping");
  assert.equal(sent[0].jsonrpc, "2.0");
  assert.ok(!("params" in sent[0]), "a method taking none must omit params");
});

test("get_block_height sends the exact method string and no params", async () => {
  const { client, sent } = clientWith(ok(1234));
  assert.equal(await client.getBlockHeight(), 1234n, "heights come back as exact bigint");
  assert.equal(sent[0].method, "get_block_height");
  assert.ok(!("params" in sent[0]));
});

test("produce_block sends no params at all, and never max_txs", async () => {
  const { client, sent } = clientWith(ok("0xblockhash"));
  assert.equal(await client.produceBlock(), "0xblockhash");
  assert.equal(sent[0].method, "produce_block");
  assert.ok(!("params" in sent[0]), "produce_block is parameterless");
  assert.ok(
    !JSON.stringify(sent[0]).includes("max_txs"),
    "max_txs is not part of the RPC contract",
  );
});

test("get_latest_block_hash sends the exact method string and no params", async () => {
  const { client, sent } = clientWith(ok("0xtiphash"));
  assert.equal(await client.getLatestBlockHash(), "0xtiphash");
  assert.equal(sent[0].method, "get_latest_block_hash");
  assert.ok(!("params" in sent[0]));
});

test("get_block_by_height sends the canonical object, never a bare number", async () => {
  const block = {
    header: {
      parent_hash: "0x00",
      state_root: "0x00",
      transactions_root: "0x00",
      timestamp: 0,
      height: 5,
    },
    body: { transactions: [] },
  };
  const { client, sent } = clientWith(ok(block));
  const got = await client.getBlockByHeight(5);
  assert.equal(got.header.height, 5n, "the nested block shape must survive");
  assert.deepEqual(got.body.transactions, []);
  assert.equal(sent[0].method, "get_block_by_height");
  assert.deepEqual(
    sent[0].params,
    { height: 5 },
    "the runtime also tolerates a bare number; the SDK must not rely on that",
  );
  assert.equal(
    typeof sent[0].params,
    "object",
    "params must never be a bare numeric",
  );
});

test("submit_transaction sends the structured object, not the historical hex form", async () => {
  const tx = {
    tx_type: "Transfer",
    sender: "0xe734ea6c2b6257de72355e472aa05a4c487e6b463c029ed306df2f01b5636b58",
    receiver: "0x2222222222222222222222222222222222222222222222222222222222222222",
    amount: 100,
    nonce: 0,
    payload: "None",
    signature:
      "0x1c37e5d2236bba0eb9017ca49cf67ead73a8e30fa7a5afa982aeedb3c4b20485c9031e974dad586e9e4e9134d22ef003541018101c877867170fd568984cee0a",
  };
  const { client, sent } = clientWith(ok("0xtxhash"));
  assert.equal(await client.submitTransaction(tx), "0xtxhash");
  assert.equal(sent[0].method, "submit_transaction");
  assert.deepEqual(sent[0].params, tx, "the transaction is sent verbatim");
  assert.ok(
    !Array.isArray(sent[0].params),
    "the historical [signed_tx_hex] array form is not the v0.2 contract",
  );
  assert.equal(typeof sent[0].params, "object");
});

// ── Request ids ──────────────────────────────────────────────────────

test("request ids increment per call and are always present", async () => {
  const { client, sent } = clientWith(ok("pong"));
  await client.ping();
  await client.ping();
  await client.ping();
  assert.deepEqual(
    sent.map((r) => r.id),
    [1, 2, 3],
  );
});

// ── Errors ───────────────────────────────────────────────────────────

test("a JSON-RPC error object becomes MbongoRpcError with code preserved", async () => {
  const { client } = clientWith({
    status: 404,
    raw: {
      jsonrpc: "2.0",
      id: 1,
      error: { code: -32601, message: "Method not found: nope" },
    },
  });
  await assert.rejects(
    () => client.ping(),
    (err) => {
      assert.ok(err instanceof MbongoRpcError);
      assert.equal(err.code, -32601);
      assert.equal(
        err.isMethodUnavailable,
        true,
        "-32601 means the method is unavailable",
      );
      assert.notEqual(
        err.isInvalidParams,
        true,
        "-32601 must not be read as anything else",
      );
      return true;
    },
  );
});

test("-32601 is not translated into a missing resource", async () => {
  // A reserved compute method is unavailable, not absent data. If the SDK
  // ever resolves to null or undefined here instead of raising, a caller
  // could mistake unavailability for an empty result.
  const { client } = clientWith({
    status: 404,
    raw: {
      jsonrpc: "2.0",
      id: 1,
      error: { code: -32601, message: "Method not found: get_compute_receipt" },
    },
  });
  await assert.rejects(() => client.getBlockByHeight(1), MbongoRpcError);
});

test("error data is preserved when present", async () => {
  const { client } = clientWith({
    status: 400,
    raw: {
      jsonrpc: "2.0",
      id: 1,
      error: { code: -32602, message: "invalid transaction", data: { at: 0 } },
    },
  });
  await assert.rejects(
    () => client.getBlockHeight(),
    (err) => {
      assert.equal(err.code, -32602);
      assert.equal(err.isInvalidParams, true);
      // error.data is arbitrary server JSON, so its integers are preserved
      // exactly rather than coerced to number: an unknown field could be
      // outside the safe range, and rounding it would be the bug this
      // package exists to avoid.
      assert.deepEqual(err.data, { at: 0n });
      return true;
    },
  );
});

test("a non-JSON body is a transport error, not an RPC error", async () => {
  const { client } = clientWith({ status: 500 });
  await assert.rejects(() => client.ping(), MbongoTransportError);
});

test("a body that is not JSON-RPC 2.0 is a transport error", async () => {
  const { client } = clientWith({ raw: { hello: "world" } });
  await assert.rejects(() => client.ping(), MbongoTransportError);
});

test("a response with neither result nor error is a transport error", async () => {
  const { client } = clientWith({ raw: { jsonrpc: "2.0", id: 1 } });
  await assert.rejects(() => client.ping(), MbongoTransportError);
});

test("a failed fetch is a transport error", async () => {
  const client = new MbongoClient(URL, {
    fetch: async () => {
      throw new Error("ECONNREFUSED");
    },
  });
  await assert.rejects(() => client.ping(), MbongoTransportError);
});

// ── No stale surface ─────────────────────────────────────────────────

test("the exported method map contains exactly the six v0.2 methods", () => {
  assert.deepEqual(Object.values(RPC_METHODS).sort(), [
    "get_block_by_height",
    "get_block_height",
    "get_latest_block_hash",
    "ping",
    "produce_block",
    "submit_transaction",
  ]);
});

test("no mbg_ method is reachable from the client", () => {
  const stale = [
    "getAccount",
    "getBlockByNumber",
    "getBlockNumber",
    "getTransaction",
    "getValidatorSet",
    "sendTransaction",
  ];
  const client = new MbongoClient(URL);
  for (const name of stale) {
    assert.equal(
      typeof client[name],
      "undefined",
      `${name} targeted a method the node never served`,
    );
  }
});

// ── Exact integers on the request path ───────────────────────────────
//
// The response direction is covered in numeric.test.mjs. These assert the
// other half: what leaves this client must carry every digit the caller
// meant, as an unquoted JSON number.

/** Builds a bigint from digits, proving identity before any use. */
function exactValue(decimal) {
  const value = BigInt(decimal);
  assert.equal(value.toString(), decimal, "harness failure: intended value not preserved");
  return value;
}

test("getBlockByHeight sends a bigint height as exact unquoted digits", async () => {
  const decimal = "9007199254740993";
  const intended = exactValue(decimal);
  const { client, sentRaw } = clientWith(ok("0xhash"));

  await assert.rejects(() => client.getBlockByHeight(intended)); // result is not a block
  assert.ok(
    sentRaw[0].includes(`"height":${decimal}`),
    `request must carry the exact digits, got: ${sentRaw[0]}`,
  );
  assert.ok(!sentRaw[0].includes("9007199254740992"), "must not be rounded");
  assert.ok(!sentRaw[0].includes(`"height":"`), "must not be quoted");
});

test("submitTransaction sends exact amount and nonce as JSON numbers", async () => {
  const nonce = "18446744073709551615"; // u64::MAX
  const amount = "9007199254740993";
  const { client, sentRaw, sent } = clientWith(ok("0xhash"));

  await client.submitTransaction({
    tx_type: "Transfer",
    sender: "0x11",
    receiver: "0x22",
    amount: exactValue(amount),
    nonce: exactValue(nonce),
    payload: "None",
    signature: "0x00",
  });

  const body = sentRaw[0];
  assert.ok(body.includes(`"amount":${amount}`), `amount digits: ${body}`);
  assert.ok(body.includes(`"nonce":${nonce}`), `nonce digits: ${body}`);
  assert.ok(!body.includes('"amount":"'), "amount must not be quoted");
  assert.ok(!body.includes('"nonce":"'), "nonce must not be quoted");
  // The envelope is unchanged: same method string, id still a number.
  assert.equal(sent[0].method, "submit_transaction");
  assert.equal(typeof sent[0].id, "number");
});

test("a safe number input still works and is sent identically to its bigint", async () => {
  const a = clientWith(ok("0xhash"));
  const b = clientWith(ok("0xhash"));
  const base = {
    tx_type: "Transfer",
    sender: "0x11",
    receiver: "0x22",
    payload: "None",
    signature: "0x00",
  };

  await a.client.submitTransaction({ ...base, amount: 100, nonce: 0 });
  await b.client.submitTransaction({ ...base, amount: 100n, nonce: 0n });

  assert.equal(a.sentRaw[0], b.sentRaw[0], "number and bigint inputs must serialise alike");
  assert.ok(a.sentRaw[0].includes('"amount":100'));
  assert.ok(a.sentRaw[0].includes('"nonce":0'));
});

test("an amount above u64::MAX is refused before transmission", async () => {
  const past = exactValue("18446744073709551616"); // u64::MAX + 1
  const { client, sentRaw } = clientWith(ok("0xhash"));

  await assert.rejects(
    () =>
      client.submitTransaction({
        tx_type: "Transfer",
        sender: "0x11",
        receiver: "0x22",
        amount: past,
        nonce: 0n,
        payload: "None",
        signature: "0x00",
      }),
    (err) => {
      assert.equal(err.name, "MbongoNumericRangeError");
      // SCALE could encode this as a u128; the limit is the node's block
      // read path, so the message must not blame u128.
      assert.ok(!/u128/i.test(err.message), "must not be reported as a u128 overflow");
      return true;
    },
  );
  assert.equal(sentRaw.length, 0, "nothing may be transmitted");
});

test("u64::MAX is accepted for nonce and height", async () => {
  const m = "18446744073709551615";
  const { client, sentRaw } = clientWith(ok("0xhash"));
  await client.submitTransaction({
    tx_type: "Transfer",
    sender: "0x11",
    receiver: "0x22",
    amount: 0n,
    nonce: exactValue(m),
    payload: "None",
    signature: "0x00",
  });
  assert.ok(sentRaw[0].includes(`"nonce":${m}`));
});
