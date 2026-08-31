/**
 * Locating the npm CLI as a plain JavaScript file.
 *
 * The consumer smoke runs npm through `process.execPath` rather than by
 * spawning `npm`. Recent Node refuses to spawn `npm.cmd` without a shell, and
 * a shell is exactly what a packaging test should avoid: it reintroduces
 * quoting and `PATH` resolution, which is where paths with spaces go wrong.
 *
 * ## Why this is a separate module
 *
 * It is imported by `consumer-smoke.mjs` and by its test. The smoke runs work
 * at import time, so a test cannot import it to reach one function.
 *
 * ## The layouts
 *
 * Two are supported, because npm sits in a different place depending on how
 * Node was installed:
 *
 * | | |
 * |---|---|
 * | `<dir>/node_modules/npm/…` | Windows official installer — npm beside the binary |
 * | `<dir>/../lib/node_modules/npm/…` | the Unix prefix layout: `<prefix>/bin/node` with `<prefix>/lib/node_modules` |
 *
 * The second is what `actions/setup-node` produces on a Linux runner, and its
 * absence is the defect this module exists to fix: the release workflow
 * invokes the smoke directly with `node`, so `npm_execpath` is unset and the
 * fallback was the only route left. Ordinary CI never noticed, because it goes
 * through `npm run`, which sets `npm_execpath`.
 *
 * `PATH` is deliberately not searched. On Windows that finds `npm.cmd`, which
 * is the thing this whole approach exists to avoid.
 */

import { existsSync } from "node:fs";
import path from "node:path";

/**
 * The ordered candidate paths for the npm CLI entry point.
 *
 * Pure: it consults no filesystem and no ambient state, so it can be tested
 * against a layout that is not the one this process is running under.
 *
 * @param {string} execPath the Node binary, normally `process.execPath`
 * @param {Record<string, string | undefined>} env normally `process.env`
 * @returns {string[]} candidates, most authoritative first
 */
export function npmCliCandidates(execPath, env = {}) {
  const candidates = [];

  // Set by npm for anything it runs. When present it is authoritative: it
  // names the exact npm that invoked us, not merely one that exists.
  const fromEnv = env.npm_execpath;
  if (fromEnv && fromEnv.endsWith(".js")) {
    candidates.push(fromEnv);
  }

  const dir = path.dirname(execPath);
  candidates.push(path.join(dir, "node_modules", "npm", "bin", "npm-cli.js"));
  candidates.push(
    path.join(dir, "..", "lib", "node_modules", "npm", "bin", "npm-cli.js"),
  );

  return candidates;
}

/**
 * The first candidate that exists.
 *
 * @throws {Error} naming every path tried, so a future layout failure is
 * diagnosable from the log rather than by re-deriving the candidates.
 */
export function resolveNpmCli(
  execPath = process.execPath,
  env = process.env,
  exists = existsSync,
) {
  const candidates = npmCliCandidates(execPath, env);
  for (const candidate of candidates) {
    if (exists(candidate)) return candidate;
  }
  throw new Error(
    "could not locate the npm CLI entry point; tried:\n  " +
      candidates.join("\n  "),
  );
}
