#!/usr/bin/env node
/**
 * Post-publish registry verification with bounded retry.
 *
 * Answers, from the registry and nothing else, whether `<package>@<version>`
 * is published with exactly the bytes that were packed, and whether the
 * registry serves a provenance attestation for those bytes. It never
 * publishes, never retries a publish, never reads a credential: every
 * request is an anonymous GET.
 *
 * Why it retries: the registry's read path can lag its write path by a few
 * seconds. The 0.2.0 release (run 34156693796) published at 20:50:02.5Z and
 * the single immediate `npm view` saw no such version, so a correct publish
 * was reported as a failure. A delay is not an outcome; only the registry's
 * eventual answer is. So each state below decides between "retry", "pass"
 * and "stop", and the whole thing is bounded.
 *
 *   version document absent (404)        → not visible yet: retry
 *   document present, `dist` incomplete  → not complete yet: retry
 *   transient transport / 5xx            → retry
 *   integrity present and equal          → PUBLISH_VERIFIED
 *   integrity present and different      → STOP: exists with other bytes; never retry, never republish
 *   document names another version       → STOP: wrong document; a registry bug or a wrong query
 *   schedule exhausted                   → STOP: VERIFY_TIMEOUT; the publish outcome is unknown, do not retry it
 *
 * Retry schedule (seconds between attempts): 5, 10, 20, 30, 30 — six attempts,
 * 95 s of waiting, plus the requests themselves. Bounded by construction.
 *
 * Usage (the release workflow):
 *   node scripts/verify-published.mjs --package @mbongo/sdk --version 0.2.0 \
 *        --tarball <path>            # expected integrity derived from the packed file
 *        [--publish-status <n>]      # exit status of the publish command, for the report
 *        [--provenance]              # also require a SLSA provenance attestation for the bytes
 *        [--registry https://registry.npmjs.org]
 *
 * The exported `verifyPublished` takes the same inputs plus injectable
 * `fetchImpl`, `sleepImpl` and `log`, which is how the tests drive every
 * registry state without a network or a real delay.
 */

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export const RETRY_DELAYS_SECS = [5, 10, 20, 30, 30];
export const DEFAULT_REGISTRY = "https://registry.npmjs.org";
export const SLSA_PROVENANCE_V1 = "https://slsa.dev/provenance/v1";

/** Both digests over the same raw tarball bytes (RELEASE.md §6.2). */
export function digestsOf(tarballBytes) {
  return {
    sri: "sha512-" + createHash("sha512").update(tarballBytes).digest("base64"),
    sha512hex: createHash("sha512").update(tarballBytes).digest("hex"),
    sha256hex: createHash("sha256").update(tarballBytes).digest("hex"),
  };
}

const encodeName = (name) => name.replace("/", "%2f");

/**
 * Verifies the publication. Resolves to a result object; never throws for
 * a registry answer, only for programming errors (bad arguments).
 *
 * Result shape: { state, attempts, elapsedMs, registryIntegrity, provenance }
 *   state ∈ PUBLISH_VERIFIED | INTEGRITY_MISMATCH | WRONG_VERSION | VERIFY_TIMEOUT
 *   provenance ∈ null | PROVENANCE_ATTESTATION_PRESENT_AND_SUBJECT_MATCHES
 *              | PROVENANCE_ATTESTATION_MISSING | PROVENANCE_PREDICATE_MISSING
 *              | PROVENANCE_SUBJECT_MISMATCH | PROVENANCE_MALFORMED_RESPONSE
 */
