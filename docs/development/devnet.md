# Devnet Developer Guide

How to run, inspect and tear down the three-node Mbongo devnet on your own
machine. For why it is built this way, see
[Deterministic Devnet Architecture](../architecture/devnet-infrastructure.md).

## Prerequisites

- **Docker**, with **Compose v2** — the tooling calls `docker compose`
  (the space-separated subcommand), not the older `docker-compose` binary.
- **GNU Make**, if you want the `make` shortcuts. Everything is also reachable
  by calling the script directly, so Make is convenient rather than required.
- Nothing else: the Rust workspace is compiled **inside** the image, so you do
  not need a local toolchain to run the devnet.

No minimum versions are pinned by the repository. CI runs this on
`ubuntu-latest`, and the workflow prints the versions it found before booting,
so a job log is the reference for what is known to work.

## Quick Start

From a clean checkout:

```bash
make devnet-up
```

When you are done:

```bash
make devnet-down
```

Without Make, identically:

```bash
./scripts/devnet/docker-devnet.sh up
./scripts/devnet/docker-devnet.sh down
```

The Make targets are two-line delegations to that script, so the two forms run
exactly the same bootstrap.

`make devnet-up` exits `0` only if all three nodes became healthy **and** the
convergence check passed. On failure it prints diagnostics and exits non-zero.

## What `devnet-up` Does

```
make devnet-up
  └─ scripts/devnet/docker-devnet.sh up
       ├─ docker compose up --detach --build --wait   (producer, follower-a, follower-b)
       │    └─ waits until every container reports healthy (bounded)
       └─ docker compose run --rm convergence-check
            └─ convergence_probe  →  exit 0 / non-zero
```

The script builds the image if needed, starts the three nodes, and blocks
until Compose reports them healthy — there is no `sleep` anywhere in the path.
It then runs the one-shot `convergence-check` service, whose verdict is the
verdict of the whole command.

The health wait is bounded by `DEVNET_WAIT_TIMEOUT` (default 420 seconds), and
the probe's own readiness and convergence waits are bounded by the environment
layer in use.

## Environment Selection

`DEVNET_ENV` selects which layer is applied on top of `.env.base`:

| Value | Effect |
|---|---|
| unset or `local` | applies `.env.local` **if the file exists** |
| `ci` | applies `.env.ci` |
| anything else | rejected, exit code 2 |

```bash
DEVNET_ENV=ci ./scripts/devnet/docker-devnet.sh up
```

Compose applies `--env-file` arguments left to right, so the second layer
overrides `.env.base`. `ci` and `local` are mutually exclusive: selecting `ci`
does not read `.env.local`.

## Local Overrides

`.env.base` is versioned and sufficient on its own — a fresh checkout boots
with no extra setup. To change something for yourself, create `.env.local`
with only the keys you want to override, and never edit `.env.base`:

```bash
echo "MBONGO_HOST_RPC_PORT=31944" >  .env.local
echo "MBONGO_BLOCK_TIME=2"        >> .env.local
```

`.env.local` is gitignored, so it will not be committed by accident. It is
optional; the script adds that layer only when the file is present.

Do not put secrets in any of these files. The devnet needs none, and
`.env.base` and `.env.ci` are versioned and public.

## CI Environment

`.env.ci` is versioned and holds the overrides used by the
`Docker Devnet Bootstrap` job: longer readiness and convergence timeouts for
slower shared runners, a quieter log level, and `MBONGO_HOST_RPC_PORT=0` so
Docker picks a free host port instead of colliding with a concurrent job.

The CI job runs `DEVNET_ENV=ci make devnet-up`, i.e. the same entrypoint you
run locally, and always runs `DEVNET_ENV=ci make devnet-down` afterwards.

## Checking the Network

**Only one port is published to your host**: the producer JSON-RPC, bound to
loopback, at `127.0.0.1:${MBONGO_HOST_RPC_PORT}` (default `29944`). That port
was chosen to avoid the operational devnet (9944-9946) and the in-process
harness (19944-19946).

```bash
curl -X POST -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"get_block_height","id":1}' \
  http://127.0.0.1:29944/rpc
```

The follower RPC endpoints, every REST surface and every P2P port are **not**
published; they are reachable only from inside the Compose network. To query a
follower, go through a container:

```bash
docker compose --env-file .env.base exec follower-a \
  curl -s -X POST -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"get_block_height","id":1}' \
  http://127.0.0.1:9944/rpc
```

Container state and logs:

```bash
./scripts/devnet/docker-devnet.sh status
docker compose --env-file .env.base logs -f producer
```

## Stopping and Cleaning Up

```bash
make devnet-down
```

This runs `docker compose down --volumes --remove-orphans`, removing the three
containers and the devnet network. It is safe to run when nothing is up and
safe to run twice — `down` on an absent project is a no-op that still exits
`0`.

