# MVP v0.2 — End-to-End Functional Private Network

**Status:** Planning structure approved (2026-11-05). It authorizes no implementation: each stage has its own gate.
**Created:** 2026-11-05
**Authority it holds:** what MVP v0.2 delivers, and its gates.
**Authority it does not hold:** every normative document keeps its authority. Protocol
facts stay in the protocol lock and the RFCs. See [`INDEX.md`](../INDEX.md).
**Progress and decisions:** the master EPIC, #209.
**History:** [`DEVELOPMENT_HISTORY.md`](../DEVELOPMENT_HISTORY.md).

"MVP v0.2" is a **product** version. It is not protocol v0.2, nor the
`v0.2-devnet-stable` tag. This MVP needs protocol v0.5.

## 1. Objective

One private network on which someone other than its authors can hold test MBO, send
it, run a generic compute job, and verify the result independently.

The compute layer is ahead of the chain beneath it. MVP v0.2 closes that gap and
nothing else.

| User | Can do |
|---|---|
| Network operator | Define a genesis, start authenticated nodes, reset and recover |
| Wallet user | Create a key, get test MBO, send it, verify a balance |
| Compute client | Pay nothing, submit a generic job, verify a T1 receipt |
| Provider | Run the existing contained service against this network |

## 2. Functional scope

| Stage | Delivers | Depends on |
|---|---|---|
| **S0** (#204) | Native balance conservation, with invariant tests. Details stay in private security tracking | Prepared now. Merges under RFC 0006, once accepted |
| **S1** (#205) | RFC 0006 and its implementation: `chain_id`, configurable genesis, signed blocks, state commitment | RFC accepted, S0 merged |
| **S2** (#206) | Test MBO through genesis, a wallet CLI, SDK transfers, a faucet | RFC 0006 accepted. Key handling can start earlier |
| **S3** (#207) | Transaction forwarding to the producer, mempool and peer limits, sync recovery, release binaries, operator guide | S0 merged. Preparation can start earlier. Blocks signed by S1 for its final tests |
| **S4** (#208) | End-to-end acceptance and release | S1, S2, S3 |

```text
RFC 0006 accepted ──> S0 merged ──> S1 implemented ──┬──> S2 ──┐
                         └─────────────> S3 ─────────┴─────────┴──> S4
```

## 3. Reusable components

| Component | State | Use |
|---|---|---|
| Node, storage, sync, block notify | Tested on protocol v0.4 | Extended by S1 |
| `@mbongo/sdk` 0.2.0 | Published | Gains signing and transfers |
| `provider_service`, `job_client`, `mbongo-runner:v1` | Tested, CI | Unchanged in role |
| Containment, network denial, TLS transport | Tested on Linux | Unchanged |
| `compute-conformance-v1` | 38 of 38 on the provider service | Regression |
| Docker devnet | Tested, CI | Base of S4 |
| `mbongo-explorer` | Blocks only, no tests | Optional |

## 4. Dependencies and blockers

- RFC 0006, under review in [PR #211](https://github.com/MbongoChain/mbongo-chain/pull/211), must be accepted before any protected
  protocol work. The process requires a 7-day cooling period and a
  written adversarial review, with no bypass.
- #103: one maintainer, so RFCs follow the sole-maintainer path.
- #146 and PR #147: sync can stall after a failed request. Part of S3.
- S0 changes a protected state transition, so it merges only under an accepted RFC.
  There is no emergency exception.

## 5. Security assumptions

- **Private network:** known operators. No public endpoint and no value at risk.
- **One authority** in this MVP. If it stops, the chain stops.
- **Test keys and test MBO only.** Wallet keys are protected for a test network.
- **A client trusts its node** about inclusion.
- **Evidence stops at T1.** Provider honesty is not verified.
- **Unchanged limits:** no isolation on Windows, no complete network or IPC isolation,
  no protection from a compromised provider service.

## 6. Acceptance criteria

All from a clean setup, on a multi-node private devnet, with synthetic fixtures.

| # | Criterion |
|---|---|
| AC1 | A network starts from a genesis file, with its own `chain_id` |
| AC2 | Two wallets are created, with keys encrypted at rest |
| AC3 | A wallet gets test MBO from the faucet, within its limit |
| AC4 | It transfers test MBO to the other wallet, through a follower |
| AC5 | Inclusion is in a block signed by the authority, and is verified on every node |
| AC6 | Balances and the transaction are verified independently, and total supply is unchanged |
| AC7 | A forged block, and a transaction signed for another network, are refused |
| AC8 | A generic job runs on the contained provider, paid by nobody, and reaches an independently verified T1 receipt |
| AC9 | Node, provider and follower restarts recover, with state agreeing on all nodes |
| AC10 | A clean devnet comes up from published artifacts, following the guide alone |
| AC11 | All existing conformance, containment, transport and replay checks pass |

## 7. Release evidence

Per [`ENGINEERING_EVIDENCE.md`](../ENGINEERING_EVIDENCE.md): CI bound to the exact SHA, PR
and push CI reported separately, skipped jobs named, committed identity proven from Git
objects, cardinality stated before comparing. Plus a transcript of AC1 to AC10 from a
clean machine, artifact hashes, the protocol lock for v0.5, and stated limits.

## 8. Exclusions

Public deployment, token launch, economically valuable MBO, provider settlement, staking,
slashing, validator elections, GPU work, customer workload optimization, a general-purpose
VM, transaction gossip unless S3 proves it necessary, and paid P3-D deployment.

## 9. Approval gates

| Gate | Decision | Who |
|---|---|---|
| G0 | Approve this specification and the planning structure | Maintainer |
| G1 | RFC 0006: `READY_FOR_REVIEW`, then acceptance after cooling and adversarial review | Maintainer, under the process |
| G2 | S0: the prepared correction, reviewed privately, merged under the accepted RFC | Maintainer |
| G3 | S1 implementation merged, protocol lock v0.5 | Maintainer |
| G4 | S2 and S3, each with its own approval | Maintainer |
| G5 | S4 evidence accepted, MVP v0.2 released | Maintainer |

## 10. Implementation PR requirements

Every implementation PR states: its specification and issue, scope and exclusions, tests
and evidence, protected-surface changes, known limitations, and required approvals.
It uses `Refs #N` unless closing that issue is the intent.

## 11. Decision log

Append only. Never rewrite an entry: a change of mind is a new entry.

| Date | Decision | Source |
|---|---|---|
| 2026-11-05 | Development is coordinated around one MVP v0.2 EPIC | Maintainer |
| 2026-11-05 | Order: S0, then S1 through its RFC, then S2 and S3 in parallel, then S4 | Maintainer |
| 2026-11-05 | Paid P3-D deployment is excluded from this MVP. Its plan stays approved in principle | Maintainer |
| 2026-11-05 | The monetary-conservation finding stays in private tracking | Maintainer |
| 2026-11-05 | G0: planning structure approved. RFC 0006 moves to its own PR | Maintainer |
| 2026-11-05 | Private vulnerability tracking approved. No emergency exception is claimed | Maintainer |
| 2026-11-05 | A protected state-transition change merges only after its governing RFC is accepted | Maintainer |
