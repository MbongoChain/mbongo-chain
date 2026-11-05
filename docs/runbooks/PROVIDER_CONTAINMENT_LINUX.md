# Provider Containment on Linux (P2)

How a provider runs `provider_service` so that its external runner cannot read
the provider's keys or state. **Linux only.** No isolation is claimed on any
other platform, and `isolation.mode = required` refuses to start there.

The boundary protects the service from its runner. It does not protect the
host from the service, and it is not a sandbox for untrusted code: runners stay
provider-managed, and clients cannot supply a binary. `binary_sha256` pins one
when the provider sets it, and is checked at startup only. The threat is a
provider-chosen runner compromised by what it processes.

## Supported configuration

| Requirement | Why |
|---|---|
| Linux 5.16 or later, cgroup v2 mounted at `/sys/fs/cgroup` | `cgroup.kill`, and open-time credentials for joining a cgroup. Older kernels refuse |
| `memory` and `pids` controllers, swap accounting (`memory.swap.max`) | The memory and process limits |
| util-linux `setpriv`, coreutils at `/bin`, `/bin/bash` with `/dev/tcp` | The launcher, and the startup probes |
| `nftables` with `inet` tables, `meta skuid` and `reject` | The runner's network policy |
| A runner account used by nothing else, with no login shell | The policy follows the uid |
| Local filesystems for keys and state | Same rule as P1 |

Tested on the CI runner, `ubuntu-24.04`, by the job
`Mbongo Provider Containment (Linux)`. Other distributions are untested.

## Start

```bash
sudo MBONGO_INBOUND_ALLOW=22 scripts/provider-containment-linux.sh prepare /srv/mbongo
```

This creates the accounts, the directories, the delegated cgroup and the
firewall, then prints the ids the configuration needs. Put the keys in `/srv/mbongo/keys`, run
`prepare` again to set their modes, and configure the service:

```json
"executor_key_file": "/srv/mbongo/keys/executor.key",
"state_key_file": "/srv/mbongo/keys/state.key",
"state_dir": "/srv/mbongo/state",
"artifact_dir": "/srv/mbongo/artifacts",
"work_dir": "/srv/mbongo/work",
"isolation": {
  "mode": "required",
  "runner_uid": 991,
  "runner_gid": 991,
  "cgroup_root": "/sys/fs/cgroup/mbongo-provider",
  "max_memory_mib": 2048,
  "max_pids": 256,
  "network": "denied",
  "denied_paths": []
}
```

```bash
sudo MBONGO_INBOUND_ALLOW=22 scripts/provider-containment-linux.sh run /srv/mbongo /srv/mbongo/bin/provider_service /srv/mbongo/provider.json
```

`isolation` and its `network` field have no default. `"network": "open"` leaves
the runner's network unrestricted, and needs `MBONGO_RUNNER_NETWORK=open` for the
launcher. A configuration that omits either does not load, and
`{"mode": "none"}` isolates nothing and claims nothing. It is for controlled
local testing only.

The binary, the runner and its interpreter must be readable and executable by
the two accounts, and must not be writable by the runner account. Nothing
checks that last point: it is the provider's to get right.

## What is enforced

Each row is proven by `provider_harness --contained`, through the real job
path, on the configuration above.