export async function verifyPublished({
  packageName,
  version,
  expectedIntegrity,
  expectedSha512Hex = null,
  requireProvenance = false,
  registry = DEFAULT_REGISTRY,
  delaysSecs = RETRY_DELAYS_SECS,
  fetchImpl = globalThis.fetch,
  sleepImpl = (ms) => new Promise((r) => setTimeout(r, ms)),
  now = () => Date.now(),
  log = (line) => process.stdout.write(line + "\n"),
}) {
  if (!packageName || !version || !expectedIntegrity) {
    throw new Error("verifyPublished: packageName, version and expectedIntegrity are required");
  }
  const base = registry.replace(/\/+$/, "");
  const versionUrl = `${base}/${encodeName(packageName)}/${version}`;
  const start = now();
  const elapsed = () => `${((now() - start) / 1000).toFixed(1)}s`;
  const outcome = (state, extra = {}) => ({ state, attempts, elapsedMs: now() - start, registryIntegrity, provenance: null, ...extra });

  let attempts = 0;
  let registryIntegrity = null;

  // ── phase 1: the version document ────────────────────────────────────
  for (let i = 0; ; i++) {
    attempts += 1;
    log(`[verify] attempt ${attempts}/${delaysSecs.length + 1} at ${elapsed()}: GET ${versionUrl}`);
    let status = null;
    let doc = null;
    let transport = null;
    try {
      const res = await fetchImpl(versionUrl, { headers: { accept: "application/json", "user-agent": "mbongo-release-verify" } });
      status = res.status;
      if (status === 200) {
        try { doc = await res.json(); } catch { doc = undefined; }
      }
    } catch (e) {
      transport = e && e.message ? e.message : String(e);
    }

    if (transport !== null) {
      log(`[verify]   transport error: ${transport} — retrying`);
    } else if (status === 404) {
      log(`[verify]   HTTP 404: version document not visible yet — retrying`);
    } else if (status !== 200) {
      log(`[verify]   HTTP ${status}: transient registry answer — retrying`);
    } else if (doc === undefined || doc === null || typeof doc !== "object") {
      log(`[verify]   HTTP 200 but the body is not a JSON document — retrying`);
    } else if (doc.version !== version || doc.name !== packageName) {
      log(`[verify]   HTTP 200 but the document is ${doc.name}@${doc.version}, not ${packageName}@${version} — stopping`);
      return outcome("WRONG_VERSION");
    } else if (!doc.dist || typeof doc.dist.integrity !== "string" || doc.dist.integrity.length === 0) {
      log(`[verify]   HTTP 200, metadata present, dist.integrity absent — not complete yet, retrying`);
    } else {
      registryIntegrity = doc.dist.integrity;
      log(`[verify]   HTTP 200, metadata present, dist.integrity present`);
      if (registryIntegrity !== expectedIntegrity) {
        log(`[verify]   expected ${expectedIntegrity}`);
        log(`[verify]   registry ${registryIntegrity}`);
        log(`[verify]   INTEGRITY_MISMATCH: ${packageName}@${version} exists with different bytes than were packed. Do not retry, do not republish.`);
        return outcome("INTEGRITY_MISMATCH");
      }
      log(`[verify]   integrity matches the packed tarball`);
      break;
    }

    if (i >= delaysSecs.length) {
      log(`[verify] VERIFY_TIMEOUT after ${attempts} attempts and ${elapsed()}: the registry never confirmed ${packageName}@${version}. The publish outcome is unknown — do not retry the publish; query the registry by hand (RELEASE.md §12.1).`);
      return outcome("VERIFY_TIMEOUT");
    }
    log(`[verify]   waiting ${delaysSecs[i]}s`);
    await sleepImpl(delaysSecs[i] * 1000);
  }

  // ── phase 2: provenance, observed not verified ─────────────────────────
  let provenance = null;
  if (requireProvenance) {
    const url = `${base}/-/npm/v1/attestations/${encodeName(packageName)}@${version}`;
    provenance = "PROVENANCE_ATTESTATION_MISSING";
    for (let i = 0; ; i++) {
      attempts += 1;
      log(`[verify] provenance attempt at ${elapsed()}: GET ${url}`);
      let res = null;
      try { res = await fetchImpl(url, { headers: { accept: "application/json", "user-agent": "mbongo-release-verify" } }); } catch { res = null; }
      if (res && res.status === 200) {
        let body;
        try { body = await res.json(); } catch { body = null; }
        if (!body || typeof body !== "object") {
          provenance = "PROVENANCE_MALFORMED_RESPONSE";
          break;
        }
        const slsa = (body.attestations || []).find((a) => a && a.predicateType === SLSA_PROVENANCE_V1);
        if (!slsa) { provenance = "PROVENANCE_PREDICATE_MISSING"; break; }
        let stmt;
        try { stmt = JSON.parse(Buffer.from(slsa.bundle.dsseEnvelope.payload, "base64").toString()); } catch { provenance = "PROVENANCE_MALFORMED_RESPONSE"; break; }
        const subject = (stmt.subject || [])[0];
        const expectedName = `pkg:npm/${packageName}@${version}`;
        const nameOk = subject && (subject.name === expectedName || subject.name === expectedName.replace("@", "%40"));
        const digestOk = subject && subject.digest && (expectedSha512Hex === null || subject.digest.sha512 === expectedSha512Hex);
        if (!nameOk || !digestOk) { provenance = "PROVENANCE_SUBJECT_MISMATCH"; break; }
        provenance = "PROVENANCE_ATTESTATION_PRESENT_AND_SUBJECT_MATCHES";
        log(`[verify]   ${provenance} (subject ${subject.name}; signature NOT verified here)`);
        break;
      }
      const why = res ? `HTTP ${res.status}` : "transport error";
      if (i >= delaysSecs.length) {
        log(`[verify]   ${why}: no attestation after ${attempts} attempts — PROVENANCE_ATTESTATION_MISSING`);
        break;
      }
      log(`[verify]   ${why}: attestation not visible yet — waiting ${delaysSecs[i]}s`);
      await sleepImpl(delaysSecs[i] * 1000);
    }
  }
  const result = outcome("PUBLISH_VERIFIED", { provenance });
  log(`[verify] PUBLISH_VERIFIED ${packageName}@${version} integrity=${registryIntegrity} attempts=${attempts} elapsed=${elapsed()}${requireProvenance ? ` provenance=${provenance}` : ""}`);
  return result;
}

