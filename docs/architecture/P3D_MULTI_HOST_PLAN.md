# P3-D: Private Multi-Host Interoperability Plan

**Status: approved in principle (2026-11-05).** The environment and its cost are
**not** approved. Nothing here is provisioned or started: no infrastructure is
purchased, no billing is activated, and no endpoint is public. D1 to D12 are kept as
proposed private acceptance criteria.

The producer and follower topology with SSH forwarding is a **temporary test
arrangement**, not the target decentralized network architecture.

**Goal:** show that what passed on one host works across two genuinely separate
Linux hosts, with two independently configured providers. No new workload, and no
protocol change.

**Out of scope:** open provider assignment, scheduling, payments and settlement,
customer workloads, real data, GPU, and new consensus or protocol features.

## 1. What was verified

| Question | Finding |
|---|---|
| Existing machines | **None confirmed.** This Windows machine has Docker Desktop, no Linux distribution of its own and no SSH hosts configured. `gcloud` is installed with no authenticated account, so existing VMs could not be listed. That is not proof that none exist |
| GitHub runners | Not usable: two jobs cannot reach each other, and neither is a host we control |
| Same machine, two VMs | Not genuinely separate hosts, so it would not meet the goal |
| Joining one devnet | Works today: `--bootnodes`, one binary, one genesis, `--no-mdns` off a shared LAN. The Docker devnet does this across containers |
| **Transactions** | **Only blocks are gossiped, not transactions** (`mbongo-node/src/backend.rs`). A transaction sent to a follower never reaches the producer. This decides the topology below |

## 2. Proposed configuration

Two identical VMs, Ubuntu 24.04, 2 vCPU, 4 GiB, 20 GiB, on one private subnet.

```text
host A                                     host B
  mbongo-node --producer  <── P2P blocks ──  mbongo-node (follower)
       ^ loopback                                 (witness: reads only)
       |  <═══════ SSH tunnel, RPC and REST ═════ provider B, client B
  provider A + nginx A  <──── TLS 8443 ────  client B
  client A              ───── TLS 8443 ────> nginx B + provider B
  runner: Node fixture                       runner: independent implementation
```

- **One producer, on host A.** Consensus is unchanged.
- **Host B's follower is a witness.** Every receipt is read from both nodes and
  must match.
- **Everything host B submits goes to the producer,** because transactions are not
  gossiped: provider B's receipts and client B's tasks.

### How a client reaches its node

The node's RPC has no authentication, so it never crosses the network in the clear.

| Client | Path |
|---|---|
| Client A | Loopback to the producer |
| Client B, provider B | An SSH tunnel from B to A, bound to loopback on both ends |

The tunnel is authenticated and encrypted, adds no port, and is manual, like the
rest of this test. Rejected: exposing RPC on the private network, which leaves it
open to anything on that subnet, and adding transaction gossip, which is a protocol
change.

A client still trusts its node about inclusion. That stays a stated limit.

### Ports

| Port | From | To | Purpose |
|---|---|---|---|
| 8443/tcp | The other host | Both | Provider TLS |
| 30333/tcp | Host B | Host A | Blocks |
| 22/tcp | Host B, operator through the provider's identity-aware tunnel | Both | The tunnel, administration |

`MBONGO_INBOUND_ALLOW=22,8443,30333` on both hosts. RPC, REST and the provider
backend stay on loopback. The launcher's list has no source addresses, so the cloud
firewall restricts sources to the subnet. **No inbound rule from the internet.**

### Identity and certificates

| | Provider A | Provider B |
|---|---|---|
| Executor key, state key | Generated on host A, never leaves it | Generated on host B, never leaves it |
| Certificate authority | Its own, private | Its own, private |
| Certificate name | `provider-a.internal` | `provider-b.internal` |
| Runner | Node fixture | Independent implementation |

Each client holds a `provider-endpoint-v1` file per provider, delivered by the
operator. Names resolve through `/etc/hosts`. The executors are funded from the
devnet's genesis test account. Nothing here is committed, and everything dies with
the hosts.

### The independent runner

