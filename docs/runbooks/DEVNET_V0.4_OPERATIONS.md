# Devnet v0.4 Operations Runbook — the first Compute vertical

**Protocol authority:** [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md) (FROZEN)
**RPC authority:** [`rpc_v0.3.md`](../specs/rpc_v0.3.md) (FROZEN)
**Storage schema:** 3
**Source:** the `v0.4-devnet-stable` tag @ `fcec8ddc7b06247460e04db987de08232992e2fc` — the release commit carrying RFC 0005 at Released, PROTOCOL_LOCK_v0.4 and `@mbongo/sdk` 0.2.0; `dev` at or after it
**Profile:** LOCAL / DEVNET REFERENCE PROFILE — three native `mbongo-node` processes on one host, or the three-container Docker devnet. **Not a production topology.**
**Supersedes:** [`DEVNET_V0.3_OPERATIONS.md`](./DEVNET_V0.3_OPERATIONS.md) for everything Compute-related. That runbook's PowerShell deployment stays pinned to `v0.3-devnet-stable` and cannot run v0.4 (see [Known limitations](#known-limitations)).

> This is a **devnet** procedure. Do not use production secrets or real
> funds. Every key below is a **TEST / DEV KEY**: public, worthless, and
> printed in the source. An anchored receipt is a bound claim, not a proof
> that any computation was correct (RFC 0005 §9.2).

---

## What this runbook proves

From a clean machine, an operator can run the complete first Compute
vertical on the frozen v0.4 base:

```
fresh v0.4 network (3 nodes, fresh genesis, schema 3)
  → client funded (genesis dev account) → executor funded
  → private input registered off-chain (reference data plane)
  → ComputeTask submitted through the public SDK, included in a block, decoded back
  → reference control plane discovers the task
  → only the named executor is offered it; a squatter is refused by the node
  → lease → capability → input fetched → commitment verified → reference execution
  → private result persisted → bound Receipt built and signed by the executor
  → AnchorReceipt included → observed and verified through the SDK
  → private result retrieved by the owner only
```

The executable form of this runbook is the **Mbongo Compute v0.4 Vertical
Harness**. Its `PASS` line is the acceptance criterion; CI runs it on every
pull request and every push as the job **Mbongo Compute v0.4 Vertical**.

---

## Prerequisites

- Rust 1.94.0 (the version CI validates), a C/C++ toolchain for RocksDB.
- Node.js **24** (or ≥ 20.19.0) and npm — for the SDK part of the flow.
- Docker with Compose v2 — only for the Docker profile.
- A clean checkout of `dev` at or after the lock commit above.
- Ports free: harness profile `39944-39946` (RPC), `38080-38082` (REST), `40333-40335` (P2P); Docker profile `127.0.0.1:29944` on the host.

**Do not use `@mbongo/sdk` 0.1.0 for this.** It predates ComputeTask support
and cannot submit a task or decode a task-bearing block. `@mbongo/sdk` 0.2.0
is the first v0.4-capable release (the `sdk-typescript-v0.2.0` tag; confirm it
on the registry with `npm view @mbongo/sdk versions` before relying on it).
The vertical harness always uses the workspace SDK source, built locally, so
that the proof and the source agree byte for byte:

```bash
cd sdk/typescript && npm ci && npm run build && cd ../..
```

---

## Version visibility

Every node announces its authority on start-up, before it opens storage:

```text
Starting Mbongo Chain node...
  Protocol: PROTOCOL_LOCK_v0.4 (rpc_v0.3, storage schema 3)
```

`mbongo-node --version` prints the crate version (`0.1.0`), which is the
**same for v0.3 and v0.4 binaries** and therefore proves nothing about the
protocol. Use the `Protocol:` line, the git commit the binary was built
from, or the binary hash. The harness refuses any node that does not print
the v0.4 line and prints the BLAKE3 fingerprint of the one binary all three
nodes run.

