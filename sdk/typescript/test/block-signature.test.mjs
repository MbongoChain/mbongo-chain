// TypeScript side of the shared block-signature vectors
// (test-vectors/block/block-signature-v1.json, RFC 0006 section 3). Rust reads
// the same file.
//
// The SDK verifies no block: the node does (rules t, u, v). What a client
// needs is the block shape, which getBlockByHeight decodes, and one strict
// Ed25519 definition shared with the node. So this file checks, for every
// case, the header and block layout and the message a producer signs,
// rebuilt here from the case's fields, and the strict verdict through the
// SDK's verifier; and that a block result carrying the new fields decodes.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { blake3 } from "@noble/hashes/blake3.js";

import { MbongoClient, computeTasksInBlock } from "../dist/index.js";
import { verifyEd25519Strict } from "../dist/ed25519-strict.js";

const load = (rel) => JSON.parse(readFileSync(new URL(rel, import.meta.url), "utf8"));
const BX = load("../../../test-vectors/block/block-signature-v1.json");
const RPC = load("../../../test-vectors/rpc/compute-task-rpc-v3.json");
assert.equal(BX.fixture_version, 1, "block-signature fixture: unsupported schema version");
assert.equal(RPC.fixture_version, 3, "rpc fixture: unsupported schema version");

const unhex = (s) => {
  assert.ok(/^[0-9a-f]*$/.test(s) && s.length % 2 === 0, "lowercase even-length hex");
  return Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));
};
const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, "0")).join("");
const cat = (...parts) => Uint8Array.from(parts.flatMap((p) => [...p]));
const u64le = (decimal) => {
  let v = BigInt(decimal);
  const out = new Uint8Array(8);
  for (let i = 0; i < 8; i++) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  assert.equal(v, 0n, "fits in u64");
  return out;
};
const DOMAIN = new TextEncoder().encode("mbongo-block:v1");
/** The SDK verifier takes (signature, message, publicKey). */
const strict = (publicKey, message, signature) => verifyEd25519Strict(signature, message, publicKey);

test("the fixture holds the cases it states", () => {
  assert.equal(BX.cases.length, 24);
  const count = (field, value) => BX.cases.filter((c) => c[field] === value).length;
  assert.equal(count("expected", "valid"), 6);
  assert.equal(count("expected", "invalid"), 18);
  const outcomes = Object.fromEntries(BX.apply_block_outcomes.map((o) => [o, count("apply_block", o)]));
  assert.deepEqual(outcomes, {
    accepted: 3,
    BadParent: 1,
    UnauthorizedProducer: 4,
    InvalidBlockSignature: 14,
    InvalidTimestamp: 2,
  });
  assert.equal(hex(DOMAIN), BX.domain.hex);
});

test("every header, signing hash, block hash and strict verdict is the file's", () => {
  let checked = 0;
  for (const c of BX.cases) {
    const chainId = unhex(BX.chains[c.chain].chain_id);
    const h = c.header;
    // The header layout of RFC 0006 section 3, producer appended last.
    const header = cat(
      unhex(h.parent_hash),
      unhex(h.state_root),
      unhex(h.transactions_root),
      u64le(h.timestamp),
      u64le(h.height),
      unhex(h.producer),
    );
    assert.equal(header.length, 144, c.name);
    assert.equal(hex(header), c.header_encoding, `${c.name}: header encoding`);
    const signingHash = blake3(cat(DOMAIN, chainId, header));
    assert.equal(hex(signingHash), c.signing_hash, `${c.name}: signing hash`);
    // The block: header, an empty body (compact 0), the signature last. Its
    // hash covers the signature.
    const block = cat(header, [0x00], unhex(c.producer_signature));
    assert.equal(hex(block), c.block_encoding, `${c.name}: block encoding`);
    assert.equal(hex(blake3(block)), c.block_hash, `${c.name}: block hash`);
    // The strict definition the node applies under rule (u).
    const verdict = strict(unhex(h.producer), signingHash, unhex(c.producer_signature));
    assert.equal(verdict ? "valid" : "invalid", c.expected, `${c.name}: strict verdict`);
    checked++;
  }
  assert.equal(checked, 24);
});

test("a signature for one network does not verify on another with the same authority", () => {
  const valid = BX.cases.find((c) => c.name === "valid-dev");
  const replayed = BX.cases.find((c) => c.name === "cross-chain-signature");
  assert.deepEqual(valid.header, replayed.header);
  assert.equal(BX.chains.dev.authority, BX.chains.other.authority);
  const header = unhex(valid.header_encoding);
  const messageFor = (chain) => blake3(cat(DOMAIN, unhex(BX.chains[chain].chain_id), header));
  const key = unhex(valid.header.producer);
  assert.ok(strict(key, messageFor("dev"), unhex(valid.producer_signature)));
  assert.ok(!strict(key, messageFor("other"), unhex(valid.producer_signature)));
  assert.ok(strict(key, messageFor("other"), unhex(replayed.producer_signature)));
  assert.ok(!strict(key, messageFor("dev"), unhex(replayed.producer_signature)));
});

test("a block result with producer and producer_signature decodes, and its tasks are read", async () => {
  const pinned = RPC.block.object;
  const client = new MbongoClient("http://localhost:8080/rpc", {
    fetch: async () => ({
      status: 200,
      text: async () => JSON.stringify({ jsonrpc: "2.0", id: 1, result: pinned }),
    }),
  });
  const block = await client.getBlockByHeight(1);
  assert.equal(block.header.producer, "0x" + RPC.block.expected.producer);
  assert.equal(block.producer_signature, "0x" + RPC.block.expected.producer_signature);
  assert.equal(block.body.transactions.length, 3);
  // The signature in the pinned result is the dev authority's over its header.
  const h = pinned.header;
  const header = cat(
    unhex(h.parent_hash.slice(2)),
    unhex(h.state_root.slice(2)),
    unhex(h.transactions_root.slice(2)),
    u64le(h.timestamp),
    u64le(h.height),
    unhex(h.producer.slice(2)),
  );
  assert.equal(hex(header), RPC.block.expected.header_encoding);
  const signingHash = blake3(cat(DOMAIN, unhex(RPC.chain.chain_id), header));
  assert.equal(hex(signingHash), RPC.block.expected.signing_hash);
  assert.ok(
    strict(unhex(h.producer.slice(2)), signingHash, unhex(pinned.producer_signature.slice(2))),
  );
  // Reading tasks needs only the body: the new fields change nothing there.
  assert.equal(computeTasksInBlock(pinned).length, 1);
  assert.equal(computeTasksInBlock(block).length, 1);
});