| Protection | Mechanism | Evidence |
|---|---|---|
| The runner has its own identity | `setuid`, `setgid`, no supplementary groups | Read back from `/proc/self/status`, at startup and inside a job |
| It holds no capability and cannot gain one | All capability sets cleared, `no_new_privs` | Same. `setuid(0)` from the runner fails with `EPERM` |
| It cannot read, write or delete keys or state | Keys and state belong to the provider account, 0700 directories, under parents only root or that account can write | Startup probe, then read, write and unlink of both keys, the journal and the lock from inside a job, all `EACCES` |
| It cannot read the service's memory or environment, or signal it | Different uid | `/proc/<pid>/environ`, `/proc/<pid>/mem`: `EACCES`. Signal: `EPERM` |
| It leaves the service's session and terminal | `setsid` | The runner reads itself as a session leader |
| It inherits no descriptor | `close_range` with `CLOSE_RANGE_CLOEXEC` before exec, whatever the descriptor's flags | A deliberately inheritable canary at startup; the service's sockets and files compared with the runner's, from outside |
| Descendants die with the job | One cgroup per job, `cgroup.kill` at the end of every job, success or failure. If they cannot be killed, no further job runs | A process in its own session, with the parent exiting, then with the parent killed at the deadline |
| Memory is limited | `memory.max` from `max_memory_mib`, capped by the provider's ceiling, no swap, `memory.oom.group` | Under the limit: verified at T1, no OOM kill. Over it, and over the ceiling: killed, `oom_kill` rises |
| Processes are limited | `pids.max` | A job attempting three times the limit: stopped at it, `pids.events` records the denials, no process left |
| It cannot leave its cgroup or change its limits | Interface files belong to the provider account, and the job's cgroup directory is closed to the runner | Moving itself out, raising `memory.max` and creating a child cgroup: `EACCES` |
| `SIGTERM` or Ctrl-C leaves no job running | The service refuses further jobs, kills each running job's cgroup, and exits 0. The task stays unfinished in the journal, and the next start runs it again | A job and its detached descendant killed, exit 0, no cgroup left, the task completed at T1 after a restart |
| A restart leaves nothing from before it running | With the state lock held, the service kills every leftover `job-*` cgroup before it runs anything, and removes their job directories | Real `SIGKILL` of the service, orphan observed alive, then gone; the task completes at T1 |
| With `network: denied`, its traffic is refused | An `nftables` rule on the runner's uid, in an `inet` table, installed by the launcher as root before the service starts. The service gains no capability, and cannot read the rule | From the runner and from a detached descendant: TCP and UDP over IPv4 and IPv6, an IPv4 address written in IPv6, the host's own address, the node RPC and REST ports, the service, and an outside address where one is reachable. Nothing arrived at the real endpoints. Without the rule, the same job reached all of them |
| The network policy is proven, not assumed | Before anything runs, the runner must fail to reach a loopback listener, over IPv4 and IPv6, that the service's own account reaches with the same command. Repeated before every job | Policy absent, naming another account, letting loopback or IPv6 through: refused at startup. Removed or changed while running: no runner starts, and none does until a restart, even once repaired |
| The runner's account is not shared | At startup, any process under its uid refuses the start. The launcher refuses an account with a login shell | A foreign process under that uid: refused |
| It starts only if all of this holds | `Containment::establish` | 23 refusal cases, over 14 of the codes below. `UNSUPPORTED_PLATFORM` is tested on Windows. The three probe failures are produced by replacing the probe's tool in a private mount namespace, which tests the service's decision, not a real kernel fault |

### Refusals

The service exits with `ISOLATION_REFUSED=<code>`. There is no degraded mode.

| Code | Meaning |
|---|---|
| `UNSUPPORTED_PLATFORM` | Not Linux |
| `SERVICE_IDENTITY` | The service runs as root |
| `RUNNER_IDENTITY` | The runner is root, or shares the service's identity |
| `CGROUP_UNAVAILABLE` | No cgroup v2, controller not delegated, `cgroup.kill` missing, or the service is outside `<cgroup_root>/service` |
| `PROTECTED_PATH` | A key file, the state, the artifact or the work directory has the wrong owner or mode, is a symbolic link, or sits under a directory others can write |
| `WORK_DIR` | `work_dir` is not the service's, group runner, 2750 |
| `SPAWN_DENIED` | The service cannot start a process under the runner's identity |
| `PROBE_IDENTITY` | A contained process is not what was asked for |
| `PROBE_DESCRIPTORS` | A descriptor reached a contained process |
| `PROBE_READABLE` | A contained process read a protected path |
| `PROBE_INCONCLUSIVE` | A protected path failed for another reason than permission, such as not existing |
| `PROBE_KILL` | `cgroup.kill` did not kill a contained process |
| `RUNNER_UID_SHARED` | A process already runs under the runner's account |
| `NETWORK_POLICY_INEFFECTIVE` | A contained process reached a loopback listener |
| `NETWORK_PROBE_INCONCLUSIVE` | The control did not connect, or the probe did not run |

