// Post-publish registry verification, driven through every registry state
// with a scripted fetch and a zero-delay sleep. No network, no real delay,
// no publish: the module under test has no publish code path at all.

import { test } from "node:test";
import assert from "node:assert/strict";

import { verifyPublished, digestsOf, RETRY_DELAYS_SECS, SLSA_PROVENANCE_V1 } from "../scripts/verify-published.mjs";

const PKG = "@mbongo/sdk";
const VER = "0.2.0";
const BYTES = Buffer.from("not a real tarball, but bytes are bytes");
const D = digestsOf(BYTES);

const doc = (overrides = {}) => ({ name: PKG, version: VER, dist: { integrity: D.sri, tarball: "x" }, ...overrides });
const json = (status, body) => ({ status, json: async () => body });
const notJson = (status) => ({ status, json: async () => { throw new Error("no json"); } });

/** A fetch that answers the scripted responses in order and records every call. */
function scripted(responses) {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, method: (init && init.method) || "GET" });
    const next = responses.shift();
    if (next === undefined) throw new Error("scripted fetch exhausted");
    if (next instanceof Error) throw next;
    return next;
  };
  return { fetchImpl, calls };
}

function harness(responses, extra = {}) {
  const { fetchImpl, calls } = scripted(responses);
  const sleeps = [];
  const lines = [];
  let clock = 0;
  const run = () =>
    verifyPublished({
      packageName: PKG,
      version: VER,
      expectedIntegrity: D.sri,
      expectedSha512Hex: D.sha512hex,
      registry: "https://registry.example.test",
      fetchImpl,
      sleepImpl: async (ms) => { sleeps.push(ms); clock += ms; },
      now: () => clock,
      log: (l) => lines.push(l),
      ...extra,
    });
  return { run, calls, sleeps, lines };
}

test("the retry schedule is bounded: five waits, six attempts, 95 seconds", () => {
  assert.deepEqual(RETRY_DELAYS_SECS, [5, 10, 20, 30, 30]);
  assert.equal(RETRY_DELAYS_SECS.reduce((a, b) => a + b, 0), 95);
});

test("version immediately present with the packed integrity verifies on the first attempt", async () => {
  const h = harness([json(200, doc())]);
  const r = await h.run();
  assert.equal(r.state, "PUBLISH_VERIFIED");
  assert.equal(r.attempts, 1);
  assert.deepEqual(h.sleeps, []);
  assert.equal(r.registryIntegrity, D.sri);
});

test("version absent then appears: retries with the schedule and verifies", async () => {
  const h = harness([json(404, {}), json(404, {}), json(200, doc())]);
  const r = await h.run();
  assert.equal(r.state, "PUBLISH_VERIFIED");
  assert.equal(r.attempts, 3);
  assert.deepEqual(h.sleeps, [5000, 10000]);
  assert.ok(h.lines.some((l) => l.includes("attempt 1/6")));
  assert.ok(h.lines.some((l) => l.includes("not visible yet")));
});

test("version never appears: stops after the schedule with VERIFY_TIMEOUT and no publish", async () => {
  const h = harness([json(404, {}), json(404, {}), json(404, {}), json(404, {}), json(404, {}), json(404, {})]);
  const r = await h.run();
  assert.equal(r.state, "VERIFY_TIMEOUT");
  assert.equal(r.attempts, 6);
  assert.deepEqual(h.sleeps, [5000, 10000, 20000, 30000, 30000]);
  assert.ok(h.calls.every((c) => c.method === "GET"), "every registry call is a GET");
  assert.ok(h.lines.some((l) => l.includes("do not retry the publish")));
});

test("the registry answers with another version's document: stops, no retry", async () => {
  const h = harness([json(200, doc({ version: "0.1.0" }))]);
  const r = await h.run();
  assert.equal(r.state, "WRONG_VERSION");
  assert.equal(r.attempts, 1);
  assert.deepEqual(h.sleeps, []);
});

