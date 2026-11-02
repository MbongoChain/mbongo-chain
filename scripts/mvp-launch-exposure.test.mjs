// F-N2: the controlled-launch tooling starts nodes without mDNS and its
// health check alerts on any socket of the stack outside loopback.
//   node --test scripts/mvp-launch-exposure.test.mjs
import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import { exposure, launchNodeArgs } from "./mvp-launch-exposure.mjs";

const PORTS = { p2p: [40333, 40334, 40335], rpc: [39944, 39945, 39946], rest: [38080, 38081, 38082] };
const PIDS = { nodes: [101, 102, 103], service: 200, llama: [300] };
const LAUNCH_LINES = [101, 102, 103].map(() => "mbongo-node.exe --p2p-host 127.0.0.1 --no-mdns --data-dir x");
// The healthy controlled-launch stack: every socket on 127.0.0.1, no UDP.
const healthy = () => [
  ...PORTS.p2p.map((port, i) => ({ proto: "tcp", address: "127.0.0.1", port, pid: PIDS.nodes[i] })),
  ...PORTS.rpc.map((port, i) => ({ proto: "tcp", address: "127.0.0.1", port, pid: PIDS.nodes[i] })),
  ...PORTS.rest.map((port, i) => ({ proto: "tcp", address: "127.0.0.1", port, pid: PIDS.nodes[i] })),
  { proto: "tcp", address: "127.0.0.1", port: 8740, pid: 200 },
  { proto: "tcp", address: "127.0.0.1", port: 9931, pid: 300 },
  { proto: "tcp", address: "127.0.0.1", port: 51000, pid: 102 }, // outbound to the bootnode
];

test("launch tooling starts every node with --p2p-host 127.0.0.1 --no-mdns", () => {
  const args = launchNodeArgs({ rpc: 39944, rest: 38080, p2p: 40333 }, "d");
  assert.deepEqual(args.slice(args.indexOf("--p2p-host"), args.indexOf("--p2p-host") + 3), ["--p2p-host", "127.0.0.1", "--no-mdns"]);
  // mvp-launch.mjs builds its node arguments with launchNodeArgs, not by hand.
  const src = fs.readFileSync(new URL("./mvp-launch.mjs", import.meta.url), "utf8");
  assert.match(src, /launchNodeArgs\(n, path\.join\(dataRoot, n\.name\)\)/);
  assert.doesNotMatch(src, /"--p2p-port", String/);
});

test("all owned sockets on loopback: healthy, expected state", () => {
  const r = exposure({ sockets: healthy(), pids: PIDS, nodeCommandLines: LAUNCH_LINES, ports: PORTS });
  assert.deepEqual(r.alerts, []);
  assert.equal(r.P2P_TCP, "LOOPBACK_ONLY");
  assert.equal(r.MDNS, "DISABLED");
  assert.equal(r.UDP_NON_LOOPBACK, 0);
  assert.equal(r.TCP_NON_LOOPBACK, 0);
  assert.equal(r.RPC, "LOOPBACK_ONLY");
  assert.equal(r.REST, "LOOPBACK_ONLY");
  assert.equal(r.owned_sockets, 12);
});

test("a Mbongo-owned UDP socket on a LAN interface is an alert", () => {
  const sockets = [...healthy(), { proto: "udp", address: "192.168.2.37", port: 61754, pid: 101 }];
  const r = exposure({ sockets, pids: PIDS, nodeCommandLines: LAUNCH_LINES, ports: PORTS });
  assert.equal(r.UDP_NON_LOOPBACK, 1);
  assert.deepEqual(r.non_loopback, ["udp 192.168.2.37:61754 pid=101"]);
  assert.match(r.alerts[0], /^NON_LOOPBACK_SOCKET: udp 192\.168\.2\.37:61754 pid=101/);
});

test("the F-N2 state observed on 2026-10-27 fails on every count", () => {
  const sockets = [
    ...healthy(),
    { proto: "udp", address: "0.0.0.0", port: 5353, pid: 102 },
    ...[101, 102, 103].flatMap((pid, i) => [
      { proto: "udp", address: "192.168.2.37", port: 61754 + 3 * i, pid },
      { proto: "udp", address: "172.24.48.1", port: 61755 + 3 * i, pid },
    ]),
  ];
  const lines = [101, 102, 103].map(() => "mbongo-node.exe --p2p-host 127.0.0.1 --data-dir x");
  const r = exposure({ sockets, pids: PIDS, nodeCommandLines: lines, ports: PORTS });
  assert.equal(r.MDNS, "ENABLED");
  assert.equal(r.UDP_NON_LOOPBACK, 7);
  assert.equal(r.P2P_TCP, "LOOPBACK_ONLY"); // what F-N1 fixed — and all the old check saw
  assert.ok(r.alerts.some((a) => a.startsWith("NON_LOOPBACK_SOCKET")));
  assert.ok(r.alerts.some((a) => a.startsWith("MDNS_ENABLED")));
});

