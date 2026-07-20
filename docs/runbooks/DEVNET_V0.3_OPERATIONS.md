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

## Receipt smoke test

With the devnet running, submit and verify one real `AnchorReceipt`
(built and signed by the `mbongo-wallet` `submit_receipt` example using
the **public devnet key** — not suitable for funds or production
secrets):

```powershell
# 0. One-time: build and deploy the receipt tool from a PINNED commit
#    (the commit that contains crates/mbongo-wallet/examples/submit_receipt.rs;
#    re-run only after an approved tooling update, with the new commit SHA)
.\build-receipt-tool.ps1 -SourceCommit <full 40-hex commit sha>

# 1. Start (if not already running)
.\start-devnet.ps1

# 2. Submit one receipt (any fresh 64-hex-char task id)
.\submit-receipt.ps1 -TaskId 11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa

# 3. Verify inclusion on all three nodes
.\verify-receipt.ps1 -TaskId 11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa

# 4. Optional: deterministic duplicate-rejection test
.\verify-receipt.ps1 -TaskId 11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa11aa -DuplicateTest
```

### Receipt-tool provenance

The submission tool is a **deployed artifact** with the same discipline
as the node binary — `submit-receipt.ps1` never executes code from an
arbitrary working tree:

- `build-receipt-tool.ps1` builds `submit_receipt.exe` in a **clean git
  worktree pinned to an explicitly named commit** (mandatory
  `-SourceCommit`, full SHA; refuses dirty or mismatched trees) and
  copies it to `<DevnetRoot>\bin\submit_receipt.exe`.
- It stamps `<DevnetRoot>\receipt-tool-manifest.json` with: the tool
  source commit, the compatible protocol tag (`v0.3-devnet-stable`) and
  protocol commit, the absolute tool path, the SHA-256 of the deployed
  binary, and the build timestamp.
- Before **every** submission, `submit-receipt.ps1` verifies the
  manifest exists, the tool path matches, the protocol tag/commit match
  this deployment, and the recomputed SHA-256 matches — and refuses on
  any mismatch. It never silently rebuilds from the current branch.
- After an approved tooling change is committed, rebuild by re-running
  `build-receipt-tool.ps1 -SourceCommit <new sha>`; the manifest is
  restamped with the new provenance. Generated binaries and manifests
  live only under the deployment root, never in Git.

Details:

- The submitter reads the dev account's current nonce from the
  producer's REST API (`/accounts/{address}`) — never assumed.
- Each submission writes a record to
  `<DevnetRoot>\receipts\<task_id>.json` (task id, tx hash, nonce,
  sender, receipt hash, endpoint, timestamp). `submit-receipt.ps1`
  refuses a task id that already has a record unless `-AllowDuplicate`
  is passed (used only by the duplicate test).
- Verification scans blocks by height on **all three nodes**, requires
  the task id to appear **exactly once** at the **same height**
  everywhere, and reports per-node tips.
- **Verification limit:** there is no `get_receipt` RPC yet, so the
  scripts verify *inclusion of the anchoring transaction in the
  canonical chain*, not the stored receipt bytes. Byte-level
  canonicality is proven by the node test suite and the replay harness
  at the deployed tag; the duplicate-rejection test additionally proves
  the persistent receipt index is live.

## Backup

`backup-devnet.ps1 [-Label <name>]` creates a single verified ZIP under
`<DevnetRoot>\backups\` (timestamped, or named via `-Label`; existing
archives are never overwritten) plus a `.metadata.json` sidecar.

- **Requires all nodes stopped** (consistent RocksDB copy); refuses
  otherwise and names `stop-devnet.ps1`.
- **Included:** each node's `data\`, its `deployment.json` provenance
  marker, its `node.pid.json` (audit only), the latest log pair per
  node, `receipts\` submission records, and both manifests.
- **Excluded:** build worktrees, Cargo target directories, and deployed
  binaries — reproducible from the pinned commits recorded in the
  manifests.
- The sidecar records timestamp, protocol tag/commit, operational
  tooling commit, node binary and receipt-tool hashes, source root, the
  archive SHA-256 and verified entry count, a config snapshot, and the
  full file list.

## Reset (confirmed wipe to fresh genesis)

`reset-devnet.ps1 -ConfirmReset` — then type the **exact deployment
root path** at the prompt (or pass it via `-ConfirmRoot` in scripted
use). Nothing is ever wiped automatically:

- Refuses without `-ConfirmReset`, on any typed-confirmation mismatch,
  while any deployment node process is running, if the deployment
  manifest fails validation, or if any non-empty data directory lacks a
  matching provenance marker.
- Takes an **automatic verified backup first** (`pre-reset-<utc>`), and
  aborts the reset if that backup fails — unless the emergency override
  `-EmergencySkipBackupIUnderstandDataLoss` is explicitly supplied
  (there is deliberately no shorter alias).
- Removes only deployment-owned runtime data: node `data\`, `logs\`,
  PID metadata, provenance markers, and `receipts\` records (all
  captured in the pre-wipe backup). Preserves binaries, both manifests,
  all backups, and build sources. Every deletion path is re-validated
  to live strictly inside the deployment root.
- Recreates the empty directory structure and prints exactly what was
  removed and preserved. The next `start-devnet.ps1` begins from fresh
  genesis.

## Restore (manual, documented)

A backup is valid **only for the same protocol and storage version**
(`v0.3-devnet-stable`, schema v2). There is no v0.2 rollback.

1. Stop the devnet: `stop-devnet.ps1`.
2. Expand the archive:
   `Expand-Archive <backups>\devnet-backup-<name>.zip -DestinationPath <staging>`
   (optionally verify first: `Get-FileHash` against the sidecar's
   `archiveSha256`).
3. For each node, copy the restored `<node>\data` and
   `<node>\deployment.json` back into place (after moving aside or
   resetting the current ones).
4. **Delete or ignore any restored `node.pid.json`** — restored PID
   files are audit records, never live process state.
5. Restore `receipts\` if receipt records are wanted.
6. Confirm the deployment manifest still validates (the tag, commit,
   and binary hash checks run automatically on `start-devnet.ps1`).
7. Start the devnet; it resumes from the backup's persisted height, and
   `verify-receipt.ps1` confirms restored anchors.

---

## Soak observability

Long-running observation of a healthy devnet. Read-only: the soak
tooling never mutates chain data and never starts, stops, or restarts
node processes.

```powershell
# Start sampling (devnet must already be running and manifest-valid)
.\start-soak.ps1 -IntervalMinutes 5 [-PlannedHours 48] [-Label mysoak]

