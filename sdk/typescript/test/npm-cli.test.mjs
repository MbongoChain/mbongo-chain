/**
 * npm CLI resolution for the release path.
 *
 * These exist because of a real failure. GitHub Actions run 33349968293 —
 * the first execution of the release workflow under a real tag — failed at
 * "Consumer smoke against the canonical tarball" with
 * `could not locate the npm CLI entry point`, before anything was published.
 *
 * The cause was an invocation difference, not a packaging one:
 *
 * | | |
 * |---|---|
 * | ordinary CI | `npm run test:consumer` — npm sets `npm_execpath` |
 * | release workflow | `node scripts/consumer-smoke.mjs --tarball …` — it does not |
 *
 * With `npm_execpath` unset the only route left was a filesystem guess, and
 * that guess knew one layout: npm beside the Node binary, which is what the
 * Windows installer produces. `actions/setup-node` on Linux produces the Unix
 * prefix layout instead, so the guess missed and the release path failed while
 * every ordinary CI run stayed green.
 *
 * The layouts below are built as real directories rather than mocked, so the
 * test exercises the same `existsSync` the resolver uses.
 */

import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { npmCliCandidates, resolveNpmCli } from "../scripts/npm-cli.mjs";

const CLI = path.join("bin", "npm-cli.js");

/** Builds a throwaway Node installation layout and returns its fake binary. */
function layout(kind) {
  const root = mkdtempSync(path.join(tmpdir(), "mbongo-npmcli-"));
  const bin = path.join(root, "bin");
  mkdirSync(bin, { recursive: true });
  const execPath = path.join(bin, "node");
  writeFileSync(execPath, "");

  if (kind === "windows") {
    // npm beside the binary: <dir>/node_modules/npm/bin/npm-cli.js
    const npm = path.join(bin, "node_modules", "npm", "bin");
    mkdirSync(npm, { recursive: true });
    writeFileSync(path.join(npm, "npm-cli.js"), "");
  } else if (kind === "unix") {
    // Unix prefix: <prefix>/bin/node with <prefix>/lib/node_modules/npm
    const npm = path.join(root, "lib", "node_modules", "npm", "bin");
    mkdirSync(npm, { recursive: true });
    writeFileSync(path.join(npm, "npm-cli.js"), "");
  }
  // kind === "none": no npm anywhere

  return { root, execPath };
}

const cleanup = (l) => rmSync(l.root, { recursive: true, force: true });

test("the candidate list is exactly the layouts we support", () => {
  const candidates = npmCliCandidates(path.join("/prefix", "bin", "node"), {});
  assert.equal(candidates.length, 2, "expected 2 filesystem candidates with no npm_execpath");
  assert.ok(candidates[0].endsWith(path.join("bin", "node_modules", "npm", CLI)));
  assert.ok(candidates[1].endsWith(path.join("prefix", "lib", "node_modules", "npm", CLI)));

  const withEnv = npmCliCandidates(path.join("/prefix", "bin", "node"), {
    npm_execpath: "/somewhere/npm-cli.js",
  });
  assert.equal(withEnv.length, 3, "npm_execpath adds one candidate, ahead of the guesses");
  assert.equal(withEnv[0], "/somewhere/npm-cli.js");
});

test("a non-.js npm_execpath is ignored, not spawned", () => {
  // npm sets this to npm.cmd in some shells; spawning that without a shell is
  // exactly what this whole approach avoids.
  const candidates = npmCliCandidates(path.join("/prefix", "bin", "node"), {
    npm_execpath: "C:\\Program Files\\nodejs\\npm.cmd",
  });
  assert.equal(candidates.length, 2, "the .cmd must not become a candidate");
});

test("the Windows layout resolves — the case that always worked", () => {
  const l = layout("windows");
  try {
    const resolved = resolveNpmCli(l.execPath, {});
    assert.ok(resolved.startsWith(path.join(l.root, "bin", "node_modules")), resolved);
  } finally {
    cleanup(l);
  }
});

test("the Unix prefix layout resolves — the release-path regression", () => {
  // This is the case that failed on the Linux runner. Without the second
  // candidate this throws.
  const l = layout("unix");
  try {
    const resolved = resolveNpmCli(l.execPath, {});
    assert.ok(
      resolved.startsWith(path.join(l.root, "lib", "node_modules")),
      `expected the prefix layout, got ${resolved}`,
    );
  } finally {
    cleanup(l);
  }
});

test("npm_execpath still wins when it is present", () => {
  const l = layout("unix");
  try {
    const shim = path.join(l.root, "shim-cli.js");
    writeFileSync(shim, "");
    assert.equal(resolveNpmCli(l.execPath, { npm_execpath: shim }), shim);
  } finally {
    cleanup(l);
  }
});

test("no npm anywhere fails closed, naming every path tried", () => {
  const l = layout("none");
  try {
    let err;
    try {
      resolveNpmCli(l.execPath, {});
    } catch (e) {
      err = e;
    }
    assert.ok(err instanceof Error, "must throw rather than return a guess");
    assert.match(err.message, /could not locate the npm CLI entry point/);
    // Diagnosability: a future layout change should be readable from the log.
    assert.match(err.message, /node_modules[\\/]npm/);
    assert.equal(
      err.message.split("\n").length - 1,
      2,
      "the error should list both attempted paths",
    );
  } finally {
    cleanup(l);
  }
});
