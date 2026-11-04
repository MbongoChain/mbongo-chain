# P3: Provider Network Security and Interoperability

**Status: PROPOSED. Not approved, and nothing here is implemented.** The service
stays loopback-only and no provider is deployed until this is approved, and then
only as approved. It replaces the P3 row of
[`G2_MBONGO_JOB_V1_PLAN.md`](G2_MBONGO_JOB_V1_PLAN.md) §10 if adopted.

**Goal:** a provider on another host can be reached securely, can hold a runner
without network access, and can show that it conforms. No new workload.

**Protected surfaces:** those of the G2 plan §9.11. P3 needs no protocol,
consensus, RPC or SDK change.

## 1. Starting point

| Exists | Missing |
|---|---|
| Requests are signed and bound to one recipient (`mbongo-provider-http:v1`), with persistent replay protection (P1) | Requests and responses are readable and alterable on the wire. A client cannot tell it reached the right host |
| Results are checked against the on-chain receipt, up to T1 | Status and error responses are not authenticated |
| On Linux the runner is contained (P2) | The runner has full network access |
| `compute-conformance-v1` covers the components the service reuses | It does not run against `ProviderService` (S2-10, #191) |
| Everything runs on one host | Nothing has crossed a real network |

## 2. Tasks

Each task is one PR. The order is P3-A, P3-B, P3-C, P3-D.

### P3-A: Conformance adapter (#191)

| | |
|---|---|
| Capability | The conformance suite runs against the real provider service |
| Component | One `Subject` adapter, in tests only |
| Left to others | Nothing |
| Minimal tests | `run_all` passes. Any case the service cannot drive is listed by name |
| Excluded | New cases, suite changes, certification |

It needs no network work and closes S2-10, so it goes first.

### P3-B: Runner network isolation

| | |
|---|---|
| Capability | A contained runner has no network, and the service proves it at startup |
| Component | `isolation.network = "denied"`, a startup probe, the reference launcher |
| Left to providers | The firewall on hosts that do not use the launcher |
| Minimal tests | From inside a job, connections to loopback, the node RPC, the service and an outside address all fail, with a positive control. Removing the rule refuses the start |
| Excluded | Per-job network policy, egress allowlists, a general sandbox |

**Recommended mechanism:** an `nftables` rule on the runner's uid, set by the
launcher as root. The service gains no capability.

| Option | For | Against |
|---|---|---|
| **`nftables` on the uid (recommended)** | No new privilege in the service; covers loopback; small | Does not cover abstract Unix sockets |
| Network namespace per job | Also covers abstract Unix sockets | The service would need `CAP_SYS_ADMIN`, far more than it holds now |
| Namespace set up by a privileged helper | Same coverage, service unchanged | A new privileged component to audit |

Abstract Unix sockets would stay a stated limitation. This is the prerequisite
for untrusted workloads: it does not by itself make them safe, since P2 gives no
private filesystem view.

### P3-C: Provider network authentication and TLS

| | |
|---|---|
| Capability | A client reaches a remote provider confidentially, and refuses anything else |
| Component | `ProviderClient`, a reference TLS configuration, the runbook |
| Left to providers | Certificates, the TLS terminator, exposure, rate limiting |
| Minimal tests | A client refuses `http://` to a non-loopback address. It refuses a wrong certificate. A request altered in transit is refused. A replay through the terminator is refused |
| Excluded | A certificate authority, a public endpoint, client accounts |

**Recommended approach:** the service stays loopback-only behind a TLS terminator
the provider runs. The request signature covers method, path and body, so it
survives a terminator unchanged, and the service gains no TLS dependency.

| Option | For | Against |
|---|---|---|
| **Terminator run by the provider (recommended)** | Matches the existing boundary; no new dependency in the service | One more component for the provider |
| TLS inside the service | One process | A new dependency, and certificate handling in the process that holds the key |

**TLS proves a host name, not an executor.** The binding between them needs a
decision, question 3 below. P4 (signed capability declarations) is its natural
home.

### P3-D: Independent providers on separate hosts

| | |
|---|---|
| Capability | Two providers on separate hosts, with their own keys, serve one client against one chain |
| Component | A runbook and a test procedure; a second runner written separately; a runner conformance pack |
| Left to providers | Running the hosts |
| Minimal tests | Each provider completes a synthetic job to a verified T1 result across a real network. The second runner passes the pack |
| Excluded | Discovery, failover (P5), a hosted test network, certification |

Containers on one machine do not prove this. It needs two real hosts, and it
depends on P3-B and P3-C.

## 3. What P3 will not claim

- Provider honesty: evidence still stops at T1. That needs G3 and later T2 or T3.
- Full filesystem isolation, or safety for arbitrary untrusted workloads.
- Network isolation or containment on Windows.
- Protection against a compromised provider service.
- Public deployment readiness: P4 and P5 stay open.

## 4. Approval required

| # | Decision | Recommendation |
|---|---|---|
| 1 | Approve the scope and order: P3-A, B, C, D | Yes |
| 2 | Runner network: `nftables` on the uid, with abstract Unix sockets as a stated limitation | Yes |
| 3 | How a client binds a host to an executor: out-of-band configuration now, or pull the signed declaration forward from P4 | Out-of-band now |
| 4 | TLS: a terminator run by the provider, not TLS inside the service | Yes |
| 5 | P3-D hosts: which two, who pays, and that they stay private and hold synthetic data only | Your decision. No cost is committed |
| 6 | Whether Windows stays a functional target only | Yes |

Each task then needs its own implementation approval. Public deployment needs a
separate one.