test("a node started without --no-mdns is reported even before it opens a socket", () => {
  const lines = [...LAUNCH_LINES.slice(0, 2), "mbongo-node.exe --p2p-host 127.0.0.1"];
  const r = exposure({ sockets: healthy(), pids: PIDS, nodeCommandLines: lines, ports: PORTS });
  assert.equal(r.MDNS, "ENABLED");
  assert.equal(r.UDP_NON_LOOPBACK, 0);
});

test("non-loopback TCP of the stack (wildcard listener or LAN dial) is an alert", () => {
  const sockets = healthy().map((s) => (s.port === 40333 ? { ...s, address: "0.0.0.0" } : s));
  sockets.push({ proto: "tcp", address: "192.168.2.37", port: 52000, pid: 103 });
  const r = exposure({ sockets, pids: PIDS, nodeCommandLines: LAUNCH_LINES, ports: PORTS });
  assert.equal(r.P2P_TCP, "EXPOSED");
  assert.equal(r.TCP_NON_LOOPBACK, 2);
  assert.ok(r.alerts.includes("P2P_TCP_EXPOSED"));
});

test("sockets of unrelated processes are not flagged", () => {
  const sockets = [
    ...healthy(),
    { proto: "udp", address: "0.0.0.0", port: 5353, pid: 9001 }, // e.g. a browser's mDNS
    { proto: "tcp", address: "0.0.0.0", port: 445, pid: 4 }, // a system service
    { proto: "udp", address: "192.168.2.37", port: 1900, pid: 9002 },
  ];
  const r = exposure({ sockets, pids: PIDS, nodeCommandLines: LAUNCH_LINES, ports: PORTS });
  assert.deepEqual(r.alerts, []);
  assert.equal(r.MDNS, "DISABLED");
});

test("Windows' Bound twin of an outbound loopback connection is the only exception", () => {
  // Observed live on 2026-11-01: a --no-mdns follower dialing its bootnode.
  const twin = [
    { proto: "tcp", address: "0.0.0.0", port: 53976, pid: 102, state: "Bound" },
    { proto: "tcp", address: "127.0.0.1", port: 53976, pid: 102, state: "Established" },
  ];
  const ok = exposure({ sockets: [...healthy(), ...twin], pids: PIDS, nodeCommandLines: LAUNCH_LINES, ports: PORTS });
  assert.deepEqual(ok.alerts, []);
  assert.equal(ok.ignored_bound_twins, 1);
  // A Bound row without that twin — other port, other process, or a twin
  // that is not loopback — still counts.
  for (const other of [
    { proto: "tcp", address: "0.0.0.0", port: 53977, pid: 102, state: "Bound" },
    { proto: "tcp", address: "0.0.0.0", port: 53976, pid: 103, state: "Bound" },
  ]) {
    const r = exposure({ sockets: [...healthy(), ...twin, other], pids: PIDS, nodeCommandLines: LAUNCH_LINES, ports: PORTS });
    assert.equal(r.TCP_NON_LOOPBACK, 1);
  }
  const lanTwin = [
    { proto: "tcp", address: "0.0.0.0", port: 53978, pid: 102, state: "Bound" },
    { proto: "tcp", address: "192.168.2.37", port: 53978, pid: 102, state: "Established" },
  ];
  const r = exposure({ sockets: [...healthy(), ...lanTwin], pids: PIDS, nodeCommandLines: LAUNCH_LINES, ports: PORTS });
  assert.equal(r.TCP_NON_LOOPBACK, 2);
  assert.equal(r.ignored_bound_twins, 0);
});

test("a missing listener is not reported as loopback-only", () => {
  const sockets = healthy().filter((s) => !PORTS.rest.includes(s.port));
  const r = exposure({ sockets, pids: PIDS, nodeCommandLines: LAUNCH_LINES, ports: PORTS });
  assert.equal(r.REST, "NOT_LISTENING");
  assert.ok(r.alerts.includes("REST_NOT_LISTENING"));
});
