# Devnet v0.3 Operations Runbook

**Release:** `v0.3-devnet-stable` @ `751034a121cb26701403cee2796cc3212e7a5365`
**Scope:** persistent single-host three-node devnet (Windows, native processes)
**Status:** operational Step 1 — start/stop/status only

> This is a **devnet-stable** deployment, not mainnet-ready. Do not use
> production secrets or real funds. The only funded account is the
> code-baked public devnet key (`ensure_genesis` dev account, seed
> `0xAA…AA`), which is intentionally public and worthless.

---

## Prerequisites

- Windows with PowerShell 5.1+
- Rust stable toolchain (1.75+) — used once, to build the pinned tag
- Git with the `v0.3-devnet-stable` tag available (`git fetch --tags`)
- ~2 GB free disk for the build tree, a few hundred MB for chain data

No administrator privileges are required.

---

## Layout

Everything lives **outside the repository**, under the deployment root
(default `C:\mbongo-devnet\v0.3`; override with the `MBONGO_DEVNET_ROOT`
environment variable):

```text
C:\mbongo-devnet\v0.3\
├── build\src\            git worktree pinned to v0.3-devnet-stable (build only)
├── bin\mbongo-node.exe   deployed binary (copied from the tag build)
├── manifest.json         tag, commit, binary path, SHA-256, build timestamp
├── producer\
│   ├── data\             RocksDB chain data (persistent)
│   ├── logs\             per-run timestamped stdout/stderr logs
│   ├── node.pid.json     PID, start time, exe path, log paths
│   └── deployment.json   data-directory provenance marker
├── follower-a\           same layout
└── follower-b\           same layout
```

## Topology and ports

| Node | Role | RPC | REST | P2P | Flags |
|------|------|-----|------|-----|-------|
| producer | block producer | 9944 | 8080 | 30333 | `--producer --block-time 5` |
| follower-a | follower | 9945 | 8081 | 30334 | `--bootnodes <producer>` |
| follower-b | follower | 9946 | 8082 | 30335 | `--bootnodes <producer>` |

These ports are deliberately distinct from the test-harness ranges
(19944+, 29944+, 39944+), so `cargo run --bin devnet_harness` can run
while the operational devnet is up.

---

## Tag-pinned build (automatic on first start)

`start-devnet.ps1` builds the binary the first time it runs:

1. `git worktree add <root>\build\src v0.3-devnet-stable` — a clean tree
   at the tag; the live `dev` branch is never built or run.
2. Verifies the worktree is at exactly commit
   `751034a121cb26701403cee2796cc3212e7a5365`, describes as exactly the
   tag, and is clean.
3. `cargo build --release --locked -p mbongo-node` inside the worktree.
4. Copies `mbongo-node.exe` to `<root>\bin\` and writes `manifest.json`
   with the tag, commit, binary path, SHA-256, and build timestamp.

On every subsequent start, the script recomputes the binary's SHA-256
and refuses to launch anything if the manifest, tag, commit, path, or
hash does not match.

To force a rebuild: stop the devnet, then delete `<root>\bin`,
`<root>\manifest.json`, and remove the worktree
(`git worktree remove <root>\build\src`), then start again.

---

## Operating the devnet

From `scripts\devnet\` in the repository:

```powershell
# Start (builds first if needed; producer first, then followers)
.\start-devnet.ps1

# Inspect (process, RPC, heights, tips, ports, convergence, manifest)
.\status-devnet.ps1

# Stop (only this deployment's recorded PIDs; data preserved)
.\stop-devnet.ps1
```

Start behavior: producer starts first and its RPC must answer `ping`
within 60 s; the producer's PeerId is read from its log to derive the
followers' `--bootnodes` address **fresh on every start** (identity is
ephemeral — see limitations); followers start and must answer `ping`;
the script then confirms block height is advancing before declaring
success. It fails clearly if ports are occupied, if the deployment is
already running, or if any data directory has unknown provenance.

Stop behavior: stops **only** the PIDs recorded in this deployment's
PID files, and only after verifying the live process still runs the
deployed binary path. Never kills by process name. Stale PID files are
reported and removed. Data directories are never touched.

## Fresh genesis and data persistence

Genesis is **code-defined and deterministic**: every v0.3 node computes
the identical genesis block (empty body, funded public dev account) on
first start of an empty data directory. There is no genesis file to
distribute — running the verified binary on empty directories *is* the
approved fresh genesis.

Data directories persist across restarts; a restarted devnet resumes
from its stored height. The scripts **never** delete, reset, migrate,
or overwrite chain data:

- A non-empty data directory is reused only if its `deployment.json`
  provenance marker matches this tag and commit.
- A non-empty directory without a matching marker (e.g. an old v0.2
  directory or anything of unknown origin) makes start **refuse** with
  an explanation. Backup-and-confirmed-wipe is the job of the future
  reset procedure — for now, move such directories aside manually if a
  fresh start is intended.
- Old v0.2 directories are never opened or migrated by these scripts.

## Backup note (interim)

Until the dedicated backup script exists: stop the devnet (or one
node), copy that node's `data\` directory, restart. RocksDB data is
consistent at rest. Rollback = restore a copied directory under the
same v0.3 binary. Rolling back to v0.2 is impossible by design (see
[PROTOCOL_LOCK_v0.3.md](../specs/PROTOCOL_LOCK_v0.3.md)).

---

## Known limitations (this phase)

- **Ephemeral P2P identity:** the node generates a fresh PeerId every
  start; there is no node-key flag yet. Single-host operation is
  unaffected (bootnode address is re-derived each start, and same-host
  mDNS re-discovers peers after a producer restart). Multi-host
  deployment would need a persistent-key CLI addition.
- **Loopback-only RPC/REST:** the node binds RPC and REST to
  `127.0.0.1`. All tooling must run on the same host. (This also rules
  out containerized deployment until a bind-address flag exists.)
- **No dedicated receipt RPC:** `submit_receipt`/`get_receipt` are
  reserved and return `-32601`. Receipts are submitted through
  `submit_transaction` (tooling arrives with the smoke-test step).
- **No metrics endpoint:** no Prometheus/telemetry; observability is
  logs + RPC polling until the soak tooling lands.
- **Windows process stop is forceful** (no graceful shutdown signal);
  RocksDB's write-ahead log makes this safe, and the devnet harness
  exercises exactly this restart path.

## Future steps (not yet implemented)

1. Receipt submission smoke test (wallet example + script)
2. Backup and confirmed reset/wipe scripts
3. Convergence + receipt verification script
4. 48–72 h soak test with resource/error monitoring
