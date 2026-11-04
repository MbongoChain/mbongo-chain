# Provider Containment on Linux (P2)

How a provider runs `provider_service` so that its external runner cannot read
the provider's keys or state. **Linux only.** No isolation is claimed on any
other platform, and `isolation.mode = required` refuses to start there.

The boundary protects the service from its runner. It does not protect the
host from the service, and it is not a sandbox for untrusted code: runners stay
provider-managed artifacts, identified by digest. Clients cannot supply a
binary. The threat is a provider-chosen runner compromised by what it processes.

## Supported configuration

| Requirement | Why |
|---|---|
| Linux 5.14 or later, cgroup v2 mounted at `/sys/fs/cgroup` | `cgroup.kill` |
| `memory` and `pids` controllers, swap accounting (`memory.swap.max`) | The memory and process limits |
| util-linux `setpriv`, coreutils at `/bin` | The launcher, and the startup probe |
| Local filesystems for keys and state | Same rule as P1 |

Tested on the CI runner, `ubuntu-24.04`, by the job
`Mbongo Provider Containment (Linux)`. Other distributions are untested.

## Start

```bash
sudo scripts/provider-containment-linux.sh prepare /srv/mbongo
```

This creates the accounts, the directories and the delegated cgroup, then
prints the ids the configuration needs. Put the keys in `/srv/mbongo/keys`, run
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
  "denied_paths": []
}
```

```bash
sudo scripts/provider-containment-linux.sh run /srv/mbongo /srv/mbongo/bin/provider_service /srv/mbongo/provider.json
```

`isolation` has no default. A configuration that omits it does not load, and
`{"mode": "none"}` isolates nothing and claims nothing.

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
| It cannot read, write or delete keys or state | Keys and state belong to the provider account, 0700 directories | Startup probe, then read, write and unlink of both keys, the journal and the lock from inside a job, all `EACCES` |
| It cannot read the service's memory or environment, or signal it | Different uid | `/proc/<pid>/environ`, `/proc/<pid>/mem`: `EACCES`. Signal: `EPERM` |
| It inherits no descriptor | `close_range` with `CLOSE_RANGE_CLOEXEC` before exec, whatever the descriptor's flags | A deliberately inheritable canary at startup; the service's sockets and files compared with the runner's, from outside |
| Descendants die with the job | One cgroup per job, `cgroup.kill` at the end of every job, success or failure | A process in its own session, with the parent exiting, then with the parent killed at the deadline |
| Memory is limited | `memory.max` from `max_memory_mib`, capped by the provider's ceiling, no swap, `memory.oom.group` | Under the limit: verified at T1, no OOM kill. Over it, and over the ceiling: killed, `oom_kill` rises |
| Processes are limited | `pids.max` | Set on every job. **Not exercised by a test** |
| It cannot leave its cgroup or change its limits | Interface files belong to the provider account | Moving itself out, raising `memory.max` and creating a child cgroup: `EACCES` |
| A crash leaves nothing running on restart | The service kills every leftover `job-*` cgroup and job directory, with the state lock held, before running anything | Real `SIGKILL` of the service, orphan observed alive, then gone; the task completes at T1 |
| It starts only if all of this holds | `Containment::establish` | 13 refusals, each reaching its own code |

### Refusals

The service exits with `ISOLATION_REFUSED=<code>`. There is no degraded mode.

| Code | Meaning |
|---|---|
| `UNSUPPORTED_PLATFORM` | Not Linux |
| `SERVICE_IDENTITY` | The service runs as root |
| `RUNNER_IDENTITY` | The runner is root, or shares the service's identity |
| `CGROUP_UNAVAILABLE` | No cgroup v2, controller not delegated, `cgroup.kill` missing, or the service is outside `<cgroup_root>/service` |
| `PROTECTED_PATH` | A key file, the state or the artifact directory has the wrong owner or mode |
| `WORK_DIR` | `work_dir` is not the service's, group runner, 2750 |
| `SPAWN_DENIED` | The service cannot start a process under the runner's identity |
| `PROBE_IDENTITY` | A contained process is not what was asked for |
| `PROBE_DESCRIPTORS` | A descriptor reached a contained process |
| `PROBE_READABLE` | A contained process read a protected path |
| `PROBE_INCONCLUSIVE` | A protected path failed for another reason than permission, such as not existing |
| `PROBE_KILL` | `cgroup.kill` did not kill a contained process |

A job that cannot be contained at runtime does not run: it fails with
`JOB_RUNTIME_FAILED`.

## What is not enforced

These are the provider's responsibility.

| Not provided | Consequence |
|---|---|
| **Network isolation** | The runner can reach the internet and every loopback service, including the node RPC and this service's HTTP API. It can send out anything it was given. Use firewall rules on the runner's uid, or a network namespace |
| **A private filesystem view** | The runner reads whatever is world-readable, and writes wherever its account can, such as `/tmp`. Files it leaves there persist and reach later jobs. There is no mount namespace, `chroot` or seccomp filter |
| **Isolation between jobs** | Jobs share one runner account. Only one runs at a time and no process survives a job, but files can |
| **CPU and disk limits** | A job can use all CPU until `max_wall_ms`, and fill any disk its account can write to |
| **Protection from the service** | The service holds `CAP_SETUID` and `CAP_SETGID`. A compromised service can become any uid, so the service is trusted. Its bounding set allows nothing else |
| **Kernel bugs** | The boundary is the kernel's. It is weaker than a VM |
| **Core dumps and swap of the service** | Keys are in its memory. Disable both if that matters |
| **Key protection from root or other accounts with access to the provider account** | Out of scope |

### Distinct failure codes

An out-of-memory kill reaches the client as `JOB_RUNTIME_FAILED`, which is
retried: the job contract has no memory code, and P2 does not change it. The
cgroup's `memory.events` shows the cause.

### The job directory

Under containment the job directory is read-only for the runner. Scratch space
is the provider's to arrange.

## Other platforms

On Windows and macOS, `required` refuses with `UNSUPPORTED_PLATFORM`. With
`none`, the runner runs under the service's account and can read its keys.