There is no protocol-version field on the wire and no negotiation of one;
see [Mixed-version risk](#mixed-version-risk).

---

## Activation and reset

v0.4 activates on a **clean version boundary** (PROTOCOL_LOCK_v0.4 §10):
fresh genesis, wiped data directories, schema 3, no activation height, no
version flag, no wall-clock switch. **v0.3 chain history is not carried
forward**, and no in-place migration of a devnet is offered or supported.

| Profile | Where state lives | Reset | Stale v0.3 state |
|---|---|---|---|
| **Harness** (this runbook's primary path) | `<temp>/mbongo_compute_vertical/{producer,follower-a,follower-b}` | the harness deletes **only** that directory at start and end, and prints its path | cannot occur: the directory is created fresh on every run |
| **Docker devnet** | container writable layers; no volumes | `make devnet-down` removes containers and network; the next `make devnet-up` is fresh genesis | cannot persist across `down`; `up` without `down` reuses the running containers, which are already v0.4 |
| **PowerShell persistent devnet** (`scripts/devnet/*.ps1`, pinned to `v0.4-devnet-stable` @ `fcec8ddc`) | `C:\mbongo-devnet\v0.4\<node>\data` (or `MBONGO_DEVNET_ROOT`) | `reset-devnet.ps1 -ConfirmReset` + typed root, after a verified backup | **refused**: `start-devnet.ps1` runs only the pinned build and refuses any data directory whose `deployment.json` names another tag or commit (a v0.3 directory included); `reset-devnet.ps1` refuses too, so such a directory must be moved aside by hand |
| Production | — | — | out of scope; PROTOCOL_LOCK_v0.4 claims no production migration |

A v0.4 binary **can** open a v0.3 data directory (the storage migration is
additive and tested), but that is a storage fact, not an activation path:
the blocks in it were validated under v0.3 rules. Never point a v0.4 node
at a v0.3 directory. Delete it, or move it aside, and start fresh.

---

## Bootstrap and health gate

### Harness profile (native processes)

```bash
cargo build -p mbongo-node
cargo run -p mbongo-node --bin compute_vertical
```

The harness spawns the producer, reads its PeerId, spawns two followers
with that bootnode, and then applies the health gate before any Compute
step: every node answers `ping`; every node printed the v0.4 `Protocol:`
line; all three converge to the same height and tip hash at height ≥ 3
(the same `await_convergence` primitive the devnet harness and the Docker
probe use). There is no arbitrary sleep anywhere in readiness.

### PowerShell persistent devnet (Windows, native processes)

The persistent three-node deployment under `scripts/devnet/` is pinned to
`v0.4-devnet-stable` @ `fcec8ddc` and lives outside the repository
(default `C:\mbongo-devnet\v0.4`, or `MBONGO_DEVNET_ROOT`). Its procedures
are those of the v0.3 runbook, unchanged in mechanism: `start-devnet.ps1`
builds the tag in a clean worktree on first start, verifies commit, tag and
binary hash on every start, writes a `deployment.json` provenance marker per
node and refuses any data directory that carries another deployment's
marker; `status-devnet.ps1`, `stop-devnet.ps1`, `backup-devnet.ps1`,
`reset-devnet.ps1 -ConfirmReset` (verified backup first, typed root) are as
documented there. A fresh directory starts at genesis with storage schema 3.
Ports 9944-9946 / 8080-8082 / 30333-30335.

```powershell
cd scripts\devnet
.\start-devnet.ps1     # builds v0.4-devnet-stable once, then starts producer + followers
.\status-devnet.ps1
.\stop-devnet.ps1
```

Not v0.4-usable from that toolset: `submit-receipt.ps1` and
`build-receipt-tool.ps1`, which build the `submit_receipt` example — it
submits an **unbound** receipt, which v0.4 rejects on rule (q). Use the
vertical harness for a Compute smoke test; re-basing the receipt tool on a
committed task is tracked under #50.

### Docker profile (ephemeral)

```bash
make devnet-up      # build image, start 3 nodes, wait healthy, run convergence_probe
make devnet-down    # remove containers and network; next up is fresh genesis
```

`devnet-up` exits 0 only when all three containers are healthy **and** the
convergence probe passed. Confirm the authority line:

```bash
docker compose --env-file .env.base logs producer | grep "Protocol:"
```

The Docker profile is the bootstrap/convergence proof; the Compute flow
itself is proven by the harness profile above, which owns its own nodes so
that it can also restart one.

---

## Identities and funding

| Role | Seed (TEST / DEV KEY) | How funded |
|---|---|---|
| client / task submitter / result owner | `0xAA…AA` (32 bytes) — the code-baked genesis dev account | at genesis |
| executor (the only party allowed to answer) | `0xE1…E1` | transfer from the client, nonce 0 |
| squatter (adversarial executor) | `0xE9…E9` | transfer from the client, nonce 1 |
| stranger (unauthorised requester) | `0x5E…5E` | never funded; never on-chain |
| control-plane issuer (delegated capability issuer) | `0xC0…C0` | off-chain only; holds **no** executor key |

The harness performs the two funding transfers and waits for the client's
nonce to reach 2 before continuing. The task itself is submitted at nonce 2.

---

## The Compute flow, step by step

What the harness does, in the order the contracts require, with the
component that does it:

| Step | Component | What is checked |
|---|---|---|
| private input stored | reference data plane (in-process) | object id returned; the input never goes anywhere else |
| task registered for the executor; issuer delegated | data plane | executor immutable once registered |
| `ComputeTask` built, `task_id` derived, transaction signed and submitted | **TypeScript SDK** (`scripts/compute-vertical.mjs submit`) via rpc_v0.3 | the SDK's `task_id` equals `mbongo-core`'s |
| task committed and decoded back | SDK `getBlockByHeight` + `computeTasksInBlock` | same executor, input commitment, spec, submitter |
| task observed on all nodes and by the control plane | control plane `observe`; `scan_tasks` on each follower | one interpretation everywhere |
| wrong executor | squatter worker + squatter receipt | not offered; node refuses on rule (s) |
| corrupted input | worker fault `CorruptInput` | `Failed(Input)`, no result, no receipt |
| result-store failure | data-plane fault `PutResultFails` | `Failed(Persistence)`, no result, no receipt |
| worker crash after fetch | worker fault `CrashAfterFetch`, then a new instance after the lease lapses | input marked consumed; new attempt with fresh capabilities; old grant never reopened (conformance C22) |
| lease → capability → fetch → verify → execute → persist → receipt | worker (holds the executor key) | result reference exists at submission time; receipt commits to it |
| receipt anchored and verified | **SDK** (`observe`): `receiptsInBlock`, `verifyReceiptSignature`, `assertReceiptBoundToTask`, `signBoundReceipt` comparison | bound to `task_id` and input commitment; executor and anchor sender are the named executor; bytes identical to the SDK-built receipt |
| completion derived | control plane `observe` | `Completed`; a further worker pass is idle |
| private result retrieved | data plane get-result grant | owner ok; stranger with the owner's grant refused; `task_id`-only refused for input and result |
| privacy scan | every block on every node, SCALE and RPC JSON | no input/output bytes in raw, hex or JSON-array form; spec is the public tag; receipt metadata empty |
| control-plane restart | `snapshot` → `restore` → `observe` | executor unchanged, completion reconstructed, attempts kept, no re-offer, one receipt |
| node restart | follower-a killed and respawned on its directory | reopens, rejoins, converges past the pre-restart tip; task and anchor blocks identical to the producer's |
| data-plane restart | fresh in-memory instance | **holds nothing** — the documented limitation |
| log privacy | all node stdout/stderr, control-plane/worker/data-plane logs, SDK output | no input, output or key seed in any form |

The harness ends with a `VERTICAL_TRACE` block (transaction hashes, task
id, heights, commitments, object ids — never payloads or keys) and the line
`MBONGO COMPUTE V0.4 VERTICAL: PASS`.

### The reference components, plainly

- **Control plane** — the G reference `ControlPlane`, run **in the harness
  process**; there is no daemon. It discovers tasks by scanning blocks
  through `get_block_by_height` (no task lookup RPC exists), admits a
  worker by a proof of possession of the executor key, issues 5-second
  leases renewed by heartbeat, and obtains data-plane capabilities as the
  client's delegated issuer. Durable: task records and attempt counters
  (`DurableState`). Not durable, by design: sessions and leases (a restart
  expires them). Chain-derived facts are a rebuildable cache.
- **Data plane** — the G reference `InMemoryDataPlane`, in the same
  process. Private objects, capabilities and results live in memory and
  **do not survive a process restart**. Non-production. Cleared by ending
  the process. A persistent backend is a follow-up behind the same traits.
- **Worker** — the G reference `Worker`, in the same process, holding the
  executor key (seed above), running the **REFERENCE / TEST EXECUTION
  PROFILE** `mbongo-ref:reverse-bytes:v1` (reverse the bytes). It takes no
  raw input on any command line: input reaches it only through a
  data-plane capability. Chain access is rpc_v0.3 (`http://127.0.0.1:39944/rpc`) and
  the REST nonce read (`http://127.0.0.1:38080`).
- **SDK** — the workspace `@mbongo/sdk` source built into `sdk/typescript/dist`,
  driven by `scripts/compute-vertical.mjs`, which reads its parameters on
  stdin and prints JSON on stdout. It never prints keys or payloads.

None of these is consensus (PROTOCOL_LOCK_v0.4, non-consensus reference
material). Leases, attempts, capabilities and the profile are off-chain.

---

## Restart matrix

| Component | Tested by the harness | What survives | What the operator does |
|---|---|---|---|
| node (follower) | yes — killed and respawned on its schema-3 directory | all chain state: blocks, tasks, receipts | restart the process with the same `--data-dir`; it catches up and serves the same blocks |
| node (producer) | by the devnet convergence harness (push CI) | same | same; followers reconnect |
| control plane | yes — snapshot/restore | task records, attempt counters; completion re-derived from the chain | restore from its snapshot; leases are gone, workers re-authenticate; **no executor is ever reassigned** |
| worker | yes — crash after fetch, recovered by a new instance | nothing in the process; the data plane remembers the spent capability | start a new instance; it gets a new lease and a fresh capability; if the result was already persisted it is reused, and if a receipt was already submitted the lookup-first rule applies |
| data plane (in-memory) | yes — a fresh instance holds nothing | **nothing**: `DATA_PLANE_RESTART_SURVIVAL=NO` | this is a backend limitation, not a protocol failure; chain state is intact; re-register the input for a new task, or use a persistent backend when one exists |

---

## Failure runbook

Protocol verdicts come from the node (block application and admission) and
are final; operational errors come from the reference components and are
retried under the contracts.

| Symptom | Kind | What it means | Response |
|---|---|---|---|
| task committed, worker idle forever | operational | no input object registered with the control plane, or the spec tag is not one the worker claims | register the input (`register_input`); check the worker's profile tag against `execution_spec` |
| worker idle although a task exists | operational | the worker's key is not `task.executor` | the task names another executor; there is no reassignment — a different executor is a different task |
| `receipt executor is not authorised by the task` (`-32603`) | protocol, rule (s) | someone other than the named executor tried to anchor | expected refusal; nothing to fix on the node |
| `receipt task_id is not a registered task` (`-32603`) | protocol, rule (q) | anchoring before the task, or an unbound receipt | commit the task first; wait for its block |
| `receipt input_commitment does not match the task` | protocol, rule (r) | the receipt claims an input the submitter did not commit | the worker verified the wrong input; check the data-plane object |
| worker reports `Failed(Input)` | operational | fetched bytes do not match `input_commitment` | the stored object is wrong or corrupted; fix it off-chain; nothing executed, nothing anchored |
| worker reports `Failed(Persistence)` | operational | result store failed | retry; no receipt was produced |
| worker reports `Failed(Receipt)` | protocol or transport | the node rejected the anchor (see messages above) or was unreachable | read the message; see [ambiguous submission](#ambiguous-receipt-submission) |
| worker unavailable / lease expired | operational | heartbeat stopped | the lease lapses (5 s here); a new instance is offered the task; capabilities issued under the old lease are revoked |
| `task_id already registered` / `compute task already pending` | protocol, rule (p) | same envelope committed twice | change the salt if the work must be repeated |
| `task_id already anchored` / `task_id already pending` | protocol, rules (i)/(j) | a receipt for the task already exists or is pending | first-anchored-wins; do not resubmit a different receipt |
| node fails to start: `schema version N is newer than supported` | operational | the directory was written by a newer binary | use that binary, or wipe |
| node fails to start: `unknown column family` | operational | foreign or newer layout | wipe |
| a v0.3 node stalls when following this network | mixed version | it cannot decode payload index 2 | it must not be on this network; see below |
| a v0.4 node rejects a block with `TypePayloadMismatch` or `TaskNotRegistered` that another node accepted | mixed version | the other node is v0.3 | same |

### Ambiguous receipt submission

The worker follows E §11.2 and G: **lookup first, then resubmit the same
signed bytes if absent, never build a second distinct receipt.** If the
submission response was lost, the worker's next pass scans blocks from
the task's height for a receipt with its `task_id` (`find_receipt`); if
one exists the task is complete; if not, it resubmits the identical
transaction (the node is idempotent for a pending duplicate and rejects an
already-anchored one with `task_id already anchored`). An operator checks
the same way: read blocks from the task height and look for the
`AnchorReceipt` carrying the `task_id` — for example with the SDK's
`receiptsInBlock`, or the harness's `observe` step. This path is proven
deterministically by conformance case C38 and G's `ambiguous_submission…`
test; it is not fault-injected against a live node.

---

## Mixed-version risk

PROTOCOL_LOCK_v0.4 §9 records that the P2P negotiation strings
(`/mbongo-sync/2`, `/mbongo/block_notify/0.2.0`) and the identify string
(`/mbongo/0.3.0`) were **not** bumped for v0.4. A v0.3 node and a v0.4 node
therefore negotiate successfully and are separated only by deterministic
failures afterwards: the v0.3 node cannot decode a block carrying a
`ComputeTask` and stops; the v0.4 node rejects blocks that are valid only
under v0.3 rules. **A mixed network can look healthy until the first task
or receipt.** This gap is not resolved here (bumping the strings is a
locked-surface change that needs its own RFC or addendum).

Operational mitigation in this runbook:

- every profile deploys **one binary** to all nodes: the harness prints its
  BLAKE3 fingerprint and refuses any node that does not print the v0.4
  `Protocol:` line; the Docker devnet builds one image; the PowerShell
  deployment hash-verifies one binary against its manifest;
- bootnodes are only the deployment's own producer — never point a v0.4
  node at an external v0.3 network, or the reverse;
- if a node stalls at a height while others advance, or rejects a block
  the others accepted, suspect a version mismatch first: read its
  `Protocol:` line.

---

## Shutdown

- Harness: it kills its nodes and deletes its own directory on exit, pass
  or fail.
- Docker: `make devnet-down`.
- PowerShell (v0.3 deployment): `stop-devnet.ps1`.

---

## Known limitations

- **Published SDK.** `@mbongo/sdk` 0.1.0 is not v0.4-capable; 0.2.0 is. The
  vertical uses the workspace source regardless.
- **PowerShell receipt tool.** `submit-receipt.ps1` builds an unbound
  receipt and cannot anchor under v0.4 (rule q); the deployment itself is
  pinned to `v0.4-devnet-stable`.
- **In-memory data plane.** Private inputs and results do not survive a
  process restart. Chain state does.
- **Single process for control plane, data plane and worker.** No network
  service, no daemon, no multi-worker scheduling; the reference exists to
  prove the contracts, not to operate at scale.
- **Reference profile only.** Reverse-bytes is a test transform; no CPU,
  GPU or AI execution exists, and none is required.
- **Ambiguous-submission recovery** is proven deterministically (G, C38),
  not by fault injection against a live node.
- **Docker profile** proves bootstrap and convergence; the Compute flow is
  proven by the harness profile, which needs its own nodes to restart one.

---

## Troubleshooting

- `node binary not found` — run `cargo build -p mbongo-node` first; the
  harness looks next to its own executable.
- `workspace SDK is not built` — `cd sdk/typescript && npm ci && npm run build`.
- `cannot run node` — Node.js is not on `PATH`.
- `port … is already in use` — another devnet or harness is running; the
  operational devnet uses 9944-9946, the devnet harness 19944-19946, Docker
  29944, this harness 39944-39946.
- `did not announce PROTOCOL_LOCK_v0.4` — the binary next to the harness is
  not built from this source; rebuild.
- Node logs are kept in memory for the log scan and printed only on
  failure; run with `RUST_LOG=info` on the harness itself for the
  control-plane and worker lines.
