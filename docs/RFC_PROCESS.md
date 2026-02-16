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

| Stage | Entry condition | Exit condition |
|-------|-----------------|----------------|
| **Draft** | Author opens PR with RFC document in `docs/rfcs/` | Author marks ready for review |
| **Review** | At least one Core Maintainer assigned as reviewer | All reviewers approve or request changes |
| **Accepted** | All Core Maintainer reviewers approve | Implementation work begins |
| **Implemented** | All code changes merged, tests passing | Release candidate tagged |
| **Released** | Protocol lock document updated, new git tag created | RFC status set to Released |

An RFC may also reach **Rejected** or **Withdrawn** at any stage before Accepted.

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

Core Maintainers are the gate for RFC approval. This is a **role**, not a named list. A Core Maintainer is anyone with merge rights to the `main` branch of the mbongo-chain repository.

Rules:
- Every RFC requires approval from **at least one** Core Maintainer who is not the author.
- For changes affecting multiple locked surfaces, **at least two** Core Maintainer approvals are required.
- Core Maintainers may request external review (security audit, domain expert) before approving.
- No single person may both author and approve an RFC.

### Dispute Resolution

If reviewers disagree, the RFC remains in Review until consensus is reached. A majority of Core Maintainers may force a decision after a 7-day deliberation period.

---

## File Conventions

- RFCs live in `docs/rfcs/`.
- Filename format: `NNNN-short-title.md` (e.g. `0002-add-fee-field.md`).
- Number assignment: use the next available integer. Check existing files before claiming a number.
- Template: [docs/rfcs/0001-template.md](rfcs/0001-template.md).
