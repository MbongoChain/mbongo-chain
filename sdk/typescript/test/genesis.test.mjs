// TypeScript side of the shared genesis vectors
// (test-vectors/genesis/genesis-v1.json): the chain identifier of each valid
// file, and the committed dev genesis file. Rust reads the same fixture and
// owns the file's validation. The SDK has no genesis code yet; the RPC
// comparison of chain_id comes with S1-11.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { blake3 } from "@noble/hashes/blake3.js";

const G = JSON.parse(
  readFileSync(new URL("../../../test-vectors/genesis/genesis-v1.json", import.meta.url), "utf8"),
);
assert.equal(G.fixture_version, 1, "unsupported fixture version");

const unhex = (s) => Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));
const hex = (u8) => Array.from(u8, (b) => b.toString(16).padStart(2, "0")).join("");
const DOMAIN = new TextEncoder().encode("mbongo:chain-id:v1");

test("chain_id is BLAKE3 of the domain and the exact file bytes", () => {
  assert.equal(G.valid.length, 4, "expected 4 valid files");
  for (const v of G.valid) {
    const file = unhex(v.file_hex);
    assert.equal(hex(blake3(new Uint8Array([...DOMAIN, ...file]))), v.chain_id, v.name);
  }
});

test("the committed dev genesis file is the fixture's", () => {
  const committed = readFileSync(new URL("../../../crates/mbongo-core/genesis/dev.json", import.meta.url));
  const dev = G.valid.filter((v) => v.name === "dev");
  assert.equal(dev.length, 1);
  assert.equal(hex(committed), dev[0].file_hex);
  assert.ok(!committed.includes(0x0a) && !committed.includes(0x0d), "no line ending");
});
