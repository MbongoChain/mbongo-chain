// SEC-MDNS guard: every node an MVP launcher starts runs with --no-mdns, and
// every follower names its peers with --bootnodes, so no node needs discovery.
//
// It reads the arguments the launchers build, not their source text:
//   - docker-compose.yml, as Docker Compose resolves it (`docker compose
//     config`), with the CI environment files and every profile;
//   - the MVP-D acceptance run, through the argument builder it calls
//     (scripts/mvp-d-node-args.mjs), plus a check that the run starts nodes
//     only through that builder.
// A flag in a comment, or inside another value, does not count. The set of
// node invocations is fixed: a missing or an unexpected one fails.
//
// The other covered launchers are checked where their arguments are built:
// compute_vertical (Rust unit tests, and the node's own mDNS line at run
// time), mvp-launch.mjs (scripts/mvp-launch-exposure.test.mjs) and the
// Docker devnet's running containers (scripts/devnet/docker-devnet.sh).
//
//   node scripts/check-sec-mdns.mjs        exit 0 on PASS, 1 on any failure

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { ACCEPTANCE_NODES, acceptanceNodeArgs } from "./mvp-d-node-args.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export const MDNS_OFF = "--no-mdns";

/** The node invocations each launcher must have, by name. */
export const EXPECTED = {
  compose: ["follower-a", "follower-b", "producer"],
  acceptance: ["follower1", "follower2", "producer"],
};

const MULTIADDR = /^\/ip4\/(\d{1,3}(?:\.\d{1,3}){3})\/tcp\/(\d{1,5})(?:\/p2p\/[1-9A-HJ-NP-Za-km-z]+)?$/;

/** The values of `flag`, given as `flag value` or `flag=value`. */
export function flagValues(args, flag) {
  const out = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === flag && i + 1 < args.length) out.push(args[i + 1]);
    else if (args[i].startsWith(flag + "=")) out.push(args[i].slice(flag.length + 1));
  }
  return out;
}

/**
 * Checks a launcher's node invocations: `[{ name, args, producerAddress? }]`.
 * Returns the list of failures, empty when every rule holds.
 */
export function checkInvocations(launcher, invocations, expectedNames, producer) {
  const errors = [];
  const names = invocations.map((i) => i.name).sort();
  const expected = [...expectedNames].sort();
  for (const n of expected) if (!names.includes(n)) errors.push(`${launcher}: expected node ${n} is missing`);
  for (const n of names) if (!expected.includes(n)) errors.push(`${launcher}: unexpected node invocation ${n}`);
  if (names.length !== new Set(names).size) errors.push(`${launcher}: a node name repeats`);
  for (const { name, args } of invocations) {
    if (!Array.isArray(args) || !args.every((a) => typeof a === "string")) {
      errors.push(`${launcher}/${name}: arguments are not a list of strings`);
      continue;
    }
    const flags = args.filter((a) => a === MDNS_OFF).length;
    if (flags !== 1) errors.push(`${launcher}/${name}: ${MDNS_OFF} appears ${flags} times as an argument, expected once`);
    const isProducer = args.includes("--producer");
    const boots = flagValues(args, "--bootnodes");
    if (isProducer) continue;
    if (boots.length === 0) {
      errors.push(`${launcher}/${name}: a follower without --bootnodes`);
      continue;
    }
    for (const b of boots) {
      const m = b.match(MULTIADDR);
      if (!m) {
        errors.push(`${launcher}/${name}: --bootnodes ${JSON.stringify(b)} is not an /ip4/<address>/tcp/<port> multiaddr`);
      } else if (producer && (m[1] !== producer.ip || Number(m[2]) !== producer.port)) {
        errors.push(`${launcher}/${name}: --bootnodes ${b} is not the producer at ${producer.ip}:${producer.port}`);
      }
    }
  }
  return errors;
}

// ── docker-compose.yml ────────────────────────────────────────────────────

function compose(args, cwd) {
  const r = spawnSync("docker", ["compose", ...args], { cwd, encoding: "utf8" });
  if (r.error) throw new Error(`docker compose is required: ${r.error.message}`);
  if (r.status !== 0) throw new Error(`docker compose ${args.join(" ")} failed: ${r.stderr.trim()}`);
  return r.stdout;
}

/** Docker Compose's resolved configuration of `file`, with every profile it declares enabled. */
export function resolveCompose(file, envFiles = [".env.base", ".env.ci"], cwd = ROOT) {
  const base = [...envFiles.flatMap((e) => ["--env-file", e]), "-f", file];
  const profiles = compose([...base, "config", "--profiles"], cwd).split(/\r?\n/).filter(Boolean);
  const enabled = profiles.flatMap((p) => ["--profile", p]);
  return JSON.parse(compose([...base, ...enabled, "config", "--format", "json"], cwd));
}

