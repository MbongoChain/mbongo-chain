# RFC Process

**Status:** ACTIVE
**Last updated:** 2026-02-16

---

## Purpose

This document defines when and how protocol changes to Mbongo Chain are proposed, reviewed, and released. It exists to protect the stability guarantees established by the [Protocol Lock](specs/PROTOCOL_LOCK_v0.2.md).

---

## When an RFC Is Required

An RFC is **mandatory** before merging any change that touches a locked surface. Specifically:

| Change type | Examples |
|-------------|----------|
| Protocol lock surfaces | Any item listed in [PROTOCOL_LOCK_v0.2.md](specs/PROTOCOL_LOCK_v0.2.md) under "Locked Surfaces" or "Forbidden Changes" |
| Block or transaction SCALE encoding | Adding, removing, or reordering fields in `BlockHeader`, `BlockBody`, or `Transaction` |
| Hashing rules | Changing BLAKE3 inputs, Merkle commitment scheme, or hash display format |
| `apply_block` validation rules | Adding, removing, or altering any of the five block validity rules |
| Storage semantics | Changing the meaning of `write_batch`, `get_block_by_height`, `get_latest_height`, or atomicity guarantees |
| P2P protocols or message codecs | Changing `SyncRequest`, `SyncResponse`, `SyncNotification`, `BlockNotifyAck`, frame encoding, protocol negotiation strings, or `MAX_RANGE` |
| RPC breaking changes | Renaming methods, changing parameter types, changing return types, removing methods, or altering error codes in [rpc_v0.1.md](specs/rpc_v0.1.md) |

An RFC is **not required** for changes listed under "Allowed Changes" in the protocol lock (docs, tooling, CI, logging, metrics, SDK, internal refactors that preserve locked semantics).

---

## RFC Lifecycle

Every RFC moves through these stages in order:

```
Draft → Review → Accepted → Implemented → Released
```

