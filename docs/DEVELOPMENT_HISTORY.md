# Development History

**Status:** CURRENT. A record, with no authority of its own: where it and a normative
document disagree, [`INDEX.md`](INDEX.md) decides. Covers PRs #181 to #203.
**Last updated:** 2026-11-05, at `dev` `cd0d089c`. `main` is `fbfe5f60`, and
`mvp-v0.1.0` is `099b8c08`, both unchanged in this period.

## Implemented and verified

"Verified" means the named evidence passed on the merge commit's push CI, unless stated.

| Capability | PR, commit | Evidence |
|---|---|---|
| Vision v2 adopted | #181 `c1b5fe49` | [`VISION_v2.md`](VISION_v2.md) |
| Generic job contract `mbongo-job:v1` | #188 `7852f999` | `job_contract`, contract vectors |
| Provider service, `mbongo-runner:v1`, generic client | #190 `7793c452` | `provider_contract`, `provider_harness` |
| Durable provider state | #193 `360151e8` | `provider_durability`, real crash recovery in `provider_harness` |
| Runner containment on Linux | #195 `7abe0b84` | `provider_harness --contained` |
| Conformance against the provider service | #197 `5aed23a7` | 38 of 38, Linux and Windows CI |
| Runner network denial on Linux | #198 `56678d5e` | `provider_harness --contained`, with a control |
| Secure client transport | #201 `093a7af0` | `provider_transport`, Linux and Windows CI |
| nginx reference, inbound firewall | #202 `77885064` | Remote-namespace tests, one host |
| CI reliability | #185 `36426664`, #194 `bd0d499f`, #199 `c96ff283` | Disk, ports and test timing |

Design records: #180 direction, #184 `5c7124d7` D15 and G2, #189 `20a5f910`, #192 `cb6ee524`,
#196 `1304a313` P3, #200 `224d4bae` P3-C, #203 `cd0d089c` P3-D. Security records: #182
`bdcea06a`, #186 `3d942721`, #187 `34f30d39` on #178.

## Experimental or not adopted

| Item | State |
|---|---|
| #179 approach C lab candidate | Open, not merged |
| #183 hardening candidate r2 | Open draft, not adopted |
| #147 sync recovery | Open, unmerged. Tracked by #146 |
| `mbongo-explorer` | Blocks only, no tests, not in CI |
| RFC 0003, RFC 0004 | Research, non-normative |
| RFC 0006 | Draft |

## Limitations

**Base chain** (read from code, 2026-11-05):

- Blocks carry no producer and no signature, and `state_root` is unset.
- No chain identifier. `--chain` has no effect. Genesis is code-defined.
- One fixed producer. No fork choice, finality or validator set.
- Blocks reach direct peers only. Transactions are not propagated between nodes.
- No fees, issuance, rewards or supply cap on-chain: these exist only as a research model.
- No settlement of any kind. Receipts move no value.
- `mbongo-wallet` is a stub, with no wallet CLI, faucet or encrypted keys.
- Other findings are in private security tracking and are not recorded here.

**Provider and transport:** see [`PROVIDER_CONTAINMENT_LINUX.md`](runbooks/PROVIDER_CONTAINMENT_LINUX.md) and
[`PROVIDER_TLS_REFERENCE_LINUX.md`](runbooks/PROVIDER_TLS_REFERENCE_LINUX.md). In short: Linux only, no complete
network or IPC isolation, no protection from a compromised provider service, nothing
cryptographic between TLS identity and executor, evidence stops at T1, single-host tests.

## Decisions

Decisions live where they were made. This list only points to them.

| Decisions | Where |
|---|---|
| D1 to D15, including infrastructure first | [`GENERAL_PURPOSE_COMPUTE_PROPOSAL.md`](architecture/GENERAL_PURPOSE_COMPUTE_PROPOSAL.md) §0 |
| G2 design and provider stages | [`G2_MBONGO_JOB_V1_PLAN.md`](architecture/G2_MBONGO_JOB_V1_PLAN.md) |
| P3 sequence and six decisions | [`P3_PROVIDER_NETWORK_PROPOSAL.md`](architecture/P3_PROVIDER_NETWORK_PROPOSAL.md) §4 |
| P3-C seven decisions | [`P3C_PROVIDER_TRANSPORT_DESIGN.md`](architecture/P3C_PROVIDER_TRANSPORT_DESIGN.md) §12 |
| MVP v0.2 | [`MVP_v0.2_NETWORK_READINESS.md`](product/MVP_v0.2_NETWORK_READINESS.md) §11 |

## Deferred and excluded

| Item | State |
|---|---|
| P3-D execution | Plan approved in principle. Environment and cost not approved |
| CC-4, O1-B02, V4 work (#175, #179) | Deferred, not resumed |
| Live SEV-SNP attestation (#144) | Blocked on a guest |
| Capability declarations, provider failover (P4, P5) | Not started |
| G3 re-execution | Prerequisites only |
| Excluded for now | Public deployment, token launch, valuable MBO, settlement, staking, slashing, GPU, client-specific workloads, a general-purpose VM |
