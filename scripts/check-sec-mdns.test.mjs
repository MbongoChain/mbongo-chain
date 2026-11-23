// Tests of the SEC-MDNS guard (scripts/check-sec-mdns.mjs).
//
// The positive cases run on the repository's own launchers. The negative
// cases change one thing each and require the guard to name it: Compose
// fixtures are resolved by Docker Compose itself, the acceptance run is
// checked through mutated argument builders and mutated source text.
//   node --test scripts/check-sec-mdns.test.mjs

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  EXPECTED,
  acceptanceInvocations,
  checkAcceptance,
  checkAcceptanceSpawnSite,
  checkCompose,
  checkInvocations,
  dockerfileEntrypoint,
  resolveCompose,
} from "./check-sec-mdns.mjs";
import { ACCEPTANCE_NODES, acceptanceNodeArgs } from "./mvp-d-node-args.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const COMPOSE = fs.readFileSync(path.join(ROOT, "docker-compose.yml"), "utf8");
const ACCEPTANCE = fs.readFileSync(path.join(ROOT, "scripts", "mvp-d-acceptance.mjs"), "utf8");
const ENV = [path.join(ROOT, ".env.base"), path.join(ROOT, ".env.ci")];
const ENTRY = dockerfileEntrypoint();

/** The guard's verdict on a Compose file that differs from the real one by `edit`. */
function composeVerdict(edit) {
  const text = edit(COMPOSE);
  assert.notEqual(text, COMPOSE, "the edit changed nothing");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "sec-mdns-"));
  try {
    const file = path.join(dir, "docker-compose.yml");
    fs.writeFileSync(file, text);
    return checkCompose(resolveCompose(file, ENV, dir), ENTRY);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

/** Replaces the `n`-th (1-based) occurrence of `from`, which must exist. */
function nth(text, from, to, n) {
  let at = -1;
  for (let i = 0; i < n; i++) {
    at = text.indexOf(from, at + 1);
    assert.ok(at >= 0, `occurrence ${n} of ${JSON.stringify(from)}`);
  }
  return text.slice(0, at) + to + text.slice(at + from.length);
}

const FLAG_LINE = "      - --no-mdns\n";

// ── the repository's launchers ───────────────────────────────────────────

test("the image runs mbongo-node", () => {
  assert.deepEqual(ENTRY, ["mbongo-node"]);
});

test("every Compose node runs with --no-mdns and every follower names the producer", () => {
  const { errors, count } = checkCompose(resolveCompose("docker-compose.yml"), ENTRY);
  assert.equal(count, EXPECTED.compose.length);
  assert.equal(count, 3);
  assert.deepEqual(errors, []);
  assert.equal(COMPOSE.split(FLAG_LINE).length - 1, 3, "three flag lines in the file");
});

test("every acceptance node runs with --no-mdns, built by the builder the run calls", () => {
  const { errors, count } = checkAcceptance();
  assert.equal(count, 3);
  assert.deepEqual(errors, []);
  for (const { name, args } of acceptanceInvocations()) {
    assert.equal(args.filter((a) => a === "--no-mdns").length, 1, name);
  }
});

// ── Compose: one change each ─────────────────────────────────────────────

test("a Compose node without --no-mdns fails", () => {
  for (const n of [1, 2, 3]) {
    const { errors } = composeVerdict((t) => nth(t, FLAG_LINE, "", n));
    assert.equal(errors.length, 1, `${n}: ${errors}`);
    assert.match(errors[0], /--no-mdns appears 0 times/);
  }
});

test("--no-mdns in a comment does not count", () => {
  const { errors } = composeVerdict((t) => nth(t, FLAG_LINE, "      # - --no-mdns\n", 2));
  assert.equal(errors.length, 1, String(errors));
  assert.match(errors[0], /follower-a: --no-mdns appears 0 times/);
});

test("--no-mdns inside another value does not count", () => {
  for (const value of ["--no-mdns=true", "--data-dir=/data/--no-mdns", "--no-mdnsx"]) {
    const { errors } = composeVerdict((t) => nth(t, FLAG_LINE, `      - ${value}\n`, 3));
    assert.ok(errors.some((e) => /follower-b: --no-mdns appears 0 times/.test(e)), `${value}: ${errors}`);
  }
});

test("a Compose follower without --bootnodes fails", () => {
  const { errors } = composeVerdict((t) => nth(t, "      - --bootnodes=/ip4/${MBONGO_PRODUCER_IP}/tcp/${MBONGO_P2P_PORT}\n", "", 2));
  assert.deepEqual(errors, ["docker-compose.yml/follower-b: a follower without --bootnodes"]);
});

test("a Compose follower whose bootnode is not the producer fails", () => {
  const { errors } = composeVerdict((t) => nth(t, "--bootnodes=/ip4/${MBONGO_PRODUCER_IP}/", "--bootnodes=/ip4/172.28.53.99/", 1));
  assert.equal(errors.length, 1, String(errors));
  assert.match(errors[0], /follower-a: --bootnodes \/ip4\/172\.28\.53\.99\/tcp\/\d+ is not the producer/);
});

test("an unexpected Compose node fails, whether it uses the image's entrypoint or its own", () => {
  const extra = (entry) => (t) =>
    t.replace(
      "\n  convergence-check:\n",
      `\n  follower-c:\n    <<: *node\n${entry}    command:\n      - --dev\n      - --no-mdns\n      - --bootnodes=/ip4/\${MBONGO_PRODUCER_IP}/tcp/\${MBONGO_P2P_PORT}\n    networks:\n      - devnet\n\n  convergence-check:\n`,
    );
  for (const entry of ["", '    entrypoint: ["/usr/local/bin/mbongo-node"]\n']) {
    const { errors, count } = composeVerdict(extra(entry));
    assert.equal(count, 4, entry);
    assert.deepEqual(errors, ["docker-compose.yml: unexpected node invocation follower-c"], entry);
  }
});

test("a service with another entrypoint is not a node", () => {
  // convergence-check runs convergence_probe from the same image: it is
  // seen, and is not counted as a node.
  const config = resolveCompose("docker-compose.yml");
  assert.deepEqual(config.services["convergence-check"].entrypoint, ["convergence_probe"]);
  assert.equal(checkCompose(config, ENTRY).count, 3);
});

// ── the acceptance run: one change each ──────────────────────────────────

test("an acceptance builder that drops --no-mdns fails", () => {
  const dropped = (n, dir, boot) => acceptanceNodeArgs(n, dir, boot).filter((a) => a !== "--no-mdns");
  const { errors } = checkAcceptance(dropped);
  assert.equal(errors.length, 3, String(errors));
  for (const e of errors) assert.match(e, /--no-mdns appears 0 times/);
});

test("an acceptance builder that repeats --no-mdns fails", () => {
  const doubled = (n, dir, boot) => [...acceptanceNodeArgs(n, dir, boot), "--no-mdns"];
  assert.equal(checkAcceptance(doubled).errors.length, 3);
});

test("an acceptance follower without a bootnode is refused by the builder", () => {
  for (const n of ACCEPTANCE_NODES.filter((x) => !x.producer)) {
    assert.throws(() => acceptanceNodeArgs(n, "/d", null), /a follower needs a --bootnodes address/);
  }
  const noBoot = (n, dir) => ["--dev", "--no-mdns", "--data-dir", dir, ...(n.producer ? ["--producer"] : [])];
  const { errors } = checkAcceptance(noBoot);
  assert.equal(errors.filter((e) => /a follower without --bootnodes/.test(e)).length, 2, String(errors));
});

test("an unexpected acceptance node fails", () => {
  const extra = [...ACCEPTANCE_NODES, { name: "follower3", rpc: 1, rest: 2, p2p: 3, producer: false }];
  const errors = checkInvocations("mvp-d-acceptance.mjs", acceptanceInvocations(acceptanceNodeArgs, extra), EXPECTED.acceptance, null);
  assert.deepEqual(errors, ["mvp-d-acceptance.mjs: unexpected node invocation follower3"]);
});

test("the acceptance run must start nodes through the builder, not around it", () => {
  assert.deepEqual(checkAcceptanceSpawnSite(ACCEPTANCE), []);
  const builder = "  const args = acceptanceNodeArgs(n, dir, bootnode);";
  assert.ok(ACCEPTANCE.includes(builder));
  // The builder call kept only in a comment, the real arguments built inline.
  const commented = ACCEPTANCE.replace(builder, `  // ${builder.trim()}\n  const args = ["--dev", "--data-dir", dir];`);
  assert.ok(checkAcceptanceSpawnSite(commented).some((e) => /not built by acceptanceNodeArgs/.test(e)));
  // The spawn given other arguments.
  const inline = ACCEPTANCE.replace('spawn(BIN("mbongo-node"), args,', 'spawn(BIN("mbongo-node"), ["--dev"],');
  assert.ok(checkAcceptanceSpawnSite(inline).some((e) => /spawned with other arguments/.test(e)));
  // A second launch path.
  const second = ACCEPTANCE.replace("async function spawnNode(", 'spawnSync(BIN("mbongo-node"), ["--dev"]);\nasync function spawnNode(');
  assert.ok(checkAcceptanceSpawnSite(second).length > 0);
});
