// The nodes of the MVP-D acceptance run and the arguments each one is started
// with (scripts/mvp-d-acceptance.mjs). Kept apart from the run so that a test
// checks exactly what the run passes to mbongo-node, without starting it.
//
// SEC-MDNS: every node runs with --no-mdns, and a follower reaches its peers
// only through the --bootnodes address it is given.

// Harness-profile ports of DEVNET_V0.4_OPERATIONS.md.
export const ACCEPTANCE_NODES = [
  { name: "producer", rpc: 39944, rest: 38080, p2p: 40333, producer: true },
  { name: "follower1", rpc: 39945, rest: 38081, p2p: 40334, producer: false },
  { name: "follower2", rpc: 39946, rest: 38082, p2p: 40335, producer: false },
];

/** The line a node prints after its PeerId when it runs with --no-mdns. */
export const MDNS_OFF_LINE = /^\s*mDNS:\s+disabled \(--no-mdns\)\s*$/m;

/** The mbongo-node arguments of node `n`, with data directory `dir`. */
export function acceptanceNodeArgs(n, dir, bootnode) {
  if (!n.producer && !bootnode) {
    throw new Error(`${n.name}: a follower needs a --bootnodes address`);
  }
  const args = ["--dev", "--rpc-port", String(n.rpc), "--rest-port", String(n.rest), "--p2p-port", String(n.p2p), "--no-mdns", "--data-dir", dir];
  if (n.producer) args.push("--producer", "--block-time", "1");
  if (bootnode) args.push("--bootnodes", bootnode);
  return args;
}