The devnet declares no volumes, so node state lives in the container writable
layers and disappears with them. Every `up` after a `down` therefore starts
from a fresh genesis. (Running `up` again without `down` reuses the existing
containers, and their state with them.) The built image
(`mbongo-devnet:local`) is deliberately **not** removed, so the next boot does
not rebuild from scratch; remove it by hand if you want a cold build:

```bash
docker rmi mbongo-devnet:local
```

## Troubleshooting

**Docker or Compose unavailable.** The script calls `docker compose`; if the
daemon is not running or only the legacy `docker-compose` binary is installed,
the first Compose command fails immediately. Check with
`docker --version` and `docker compose version`.

**Health timeout.** If a node never reports healthy, `up` fails after
`DEVNET_WAIT_TIMEOUT` and the script prints `docker compose ps --all`, the last
60 log lines of each node, and each container's status and health status. Read
the producer log first: followers depend on it becoming healthy before they
start.

**Convergence failure.** The probe's error names the required height floor, the
last observed height and tip hash for **each** endpoint, the elapsed time and
the number of attempts. Nodes that agree but do not advance fail too: by
default the probe requires a new block after every node is ready, so a stalled
producer is a failure, not a pass.

**Readiness failure inside the probe.** The error names every endpoint that
never answered `ping`, with its URL and last transport error — that is how you
tell "a node is down" apart from "the nodes disagree".

**Host port collision.** If `${MBONGO_HOST_RPC_PORT}` is already taken, the
producer container fails to start. Override it in `.env.local`, or set it to
`0` to let Docker choose a free port.

**Subnet conflict.** The devnet claims `${MBONGO_SUBNET}` (default
`172.28.53.0/24`) for its own bridge network. If that range clashes with
something else on your machine, network creation fails; override
`MBONGO_SUBNET` and `MBONGO_PRODUCER_IP` together in `.env.local`, keeping the
producer address inside the subnet.

**Leftover state.** `./scripts/devnet/docker-devnet.sh status` shows whether
containers still exist; `make devnet-down` clears them.

## Native Harness

The Docker devnet is not the only way to exercise a network. The Rust harness
starts its own nodes as child processes and additionally covers scenarios a
static network cannot: it kills and restarts the producer, kills and restarts a
follower, and submits receipt-anchoring traffic.

```bash
cargo run -p mbongo-node --bin devnet_harness
cargo run -p mbongo-node --bin replay_harness
```

Use the native harness when you are changing node behaviour — sync, restart
recovery, receipt handling — and want a fast in-process loop with a local
toolchain. Use the Docker devnet when you care about the packaged image, the
service graph, container readiness, or reproducing what CI does.

Both decide "converged" using the same shared code, so they cannot disagree
about the invariant itself.

`convergence_probe` can also be pointed at any already-running nodes, including
ones you started by hand:

```bash
cargo run -p mbongo-node --bin convergence_probe -- \
  --endpoint producer=http://127.0.0.1:9944 \
  --endpoint follower-a=http://127.0.0.1:9945
```

It never starts or stops a node, and it requires at least two endpoints.

## CI

Three jobs cover different surfaces
([.github/workflows/ci.yml](../../.github/workflows/ci.yml)):

| Job | Triggers | Covers |
|---|---|---|
| `Rust Checks (fmt, clippy, test, replay)` | PR to `dev`, push to `dev` | fmt, clippy, unit tests, replay harness |
| `Devnet Convergence Harness` | push to `dev` only | native process-mode harness, including restarts |
| `Docker Devnet Bootstrap` | PR to `dev`, push to `dev` | the Docker path via `DEVNET_ENV=ci make devnet-up` |

Spelling runs in a separate workflow, on pull requests only.

Practical consequence: opening a PR exercises the Docker bootstrap but not the
native restart scenarios; those run when the change lands on `dev`. If you are
touching restart or sync behaviour, run `devnet_harness` locally before
merging.

## Making Changes Safely

- **Do not reimplement convergence.** Height and tip-hash comparison belongs in
  `mbongo_node::convergence` only — never in Compose, the Makefile, a
  healthcheck, or a workflow step.
- **Keep healthchecks about readiness.** A healthcheck answers "does this node
  serve RPC yet", not "do the nodes agree".
- **Validate both paths** when you touch node startup, networking or sync:
  `cargo run -p mbongo-node --bin devnet_harness` and `make devnet-up`.
- **Keep local and CI on the same entrypoint.** If CI needs something new, add
  it to `docker-devnet.sh` or `.env.ci`, not as a CI-only command.
- **Respect the env layering.** New knobs go in `.env.base` with a default that
  works from a clean checkout; `.env.local` must stay optional.
- **Leave bind defaults alone.** `--rpc-host` and `--rest-host` default to
  `127.0.0.1`; only container configuration opts into `0.0.0.0`.
- **Keep the devnet stateless.** Adding a volume would make state survive
  `down`.
- **If you change the Rust version**, update it in both `.env.base` (image
  build) and `.github/workflows/ci.yml` (CI), which cannot share a value
  because `uses:` does not accept expressions.