// ── CLI ───────────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const out = { provenance: false, registry: DEFAULT_REGISTRY, timeoutNote: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => argv[++i];
    switch (a) {
      case "--package": out.packageName = next(); break;
      case "--version": out.version = next(); break;
      case "--tarball": out.tarball = next(); break;
      case "--expected-integrity": out.expectedIntegrity = next(); break;
      case "--publish-status": out.publishStatus = next(); break;
      case "--registry": out.registry = next(); break;
      case "--provenance": out.provenance = true; break;
      default: throw new Error(`unknown argument: ${a}`);
    }
  }
  return out;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  let expectedIntegrity = args.expectedIntegrity;
  let expectedSha512Hex = null;
  if (args.tarball) {
    const d = digestsOf(readFileSync(args.tarball));
    expectedIntegrity = d.sri;
    expectedSha512Hex = d.sha512hex;
    console.log(`[verify] expected integrity (from the packed tarball) ${expectedIntegrity}`);
  }
  if (args.publishStatus !== undefined) console.log(`[verify] publish command exit status: ${args.publishStatus}`);
  const result = await verifyPublished({
    packageName: args.packageName,
    version: args.version,
    expectedIntegrity,
    expectedSha512Hex,
    requireProvenance: args.provenance,
    registry: args.registry,
  });
  if (result.state !== "PUBLISH_VERIFIED") {
    console.log(`::error::${result.state}`);
    return 1;
  }
  if (args.provenance && result.provenance !== "PROVENANCE_ATTESTATION_PRESENT_AND_SUBJECT_MATCHES") {
    console.log(`::error::${result.provenance} — the package is published (do not republish); record and investigate`);
    return 1;
  }
  if (args.publishStatus !== undefined && args.publishStatus !== "0") {
    console.log(`::warning::the publish command exited ${args.publishStatus}, but the registry holds the exact artifact`);
  }
  return 0;
}

// The exit code is set rather than forced: process.exit() right after a
// fetch can abort while the connection is still closing (seen on Windows),
// and there is nothing left to wait for here.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  process.exitCode = await main();
}