The entry and exit conditions for **Review** and **Accepted** depend on the
governance mode in force — see [Governance Modes](#governance-modes). Every
other stage is the same in both modes.

| Stage | Entry condition | Exit condition |
|-------|-----------------|----------------|
| **Draft** | Author opens PR with RFC document in `docs/rfcs/` | Author posts an explicit `READY_FOR_REVIEW` declaration on the PR |
| **Review** — MULTI | At least one eligible independent Core Maintainer assigned as reviewer | All required reviewers approve or request changes |
| **Review** — SINGLE | `READY_FOR_REVIEW` declared and the cooling period started, PR open to public comment | Cooling period elapsed with no unresolved substantive objection |
| **Accepted** — MULTI | The required independent approvals are present | Implementation work begins |
| **Accepted** — SINGLE | Every condition in [Sole-maintainer acceptance](#sole-maintainer-acceptance) is met | Implementation work begins |
| **Implemented** | All code changes merged, tests passing | Release candidate tagged |
| **Released** | Protocol lock document updated, new git tag created | RFC status set to Released |

An RFC may also reach **Rejected** or **Withdrawn** at any stage before Accepted.

**Accepted is not Released.** Acceptance authorises implementation work; it
creates no protocol lock and activates no protocol version. That separation is
unchanged.

In SINGLE mode, **no reviewer is assigned**. A reviewer must never be recorded
who did not review.

---

## Required Sections

Every RFC MUST contain the following sections. Use the template at [docs/rfcs/0001-template.md](rfcs/0001-template.md).

| Section | Purpose |
|---------|---------|
| **Motivation** | Why this change is necessary. What problem it solves. |
| **Scope** | Exactly which locked surfaces are affected. |
| **Non-Goals** | What this RFC explicitly does not address. |
| **Design** | Technical specification of the change. Must be unambiguous and implementable. |
| **Compatibility** | How existing nodes, data, and clients are affected. Migration path if applicable. |
| **Security** | Security implications. Attack surface changes. |
| **Testing** | Required tests to validate the change. Acceptance criteria. |
| **Rollout** | Deployment sequence, version bump strategy, coordination steps. |

---

## Versioning Rules

### Breaking protocol change

A change is breaking if it alters any locked surface such that nodes running the old version cannot interoperate with nodes running the new version.

Requirements:
1. Bump to a new protocol version (e.g. v0.2 to v0.3).
2. Update the protocol lock document (create `PROTOCOL_LOCK_v0.3.md` or amend the existing one).
3. Create a new git tag (e.g. `v0.3-devnet-stable`).
4. The RFC MUST specify the new version number.

### Non-breaking additive change

A change is additive if it extends the protocol without breaking existing behaviour (e.g. adding a new optional RPC method, adding a new P2P message type that old nodes ignore).

Requirements:
1. Minor version bump (e.g. rpc v0.1 to v0.1.1, or new spec file rpc_v0.2 if method additions are substantial).
2. Update relevant spec documents.
3. No protocol lock bump required unless the addition becomes a new locked surface.

---

## Decision Authority

### Core Maintainers

Core Maintainers are the gate for RFC approval. The roster below is the **sole
authoritative source** of that authority.

| Core Maintainer | Since |
|---|---|
| @gkalombo21 | 2026-02-16 |

**Protocol approval authority is decoupled from repository permissions.**
Merge rights to any branch, organization membership, repository role,
`CODEOWNERS` entries and GitHub team membership **do not** confer it, alone or
in combination. Only presence on this roster does. A GitHub team may mirror
the roster to route review requests, but it is never the source of authority.

The converse also holds: a Core Maintainer does **not** need merge rights.
Someone may be trusted to approve protocol changes without being trusted to
write to the tree, and that is a supported configuration.

**Roster changes** are made by a pull request editing this table, stating the
rationale, and are therefore recorded in git history. A role is never acquired
silently as a side effect of a permission change.

Rules:
- No single person may both author and approve an RFC.
- Core Maintainers may request external review (security audit, domain expert)
  before approving.
- Approval requirements depend on the governance mode below.

### Governance Modes

For an RFC `R`, an **eligible independent reviewer** is a person on the roster
above who is not an author of `R`. Let `E(R)` be the number of them.

| Mode | Condition |
|---|---|
| **MULTI_MAINTAINER** | `E(R) >= 1` |
| **SINGLE_MAINTAINER** | `E(R) = 0` |

The mode is **derived, not chosen.** An author cannot select it, and it cannot
be overridden for convenience.

`E(R)` is evaluated **at acceptance time**, never at authoring time. If the
roster gains an eligible reviewer while an RFC is in Review, MULTI mode
applies immediately to that RFC, and the sole-maintainer path is no longer
available to it.

SINGLE mode exists because the project currently has one maintainer, not
because independent review is optional. It is a transparency mechanism for an
unavoidable situation, and it disappears automatically the moment it is no
longer needed.

**Nominal maintainers must not be appointed to satisfy a threshold.** Adding a
name in order to manufacture an approval defeats the entire purpose of this
section.

### MULTI_MAINTAINER approvals

- An ordinary RFC requires approval from **at least one** eligible independent
  Core Maintainer.
- An RFC affecting **multiple locked surfaces** requires **at least two**,
  when at least two eligible independent Core Maintainers exist.
- If fewer eligible independent reviewers exist than the nominal threshold,
  **all** available eligible independent reviewers must approve, **and** the
  applicable cooling period below still applies.
- When `E(R) >= 1`, sole-maintainer acceptance is **forbidden**. There is no
  fallback to the weaker path merely because the nominal count is not met.

### Cooling period

An RFC may not be accepted until a minimum period has elapsed since its
`READY_FOR_REVIEW` declaration:

| RFC affects | Minimum |
|---|---|
| any locked protocol surface | **7 calendar days** |
| no locked surface | **72 hours** |

Locked surfaces are those listed in the current protocol lock document; this
process defines no competing list. **If the classification is disputed, the
longer period applies** until the dispute is resolved.

The clock starts only from an explicit, timestamped `READY_FOR_REVIEW`
declaration made under this process. It is never backdated, and prior activity
on the PR does not start it.

The period applies in **both** modes.

**There is no emergency bypass.** At the project's current stage there is no
production network and no value at risk, so no protocol change is urgent
enough to justify one, and a bypass would defeat the control that makes SINGLE
mode honest.

### Sole-maintainer acceptance

In SINGLE mode, and only in SINGLE mode, the sole Core Maintainer may accept
an RFC they authored — but only when **every** one of the following holds:

1. the RFC exists as an open pull request;
2. all [Required Sections](#required-sections) are complete;
3. an explicit `READY_FOR_REVIEW` declaration was posted, with a timestamp;
4. the applicable cooling period has fully elapsed;
5. a **written adversarial review** is posted on the PR, naming what was
   attacked and what was found;
6. required CI is green;
7. there are **zero** unresolved CRITICAL findings and **zero** unresolved
   HIGH findings;
8. any substantive objection raised during the period has been addressed;
9. the acceptance record below is recorded;
10. implementation begins only after acceptance.

This is **not** independent approval, and must never be described as such.

### Acceptance record

Every accepted RFC records, in its front matter:

```
**Status:** Accepted
**Governance mode:** MULTI_MAINTAINER
**Accepted:** <date>
**Approved by:** <Core Maintainer handles>
```

In SINGLE mode the record is explicit about what did not happen:

```
**Status:** Accepted — sole-maintainer
**Governance mode:** SINGLE_MAINTAINER
**Accepted:** <date>
**Accepted by:** <handle> (author)
**Independent Core Maintainer review:** unavailable — `E(R) = 0` at acceptance
```

The pull request must additionally carry the audit trail: the
`READY_FOR_REVIEW` timestamp, the cooling-period end, the CI status, a
reference to the adversarial review, the unresolved CRITICAL and HIGH counts,
the governance mode and `E(R)` as evaluated at acceptance, the acceptance
timestamp, and the accepting maintainer.

### Historical RFCs

Some RFCs were released before this process was operationally enforced and
have no approval trail. **Missing historical approval evidence must never be
reconstructed or fabricated.** Such an RFC may be annotated to record that it
predates enforcement and that the required trail is unavailable; it must not
be annotated to suggest reviews occurred that did not.

### Dispute Resolution

If reviewers disagree, the RFC remains in Review until consensus is reached. A
majority of Core Maintainers may force a decision after a 7-day deliberation
period.

---

## File Conventions

- RFCs live in `docs/rfcs/`.
- Filename format: `NNNN-short-title.md` (e.g. `0002-add-fee-field.md`).
- Number assignment: use the next available integer. Check existing files before claiming a number.
- Template: [docs/rfcs/0001-template.md](rfcs/0001-template.md).
