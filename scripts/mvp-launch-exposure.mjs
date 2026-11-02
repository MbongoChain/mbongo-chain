// Network exposure of the controlled-launch stack (F-N2,
// docs/security/MVP_LAUNCH_SECURITY_REVIEW.md). Pure functions, imported by
// scripts/mvp-launch.mjs and tested by scripts/mvp-launch-exposure.test.mjs.

// How a controlled-launch node is started: P2P bound to loopback (F-N1) and
// mDNS off (F-N2), so the node neither advertises itself nor discovers or
// auto-dials peers on any interface; it reaches its peers through
// --bootnodes only.
export function launchNodeArgs(n, dataDir) {
  return ["--rpc-port", String(n.rpc), "--rest-port", String(n.rest), "--p2p-port", String(n.p2p), "--p2p-host", "127.0.0.1", "--no-mdns", "--data-dir", dataDir];
}

export const MDNS_PORT = 5353;
export const isLoopback = (address) => /^127\./.test(address) || address === "::1";

// Classifies every TCP and UDP socket owned by a process of the stack.
//   sockets:   [{ proto: "tcp" | "udp", address, port, pid, state? }] —
//              sockets of other processes may be present and are ignored
//   pids:      { nodes: [pid], service: pid, llama: [pid] }
//   nodeCommandLines: the command line of each node process
//   ports:     { p2p: [port], rpc: [port], rest: [port] }
// Any owned socket outside loopback is an alert, with one documented
// exception (BOUND_TWIN_OF_LOOPBACK_CONNECTION): for an outbound connection
// Windows also lists a TCP row in state "Bound" on the wildcard address with
// the connection's local port. It neither listens nor carries traffic; it is
// ignored only when the same process holds a connected loopback row on that
// port. Every other Bound row counts.
export function exposure({ sockets, pids, nodeCommandLines, ports }) {
  const owned = new Set([...pids.nodes, pids.service, ...pids.llama].filter((p) => Number.isInteger(p)));
  const all = sockets.filter((s) => owned.has(s.pid));
  const boundTwin = (s) =>
    s.proto === "tcp" && s.state === "Bound" && all.some((t) => t !== s && t.proto === "tcp" && t.pid === s.pid && t.port === s.port && t.state !== "Bound" && t.state !== "Listen" && isLoopback(t.address));
  const mine = all.filter((s) => !boundTwin(s));
  const outside = mine.filter((s) => !isLoopback(s.address));
  const tcpOn = (list) => mine.filter((s) => s.proto === "tcp" && list.includes(s.port));
  const loopbackOnly = (list) => {
    const on = tcpOn(list);
    if (on.length === 0) return "NOT_LISTENING";
    return on.every((s) => isLoopback(s.address)) ? "LOOPBACK_ONLY" : "EXPOSED";
  };
  const mdnsSockets = mine.filter((s) => s.proto === "udp" && s.port === MDNS_PORT);
  const allNodesNoMdns = nodeCommandLines.length === pids.nodes.length && nodeCommandLines.every((c) => /(^|\s)--no-mdns(\s|$)/.test(c || ""));
  const report = {
    P2P_TCP: loopbackOnly(ports.p2p),
    MDNS: mdnsSockets.length === 0 && allNodesNoMdns ? "DISABLED" : "ENABLED",
    UDP_NON_LOOPBACK: outside.filter((s) => s.proto === "udp").length,
    TCP_NON_LOOPBACK: outside.filter((s) => s.proto === "tcp").length,
    RPC: loopbackOnly(ports.rpc),
    REST: loopbackOnly(ports.rest),
    owned_sockets: mine.length,
    ignored_bound_twins: all.length - mine.length,
    non_loopback: outside.map((s) => `${s.proto} ${s.address}:${s.port} pid=${s.pid}`),
  };
  report.alerts = [];
  if (outside.length) report.alerts.push(`NON_LOOPBACK_SOCKET: ${report.non_loopback.join(", ")} — stop and fix before launch`);
  if (report.MDNS !== "DISABLED") report.alerts.push("MDNS_ENABLED: a node runs mDNS discovery (start nodes with --no-mdns)");
  for (const k of ["P2P_TCP", "RPC", "REST"]) if (report[k] !== "LOOPBACK_ONLY") report.alerts.push(`${k}_${report[k]}`);
  return report;
}