A small Python runner, standard library only, written from the `mbongo-runner:v1`
contract without reading the Node fixture. Both declare the same runtime identity,
so one job spec runs on either provider. The job is `BIT_EXACT`, so both must
produce identical bytes.

## 3. Resources and cost

Sizing is an **estimate**: nothing has been measured across hosts. The job ceiling
stays 512 MiB. Binaries are built once, in release mode, and copied: one binary per
role on both hosts, with its hash checked, as the devnet runbook requires.

| Option | Rate, two hosts | 8 h session | 3 days | Note |
|---|---|---|---|---|
| **A. GCP, `e2-standard-2`, Montreal (recommended)** | About $0.15/h | About $1.30 | About $11 | `gcloud` is installed, and identity-aware SSH avoids public SSH. Rate from third-party listings |
| B. Hetzner `CX23` | About €0.01–0.02/h | Under €0.20 | About €1.50 | Cheapest. Listings disagree after 2026 price changes, and CX availability is unclear. No account here |
| C. This machine and one VM | About $0.07/h | About $0.60 | About $5 | Not recommended: WSL behind NAT is not a comparable host |

Add about $1 for disks, and a few cents for egress IPs. **Prices are unverified**:
check the provider's own calculator before approving.

**Proposed cap: $25,** with a billing alert at $10. Hosts are stopped between
sessions and deleted at the end.

## 4. Manual procedures

No automation: each is a written step, exercised once in the test.

| Procedure | Steps |
|---|---|
| Certificate rotation | Issue a new leaf from the same private CA, replace the file, `nginx -s reload`. Clients are untouched. Anchor rotation means replacing the client's file |
| Monitoring | A read-only script, run by hand: both node heights, nginx 5xx counts, `NETWORK_POLICY_LOST` in the provider log, and `nft list chain` compared with `inbound.policy`, which the service cannot check |
| Recovery | Restart a provider, which sweeps and resumes; restart a node, which resyncs; restart the tunnel. Refuse to continue if the block hashes differ |
| Teardown | Delete both hosts and disks, then confirm nothing remains in the project |

## 5. Acceptance tests

All on generic fixtures. Each must be observed on the real hosts.

| # | Test | Pass when |
|---|---|---|
| D1 | Environment | Hosts are separate, on one subnet, with no inbound internet rule. Checked from outside |
| D2 | One devnet | Both nodes report the same hash at the same height |
| D3 | Containment | `provider_harness --contained` passes on each host |
| D4 | Runner conformance | Each runner passes the contract vectors alone |
| D5 | Cross-host jobs | Client A to provider B, and client B to provider A, reach T1 |
| D6 | Same job, two providers | Identical output bytes from both runners, under different executors |
| D7 | Consistent receipts | Each receipt reads the same from both nodes, and verifies under its executor |
| D8 | Wrong provider | An endpoint naming the other executor is refused before upload, and another provider's CA fails TLS |
| D9 | Replay across providers | A request signed for A and sent to B is refused |
| D10 | Reachability | Across hosts: nginx yes; backend, RPC, REST and a runner's listener no |
| D11 | Failure | Provider restart, follower restart and tunnel loss mid-job: the client recovers or fails cleanly, with one receipt per task |
| D12 | Operations | Certificate rotation with no client change, and teardown complete |

## 6. What this will not show

Provider honesty, which stays at T1. A cryptographic link between certificate and
executor. Public-network behaviour, load or availability. Two independent
organizations: one operator runs both hosts, so "independently configured" means
separate keys, state and authorities. A runner written from the contract by the same
team is a weaker test than one written by a third party.

## 7. Decisions still required

Execution needs a separate decision on these.

| # | Decision | Recommendation |
|---|---|---|
| 1 | Environment | Option A |
| 2 | Budget | $25 cap, alert at $10 |
| 3 | Account | You run `gcloud auth login`, choose the project and enable billing. Only you can |
| 4 | Topology | Producer on A, witness on B, SSH tunnel for submissions |
| 5 | Independent runner | Python, from the contract only, by a separate agent that never sees the Node fixture. A third party would be stronger |
| 6 | Scope | D1 to D12 as the gate, with IPv4 only |
| 7 | Lifetime | At most 3 days of existence, stopped between sessions, then deleted |