/** The image's entrypoint, from the Dockerfile: the binary a service runs by default. */
export function dockerfileEntrypoint(dockerfile = path.join(ROOT, "Dockerfile")) {
  const lines = fs.readFileSync(dockerfile, "utf8").split(/\r?\n/).filter((l) => /^\s*ENTRYPOINT\s/.test(l));
  if (lines.length !== 1) throw new Error(`expected one ENTRYPOINT in the Dockerfile, found ${lines.length}`);
  return JSON.parse(lines[0].replace(/^\s*ENTRYPOINT\s+/, ""));
}

/**
 * The node invocations of a resolved Compose configuration: every service
 * whose process is mbongo-node, by its own entrypoint or the image's.
 */
export function composeInvocations(config, imageEntrypoint) {
  const out = [];
  let producer = null;
  for (const [name, svc] of Object.entries(config.services ?? {})) {
    const entry = svc.entrypoint ?? imageEntrypoint;
    const bin = Array.isArray(entry) ? entry[0] : String(entry ?? "").split(/\s+/)[0];
    if (path.basename(bin ?? "") !== "mbongo-node") continue;
    const args = [...(Array.isArray(entry) ? entry.slice(1) : []), ...(svc.command ?? [])];
    out.push({ name, args });
    if (args.includes("--producer")) {
      const ip = Object.values(svc.networks ?? {}).map((n) => n?.ipv4_address).find(Boolean);
      const port = Number(flagValues(args, "--p2p-port")[0]);
      producer = ip && port ? { ip, port } : null;
    }
  }
  return { invocations: out, producer };
}

export function checkCompose(config, imageEntrypoint) {
  const { invocations, producer } = composeInvocations(config, imageEntrypoint);
  const errors = checkInvocations("docker-compose.yml", invocations, EXPECTED.compose, producer);
  if (!producer) errors.push("docker-compose.yml: no producer with a fixed ipv4_address and --p2p-port");
  return { errors, count: invocations.length };
}

// ── scripts/mvp-d-acceptance.mjs ──────────────────────────────────────────

/** The acceptance run's node invocations, built by the builder it calls. */
export function acceptanceInvocations(build = acceptanceNodeArgs, nodes = ACCEPTANCE_NODES) {
  const producer = nodes.find((n) => n.producer);
  const boot = `/ip4/127.0.0.1/tcp/${producer.p2p}/p2p/12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN`;
  return nodes.map((n) => ({ name: n.name, args: build(n, `/run/nodes/${n.name}`, n.producer ? null : boot) }));
}

/** Source text without JavaScript comments, for the static spawn-site check. */
export function stripJsComments(src) {
  return src.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:"'`\\])\/\/.*$/gm, "$1");
}

/** The acceptance run starts mbongo-node at one place, with the builder's arguments. */
export function checkAcceptanceSpawnSite(src) {
  const code = stripJsComments(src);
  const errors = [];
  // Each spawn of the node binary, with the expression of its argument list.
  const spawns = [...code.matchAll(/\bspawn\(\s*BIN\("mbongo-node"\)\s*,\s*([^,)]+)/g)].map((m) => m[1].trim());
  if (spawns.length !== 1) errors.push(`mvp-d-acceptance.mjs: ${spawns.length} spawn sites of mbongo-node, expected 1`);
  if (!/const args = acceptanceNodeArgs\(n, dir, bootnode\);/.test(code)) {
    errors.push("mvp-d-acceptance.mjs: the node arguments are not built by acceptanceNodeArgs");
  }
  if (spawns.some((a) => a !== "args")) errors.push("mvp-d-acceptance.mjs: mbongo-node is spawned with other arguments");
  // Any other process start that names the node binary is a second launch path.
  const otherStarts = code.match(/\b(?:spawn|spawnSync|exec|execFile|execSync|execFileSync|fork)\([^)]*mbongo-node/g) ?? [];
  if (otherStarts.length !== spawns.length) {
    errors.push(`mvp-d-acceptance.mjs: ${otherStarts.length} process starts name mbongo-node, expected ${spawns.length}`);
  }
  return errors;
}

export function checkAcceptance(build = acceptanceNodeArgs, src = fs.readFileSync(path.join(ROOT, "scripts", "mvp-d-acceptance.mjs"), "utf8")) {
  const invocations = acceptanceInvocations(build);
  const errors = [
    ...checkInvocations("mvp-d-acceptance.mjs", invocations, EXPECTED.acceptance, null),
    ...checkAcceptanceSpawnSite(src),
  ];
  return { errors, count: invocations.length };
}

// ── main ──────────────────────────────────────────────────────────────────

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const compose = checkCompose(resolveCompose("docker-compose.yml"), dockerfileEntrypoint());
  const acceptance = checkAcceptance();
  const errors = [...compose.errors, ...acceptance.errors];
  console.log(`docker-compose.yml: ${compose.count} node invocations checked, expected ${EXPECTED.compose.length}`);
  console.log(`mvp-d-acceptance.mjs: ${acceptance.count} node invocations checked, expected ${EXPECTED.acceptance.length}`);
  for (const e of errors) console.error(`FAIL ${e}`);
  console.log(errors.length ? `SEC-MDNS: FAIL (${errors.length})` : "SEC-MDNS: PASS");
  process.exit(errors.length ? 1 : 0);
}