# One-shot sample outside a session loop (diagnostic)
.\soak-check.ps1 -SessionPath <session dir>

# Stop the sampler and generate the final report
.\stop-soak.ps1 -SessionPath <session dir>

# (Re)generate the report at any time
.\soak-report.ps1 -SessionPath <session dir>
```

Session layout under `<DevnetRoot>\soak\<session-id>\`:

| File | Contents |
|------|----------|
| `session.json` | immutable session metadata + threshold snapshot |
| `samples.csv` | one long-format CSV (3 `node` rows + 1 `session` row per sample) |
| `state.json` | previous-sample state (external, survives sampler restart) |
| `events.log` | noteworthy transitions (restart, RPC change, convergence change) |
| `soak.pid.json` | sampler PID + exe path + sampler script + session path |
| `sampler.out.log` / `sampler.err.log` | sampler process output |
| `final-report.json` / `final-report.txt` | summary + PASS/WARN/FAIL |

**Cadence:** default 5 min, minimum 1 min. Every RPC/REST probe has a
5-second timeout; individual node/probe failures are recorded as
values, never fatal — the sampler stops only on invalid session
metadata, an unsafe path, the planned duration, or `stop-soak.ps1`.

**Convergence classification** (per sample, from existing RPCs only):

- `converged` — all nodes reachable, equal heights, identical tip hashes.
- `temporarily-skewed` — heights differ by at most the allowance
  (default 1 block) and the block at the common minimum height is
  identical on every node (an ancestry check needing no local hashing).
- `stalled` — reachable and consistent, but the producer height did not
  advance since the previous sample.
- `divergent` — inconsistent tips (equal heights with differing hashes,
  or differing block JSON at the common minimum height) or a spread
  above the allowance.
- `unreachable` — one or more nodes did not answer RPC this sample.

Priority when several apply: unreachable > divergent > stalled >
temporarily-skewed > converged. Peer count is **not** observable (no
peer-count RPC exists); convergence is judged purely from height and
block data.

**Result criteria** (evaluated against the thresholds snapshotted into
`session.json` at start, so mid-soak edits have no effect):

- **FAIL** — any divergent sample; producer stalled streak ≥ 10 min;
  node RPC outage streak ≥ 15 min; max RSS ≥ 1500 MB; missing samples
  > 20%; log errors ≥ 25 in one interval.
- **PASS WITH WARNINGS** — no FAIL, but any stalled/unreachable sample,
  RPC outage ≥ 5 min, RSS ≥ 500 MB, data growth ≥ 50 MB/h, any log
  errors, ≥ 10 warnings/interval, missing samples > 5%, detected node
  restarts, or sampler interruptions.
- **PASS** — none of the above.

Thresholds are conservative devnet defaults (not SLA claims) and live
in `$SoakThresholds` in `devnet-config.ps1`. The report also lists what
is **not** observable this phase: peer count, receipt-index bytes, and
Prometheus counters.

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
- **No metrics endpoint:** no Prometheus/telemetry; soak observability
  is logs + RPC/REST polling (see the Soak section above).
- **Windows process stop is forceful** (no graceful shutdown signal);
  RocksDB's write-ahead log makes this safe, and the devnet harness
  exercises exactly this restart path.

## Future steps (not yet implemented)

1. Run the 48–72 h soak itself (tooling above is ready; the long run
   has not yet been executed)
2. Dedicated receipt RPC (`get_receipt`) enabling byte-level receipt
   verification from scripts
