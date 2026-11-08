# Direction C — Progressive Decentralization

| | |
|---|---|
| **Status** | **APPROVED STRATEGIC RESEARCH DIRECTION (2026-11-08), NON-NORMATIVE.** It records a direction for research. It adopts no consensus protocol, no validator membership policy for any network, and no monetary parameter. Nothing here authorizes implementation. |
| **Kind** | Decision note, documentation only. Every NORMATIVE document in [`INDEX.md`](../INDEX.md) wins on its subject. |
| **Date** | 2026-11-08 |
| **Changes nothing in** | [`VISION_v2.md`](../VISION_v2.md), RFC 0006 (PR #211, its cooling period and acceptance gates), [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md), S0, the MVP v0.2 scope and gates ([`MVP_v0.2_NETWORK_READINESS.md`](../product/MVP_v0.2_NETWORK_READINESS.md)), any code |
| **Related** | #209 (MVP v0.2 EPIC), #211 (RFC 0006), #245 (legacy consensus documents marked historical), [RFC 0003](../rfcs/0003-mbo-monetary-policy-security-budget.md) and [RFC 0004](../rfcs/0004-mbo-monetary-decision-framework.md) (non-normative monetary research) |

## 0. Decision

The project owner selected Direction C on 2026-11-08:

> Progressive decentralization, starting with a verifiable single-authority MVP,
> researching an independent multi-operator BFT network, and preserving a credible
> path toward more open validator participation.

It sets the order of research. It does **not** approve a multi-validator network,
choose a consensus protocol, or fix a validator count. Each step that changes the
protocol needs an accepted RFC (VISION_v2 P8).

## 1. Three stages

| Stage | What it is | Status today |
|---|---|---|
| **1. Verifiable single authority** | MVP v0.2. One authority signs every block. The network has its own `chain_id` and a state commitment, as proposed by RFC 0006 | RFC 0006 is under review in #211 and is not accepted. Nothing here changes it or its gates. One authority means that if it stops or censors, the chain stops; its key cannot be replaced without a new genesis (RFC 0006, Security) |
| **2. Independent multi-operator BFT network** | A designated set of independent validators with deterministic finality: a block, once committed, is final. Tendermint-family and HotStuff-family protocols are being researched | **Research only.** Not approved. No protocol, network size or operator is chosen. RFC 0006's single-authority signature does **not** provide BFT finality |
| **3. More open validator participation** | Membership open beyond a designated set, through permissionless proof of stake or another defensible mechanism | **Preserved option.** Not chosen. A stake-based design depends on the economic gates of RFC 0004 |

The stages are an order, not a schedule. A stage may be revised or skipped by a later
decision. No date is attached to any of them.

## 2. Decentralization indicators

Measured per network, from operator declarations that can be checked. The values are
facts to report, not targets: targets are set by governance before a stage starts.

| # | Indicator | How it is measured | Stage 1 value |
|---|---|---|---|
| I1 | **Independent operators** | Validator keys grouped by control group (I2); the number of groups | 1 |
| I2 | **Common ownership and control** | Control groups: entities linked by ownership, employment, funding, administrators or key custody. Report the largest group's share of validator keys, and the smallest number of groups that together reach one third (can halt a BFT network) and two thirds (can control it) | 1 group holds everything |
| I3 | **Hosting and geographic diversity** | Distinct hosting providers, autonomous systems and jurisdictions among validators; the largest share held by one of each | 1 |
| I4 | **Governance concentration** | The smallest number of independent people or entities whose agreement can admit or remove a validator, or change the protocol | 1 (the sole maintainer, #103) |
| I5 | **Admission accessibility** | Whether admission criteria are published; whether a party outside the current set that meets them can be admitted, and through what decision; time to a decision; for stage 3, the requirements to enter | No admission process |
| I6 | **Key-management independence** | Share of validator keys generated and held by their own operator, in a protected module, with no copy held by another party | Not applicable until keys exist |

A BFT network with N = 3f + 1 validators keeps its guarantees only if no control group
(I2) holds more than f keys. Counting servers, or keys, overstates decentralization:
I1 counts control groups.

## 3. Gates between stages

### Stage 1 to stage 2

All of these, in addition to the existing MVP v0.2 gates, which this note does not
change:

1. MVP v0.2 released (G5): RFC 0006 accepted and implemented, S0 merged, protocol lock v0.5.
2. A **governance body** for admitting and removing validators that does not depend on
   one person (I4 above 1), with published rules and conflict-of-interest handling.
3. A **consensus RFC** accepted: the protocol family, commit certificates, quorum and
   equivocation evidence, proposer rotation, validator-set transitions, a height-based
   activation mechanism (the v0.4 lock has none), state synchronization, and what
   clients and the SDK verify.
4. Security work: a formal specification of the chosen protocol, fault-injection
   testing, a multi-node CI devnet with failure scenarios, a written adversarial review,
   and an external audit before the network carries value.
5. Operators that meet published requirements for independence, key management,
   availability and diversity, with indicator targets set **before** launch.
6. A decided source of operator compensation. It need not be MBO issuance.

### Stage 2 to stage 3

1. A **membership decision**: permissionless proof of stake, or another mechanism.
2. If stake-based: staking, slashing and unbonding designs, each through an RFC.
3. The economic gates of RFC 0004 that apply: a security requirement and threat model
   (GATE 3), a fee mechanism (GATE 4), the role of compute revenue (GATE 5), perpetual
   issuance (GATE 6), governance discretion (GATE 7), invariants the runtime enforces
   (GATE 8).
4. Client verification of finality without trusting a node.
5. An operating record of stage 2 that governance judges sufficient. No figure is set
   here.

A stage may be paused, or a step reversed, if its indicators regress below the targets
governance set for it.

## 4. Alternatives kept open

- **Permissionless proof of stake** with BFT finality.
- **A larger permissioned federation**, with published admission criteria.
- **Anchoring commitments to an external chain**, as a complement to a stage, not a
  replacement for consensus.
- Any other mechanism a future RFC defends.

Excluded by VISION_v2: validators executing compute (proof of useful work, P1).
Designs with probabilistic finality must reconcile it with P3, deterministic
verification, before they can be considered.

## 5. Validators and compute providers are separate

| | Consensus validators | Compute providers |
|---|---|---|
| Role | Order transactions and validate blocks, receipts included, by deterministic rules | Execute client workloads off-chain and sign receipts |
| Executes workloads | Never (VISION_v2 P1) | Yes |
| Keys | Consensus signing key | Executor and provider keys |
| Paid by | Not decided: fees, issuance or external funding | The client, off-chain. Settlement needs its own RFC (VISION_v2 §7) |
| Security budget | Their compensation is part of it | Never part of it |

One entity may be both a provider and a validator. That is a control link to report
under I2, not a requirement. Validator compensation never comes from compute payments
unless an accepted RFC defines a protocol share of them (RFC 0004 GATE 5).

## 6. Relationship to MVP v0.2

MVP v0.2 keeps its scope: one authority, no validator elections, staking, slashing,
rewards or fees ([`MVP_v0.2_NETWORK_READINESS.md`](../product/MVP_v0.2_NETWORK_READINESS.md)
§5 and §8). Direction C is a **post-MVP research dependency** recorded against #209. It
adds no requirement to, and moves no gate of, G0 to G5.

## 7. What this note does not do

- Adopt or rank a consensus protocol, a validator count or a membership model.
- Approve a multi-validator network, or name operators.
- Define staking, slashing, rewards, fees or any MBO supply parameter.
- Modify VISION_v2, RFC 0006, the protocol lock, S0 or the MVP v0.2 gates.
- Implement anything.

## 8. Decisions still open

1. Who holds validator admission and removal authority after the sole-maintainer
   period, and how conflicts of interest are handled.
2. Indicator targets for stage 2.
3. The consensus protocol family, and whether to integrate an existing implementation
   or write one.
4. The validator-set transition rule and the protocol activation mechanism.
5. How validators are compensated in stage 2, and whether MBO plays any part.
6. Whether and when stage 3 is pursued, and through which membership mechanism.
