#!/usr/bin/env node
/**
 * Packed consumer smoke test.
 *
 * Proves that the artifact `npm pack` produces can actually be installed and
 * used by an outside project. Everything here runs against the tarball: the
 * consumer lives outside the repository, installs `@mbongo/sdk` from the
 * `.tgz` by absolute path, and imports it by package name only. Nothing is
 * published, and no Mbongo node is contacted.
 *
 * What the repository test suite cannot tell you, and this can:
 *   - `files` and the `exports` map describe a package Node can resolve
 *   - the declarations survive packing, so a TypeScript consumer typechecks
 *   - LICENSE is really inside the tarball, not merely beside it in git
 *
 * Subprocesses are spawned with `shell: false` and argv arrays, never through
 * a shell, so paths containing spaces or a Windows drive letter are passed
 * through verbatim rather than re-parsed.
 */

import { spawnSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const PKG_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = path.resolve(PKG_DIR, "..", "..");
const EXPECTED_NAME = "@mbongo/sdk";
const EXPECTED_VERSION = "0.1.0";

let failed = 0;

function check(label, ok, detail) {
  if (ok) {
    console.log("  ok   " + label);
  } else {
    failed++;
    console.log("  FAIL " + label + (detail ? " -- " + detail : ""));
  }
}

/**
 * The npm CLI as a plain JavaScript file, so it can be run through
 * `process.execPath`. Spawning `npm.cmd` directly is refused by recent Node
 * versions unless a shell is used, and a shell is exactly what this test is
 * trying to avoid.
 */
function npmCli() {
  const fromEnv = process.env.npm_execpath;
  if (fromEnv && fromEnv.endsWith(".js") && existsSync(fromEnv)) return fromEnv;
  const bundled = path.join(
    path.dirname(process.execPath),
    "node_modules",
    "npm",
    "bin",
    "npm-cli.js",
  );
  if (existsSync(bundled)) return bundled;
  throw new Error("could not locate the npm CLI entry point");
}

function run(args, cwd) {
  return spawnSync(process.execPath, args, {
    cwd,
    encoding: "utf8",
    shell: false,
  });
}

const runNpm = (args, cwd) => run([npmCli(), ...args], cwd);

function firstLine(text) {
  return String(text ?? "").trim().split("\n")[0] ?? "";
}

const CONSUMER_JS = [
  'import {',
  '  MbongoClient,',
  '  receiptHash,',
  '  verifyReceiptSignature,',
  '  signAnchorReceiptTransaction,',
  '  receiptsInBlock,',
  '  wireReceiptToReceipt,',
  '  MbongoRpcError,',
  '  RECEIPT_VERSION,',
  '  MAX_RECEIPT_METADATA_BYTES,',
  '  ANCHOR_RECEIPT_PAYLOAD_PREFIX_BYTES,',
  '} from "@mbongo/sdk";',
  '',
  'function expect(label, ok) {',
  '  if (!ok) throw new Error("consumer assertion failed: " + label);',
  '}',
  '',
  'expect("MbongoClient is constructible", typeof MbongoClient === "function");',
  'expect(',
  '  "a client can be built without touching the network",',
  '  new MbongoClient("http://127.0.0.1:1/") instanceof MbongoClient,',
  ');',
  'for (const [name, fn] of Object.entries({',
  '  receiptHash,',
  '  verifyReceiptSignature,',
  '  signAnchorReceiptTransaction,',
  '  receiptsInBlock,',
  '  wireReceiptToReceipt,',
  '})) {',
  '  expect(name + " is callable", typeof fn === "function");',
  '}',
  'expect("MbongoRpcError extends Error", MbongoRpcError.prototype instanceof Error);',
  'expect("RECEIPT_VERSION is 1", RECEIPT_VERSION === 1);',
  'expect("metadata cap is 4096", MAX_RECEIPT_METADATA_BYTES === 4096);',
  'expect("anchor payload prefix is 90", ANCHOR_RECEIPT_PAYLOAD_PREFIX_BYTES === 90);',
  '',
  '// One offline semantic check: a block carrying no transactions yields no',
  '// receipts. Cheap, but it proves the installed code runs, not just imports.',
  'const empty = receiptsInBlock({',
  '  header: {',
  '    parent_hash: "0x" + "00".repeat(32),',
  '    state_root: "0x" + "00".repeat(32),',
  '    transactions_root: "0x" + "00".repeat(32),',
  '    timestamp: 0,',
  '    height: 0,',
  '  },',
  '  body: { transactions: [] },',
  '});',
  'expect("an empty block yields no receipts", Array.isArray(empty) && empty.length === 0);',
  '',
  'console.log("consumer-js-ok");',
  '',
].join("\n");

const CONSUMER_TS = [
  'import {',
  '  MbongoClient,',
  '  receiptHash,',
  '  verifyReceiptSignature,',
  '  signAnchorReceiptTransaction,',
  '  receiptsInBlock,',
  '} from "@mbongo/sdk";',
  'import type { Receipt, Block, Transaction, WireReceipt } from "@mbongo/sdk";',
  '',
  '// Values and types both have to resolve from the installed declarations.',
  'export const client: MbongoClient = new MbongoClient("http://127.0.0.1:1/");',
  'export const hash: (receipt: Receipt) => Uint8Array = receiptHash;',
  'export const verify: (receipt: Receipt) => boolean = verifyReceiptSignature;',
  'export const extract: (block: Block) => Receipt[] = receiptsInBlock;',
  'export const sign: typeof signAnchorReceiptTransaction = signAnchorReceiptTransaction;',
  'export type ExportedTransaction = Transaction;',
  'export type ExportedWireReceipt = WireReceipt;',
  '',
].join("\n");

const CONSUMER_TSCONFIG = {
  compilerOptions: {
    // NodeNext is the configuration that actually exercises the `exports`
    // map. If a consumer typechecks here, the SDK does not need to migrate
    // its own moduleResolution.
    //
    // `lib` is left unset on purpose, so TypeScript picks its default for the
    // target the way an ordinary consumer project does. Pinning it to
    // ["ES2022"] would drop the ambient `fetch` type that
    // `MbongoClientOptions.fetch` refers to, and the failure would be about
    // this test's configuration rather than about the package.
    target: "ES2022",
    module: "NodeNext",
    moduleResolution: "NodeNext",
    strict: true,
    noEmit: true,
    // Deliberately checking the installed declarations rather than skipping
    // them: a broken .d.ts in the tarball is precisely what this catches.
    skipLibCheck: false,
  },
  include: ["smoke.ts"],
};

const tmpRoot = mkdtempSync(path.join(tmpdir(), "mbongo-sdk-101b-"));

try {
  console.log("packed consumer smoke");
  console.log("  node " + process.version + " in " + tmpRoot);

  // --- the artifact must come from a fresh build ------------------------
  check(
    "dist/ is present (run the build first)",
    existsSync(path.join(PKG_DIR, "dist", "index.js")) &&
      existsSync(path.join(PKG_DIR, "dist", "index.d.ts")),
  );
  if (failed > 0) throw new Error("nothing to pack");

  // --- pack -------------------------------------------------------------
  const packDir = path.join(tmpRoot, "pack");
  mkdirSync(packDir);
  const packed = runNpm(["pack", "--json", "--pack-destination", packDir], PKG_DIR);
  check("npm pack succeeded", packed.status === 0, firstLine(packed.stderr));
  if (packed.status !== 0) throw new Error("pack failed");

  const meta = JSON.parse(packed.stdout)[0];
  const tarball = path.join(packDir, meta.filename);
  check("tarball written to the temporary directory", existsSync(tarball), tarball);
  check(
    "archive filename is derived, not the package name",
    meta.filename === "mbongo-sdk-" + EXPECTED_VERSION + ".tgz",
    meta.filename,
  );

  const files = meta.files.map((f) => f.path.replace(/\\/g, "/"));
  // npm force-includes package.json, README and LICENSE whatever `files`
  // says, so this gate catches the file being absent from the package
  // directory rather than being filtered out of it.
  check("LICENSE is inside the tarball", files.includes("LICENSE"));
  check("README.md is inside the tarball", files.includes("README.md"));
  check("package.json is inside the tarball", files.includes("package.json"));
  check("dist/ is inside the tarball", files.some((f) => f.startsWith("dist/")));
  const unwanted = files.filter((f) =>
    /^(src|test|scripts|node_modules)\/|tsconfig|\.map$|\.tgz$/.test(f),
  );
  check(
    "no sources, tests, config or sourcemaps packed",
    unwanted.length === 0,
    unwanted.join(", "),
  );

  // --- an outside consumer ----------------------------------------------
  const consumerDir = path.join(tmpRoot, "consumer");
  mkdirSync(consumerDir);
  writeFileSync(
    path.join(consumerDir, "package.json"),
    JSON.stringify(
      {
        name: "mbongo-sdk-consumer-smoke",
        version: "0.0.0",
        private: true,
        type: "module",
      },
      null,
      2,
    ) + "\n",
  );
  check(
    "the consumer lives outside the repository",
    !path.resolve(consumerDir).startsWith(path.resolve(REPO_ROOT) + path.sep),
    consumerDir,
  );

  const install = runNpm(
    ["install", tarball, "--no-audit", "--no-fund", "--loglevel=error"],
    consumerDir,
  );
  check(
    "npm install of the local tarball succeeded",
    install.status === 0,
    firstLine(install.stderr),
  );
  if (install.status !== 0) throw new Error("install failed");

  const installed = path.join(consumerDir, "node_modules", "@mbongo", "sdk");
  check("resolved to node_modules/@mbongo/sdk", existsSync(installed));

  // --- the package must come from the tarball, not a registry or a link --
  const lock = JSON.parse(readFileSync(path.join(consumerDir, "package-lock.json"), "utf8"));
  const entry = lock.packages["node_modules/@mbongo/sdk"] ?? {};
  const resolved = String(entry.resolved ?? "");
  check(
    "installed from the local tarball, not from a registry",
    resolved.length > 0 && !/registry\./.test(resolved) && /\.tgz$/.test(resolved),
    resolved || "(no resolved field)",
  );
  check(
    "not a link, symlink or workspace",
    entry.link !== true && !lstatSync(installed).isSymbolicLink(),
  );

  // --- installed metadata survived pack and install ----------------------
  const im = JSON.parse(readFileSync(path.join(installed, "package.json"), "utf8"));
  check("installed name", im.name === EXPECTED_NAME, im.name);
  check("installed version", im.version === EXPECTED_VERSION, im.version);
  check("installed license", im.license === "Apache-2.0", String(im.license));
  check(
    "installed exports map intact",
    im.exports?.["."]?.types === "./dist/index.d.ts" &&
      im.exports?.["."]?.import === "./dist/index.js",
    JSON.stringify(im.exports),
  );
  check(
    "installed main and types intact",
    im.main === "dist/index.js" && im.types === "dist/index.d.ts",
  );

  // --- the licence really travels with the code -------------------------
  const installedLicense = readFileSync(path.join(installed, "LICENSE"));
  const sourceLicense = readFileSync(path.join(PKG_DIR, "LICENSE"));
  const rootLicense = readFileSync(path.join(REPO_ROOT, "LICENSE"));
  const text = (buf) => buf.toString("utf8").replace(/\r\n/g, "\n");
  check("LICENSE present after install", installedLicense.length > 0);
  check(
    "packing and installing preserved LICENSE byte for byte",
    installedLicense.equals(sourceLicense),
    installedLicense.length + " vs " + sourceLicense.length + " bytes",
  );
  // Compared with line endings normalised: a checkout may render either the
  // root or the SDK copy with CRLF depending on platform and git settings,
  // and that difference says nothing about the licence that ships.
  check(
    "installed LICENSE is the repository licence text",
    text(installedLicense) === text(rootLicense),
  );
  check(
    "installed LICENSE is Apache-2.0",
    text(installedLicense).includes("Apache License") &&
      text(installedLicense).includes("Version 2.0"),
  );

  // --- JavaScript ESM consumer ------------------------------------------
  writeFileSync(path.join(consumerDir, "smoke.mjs"), CONSUMER_JS);
  const js = run([path.join(consumerDir, "smoke.mjs")], consumerDir);
  check(
    "JavaScript ESM consumer imported and ran the package",
    js.status === 0 && js.stdout.includes("consumer-js-ok"),
    firstLine(js.stderr) || firstLine(js.stdout),
  );

  // --- the exports map is enforced, not decorative ----------------------
  const subpath = run(
    ["--input-type=module", "-e", 'import("@mbongo/sdk/dist/index.js")'],
    consumerDir,
  );
  check(
    "an undeclared subpath import is refused by the exports map",
    subpath.status !== 0 && /ERR_PACKAGE_PATH_NOT_EXPORTED/.test(subpath.stderr),
    firstLine(subpath.stderr),
  );

  // --- TypeScript consumer ----------------------------------------------
  writeFileSync(path.join(consumerDir, "smoke.ts"), CONSUMER_TS);
  writeFileSync(
    path.join(consumerDir, "tsconfig.json"),
    JSON.stringify(CONSUMER_TSCONFIG, null, 2) + "\n",
  );
  // The pinned compiler from the SDK, pointed at the consumer project. tsc
  // resolves modules from the project it is given, not from where its binary
  // lives, so the declarations still have to come from node_modules.
  const tsc = path.join(PKG_DIR, "node_modules", "typescript", "bin", "tsc");
  check("the pinned TypeScript compiler is available", existsSync(tsc), tsc);
  const ts = run([tsc, "--project", path.join(consumerDir, "tsconfig.json")], consumerDir);
  check(
    "TypeScript consumer typechecks against the installed declarations (NodeNext)",
    ts.status === 0,
    firstLine(ts.stdout) || firstLine(ts.stderr),
  );

  // --- nothing leaked into the repository -------------------------------
  const strays = [PKG_DIR, REPO_ROOT].flatMap((dir) =>
    readdirSync(dir)
      .filter((f) => f.endsWith(".tgz"))
      .map((f) => path.join(dir, f)),
  );
  check("no tarball left in the repository", strays.length === 0, strays.join(", "));
} finally {
  rmSync(tmpRoot, { recursive: true, force: true });
  console.log(
    "  " +
      (existsSync(tmpRoot)
        ? "WARNING: temporary directory survived"
        : "temporary directory removed"),
  );
}

if (failed > 0) {
  console.error("\npacked consumer smoke FAILED (" + failed + " check(s))");
  process.exit(1);
}
console.log("\npacked consumer smoke passed");