A job that cannot be contained at runtime does not run: it fails with
`JOB_RUNTIME_FAILED`.

## What is not enforced

These are the provider's responsibility.

| Not provided | Consequence |
|---|---|
| **Complete network isolation** | With `network: open`, nothing is restricted. With `denied`, only IP traffic from sockets the runner's account opened is refused. See below |
| **IPC isolation** | Unix-domain sockets, abstract ones included, are not IP traffic and no rule covers them. A local daemon listening on one, such as a proxy or a resolver, can carry traffic out for the runner. A socket received from another account keeps that account's identity. Pipes, shared memory, message queues and files follow the account's permissions only |
| **A private filesystem view** | The runner reads whatever is world-readable, and writes wherever its account can, such as `/tmp`. Files it leaves there persist and reach later jobs. There is no mount namespace, `chroot` or seccomp filter |
| **Isolation between jobs** | Jobs share one runner account. The worker starts one at a time and no process survives a job, but files can, and a job past its deadline can briefly overlap the next |
| **Memory held outside a job** | `memory.max` ends with the job. What it left in `/dev/shm`, another tmpfs or System V shared memory stays, and repeated jobs can exhaust the host |
| **Running jobs when the service dies** | `SIGKILL`, a crash or power loss leaves a running job alive with no deadline until the next start kills it. To end it sooner, write `1` to `<cgroup_root>/cgroup.kill` |
| **CPU and disk limits** | A job can use all CPU until `max_wall_ms`, and fill any disk its account can write to |
| **Protection from the service** | The service holds `CAP_SETUID` and `CAP_SETGID`. A compromised service can become any uid, so the service is trusted. Its bounding set allows nothing else |
| **Inbound traffic, and packets without an owner** | The launcher's inbound policy drops new connections to any port not listed, and the remote-namespace test shows a port the runner listens on is unreachable. A listed port stays open to whatever listens on it. The service cannot check this policy: [`PROVIDER_TLS_REFERENCE_LINUX.md`](PROVIDER_TLS_REFERENCE_LINUX.md). With only the output rule, the same port was also unreachable in testing, but that is observed, and the kernel may send packets with no owning socket, which the rule would not match |
| **Continuous network enforcement** | The policy is checked at startup and before each job. Removed during a job, it is not seen until the next one. The check proves loopback only: a rule changed to let other destinations through is not detected |
| **Unsupported hosts** | The rule lives in one network namespace, the service's. A `/proc` mounted with `hidepid` hides the shared-account check. Other firewalls, container runtimes and hosts without `nftables` are untested. An administrator can remove the rule |
| **Kernel bugs** | The boundary is the kernel's. It is weaker than a VM |
| **Core dumps and swap of the service** | Keys are in its memory. Disable both if that matters |
| **Key protection from root or other accounts with access to the provider account** | Out of scope |

### Distinct failure codes

An out-of-memory kill reaches the client as `JOB_RUNTIME_FAILED`, which is
retried: the job contract has no memory code, and P2 does not change it. The
provider's log records the cause: `memory limit reached`, with the job.

At the deadline, `JOB_LIMIT_WALL_TIME` can be reported up to 50 ms before the
job's processes are killed.

### The job directory

Under containment the job directory is read-only for the runner. Scratch space
is the provider's to arrange.

## Other platforms

On Windows and macOS, `required` refuses with `UNSUPPORTED_PLATFORM`. With
`none`, the runner runs under the service's account and can read its keys.