test("metadata present but integrity absent, then complete: retries until integrity arrives", async () => {
  const incomplete = doc({ dist: { tarball: "x" } });
  const h = harness([json(200, incomplete), json(200, incomplete), json(200, doc())]);
  const r = await h.run();
  assert.equal(r.state, "PUBLISH_VERIFIED");
  assert.equal(r.attempts, 3);
  assert.ok(h.lines.some((l) => l.includes("dist.integrity absent")));
});

test("integrity mismatch: stops immediately, never retries, never republishes", async () => {
  const other = digestsOf(Buffer.from("different bytes")).sri;
  const h = harness([json(200, doc({ dist: { integrity: other, tarball: "x" } })), json(200, doc())]);
  const r = await h.run();
  assert.equal(r.state, "INTEGRITY_MISMATCH");
  assert.equal(r.attempts, 1, "no further attempt after a mismatch");
  assert.equal(h.calls.length, 1);
  assert.ok(h.lines.some((l) => l.includes("do not republish")));
});

test("transient failures — a thrown fetch, a 503 and a non-JSON 200 — are retried", async () => {
  const h = harness([new Error("ECONNRESET"), json(503, {}), notJson(200), json(200, doc())]);
  const r = await h.run();
  assert.equal(r.state, "PUBLISH_VERIFIED");
  assert.equal(r.attempts, 4);
  assert.deepEqual(h.sleeps, [5000, 10000, 20000]);
});

test("provenance: attestation absent then present with the matching subject", async () => {
  const stmt = { subject: [{ name: `pkg:npm/${PKG}@${VER}`, digest: { sha512: D.sha512hex } }] };
  const bundle = { predicateType: SLSA_PROVENANCE_V1, bundle: { dsseEnvelope: { payload: Buffer.from(JSON.stringify(stmt)).toString("base64") } } };
  const h = harness([json(200, doc()), json(404, {}), json(200, { attestations: [bundle] })], { requireProvenance: true });
  const r = await h.run();
  assert.equal(r.state, "PUBLISH_VERIFIED");
  assert.equal(r.provenance, "PROVENANCE_ATTESTATION_PRESENT_AND_SUBJECT_MATCHES");
  assert.ok(h.calls[1].url.includes("/-/npm/v1/attestations/"));
});

test("provenance: a subject naming other bytes is a mismatch, and the package is still verified", async () => {
  const stmt = { subject: [{ name: `pkg:npm/${PKG}@${VER}`, digest: { sha512: "00".repeat(64) } }] };
  const bundle = { predicateType: SLSA_PROVENANCE_V1, bundle: { dsseEnvelope: { payload: Buffer.from(JSON.stringify(stmt)).toString("base64") } } };
  const h = harness([json(200, doc()), json(200, { attestations: [bundle] })], { requireProvenance: true });
  const r = await h.run();
  assert.equal(r.state, "PUBLISH_VERIFIED");
  assert.equal(r.provenance, "PROVENANCE_SUBJECT_MISMATCH");
});

test("provenance: an attestation without the SLSA predicate is reported as such", async () => {
  const h = harness([json(200, doc()), json(200, { attestations: [{ predicateType: "https://example.test/other" }] })], { requireProvenance: true });
  const r = await h.run();
  assert.equal(r.provenance, "PROVENANCE_PREDICATE_MISSING");
});

test("logs carry attempt number, elapsed time, the query and the verdict, and no credential", async () => {
  const h = harness([json(404, {}), json(200, doc())]);
  await h.run();
  const text = h.lines.join("\n");
  assert.match(text, /attempt 1\/6 at 0\.0s: GET https:\/\/registry\.example\.test\/@mbongo%2fsdk\/0\.2\.0/);
  assert.match(text, /attempt 2\/6 at 5\.0s/);
  assert.match(text, /PUBLISH_VERIFIED @mbongo\/sdk@0\.2\.0 integrity=sha512-/);
  assert.doesNotMatch(text, /authorization|npm_token|bearer/i);
});

test("digests are computed over the raw bytes in the two forms the runbook names", () => {
  assert.match(D.sri, /^sha512-[A-Za-z0-9+/]+=*$/);
  assert.equal(D.sha512hex.length, 128);
  assert.equal(D.sha256hex.length, 64);
  assert.equal(Buffer.from(D.sri.slice(7), "base64").toString("hex"), D.sha512hex);
});
