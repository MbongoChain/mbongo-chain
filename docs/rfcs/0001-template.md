# RFC NNNN — Title

**Status:** Draft
**Author:** (your name or handle)
**Created:** YYYY-MM-DD
**Protocol version:** (current) → (proposed)
**Locked surfaces affected:** (list from [PROTOCOL_LOCK_v0.2.md](../specs/PROTOCOL_LOCK_v0.2.md))

---

## Motivation

Why is this change necessary? What problem does it solve? What happens if we do nothing?

---

## Scope

Which locked surfaces does this RFC modify? Be explicit. Reference the exact rows from the "Locked Surfaces" or "Forbidden Changes" tables in the protocol lock document.

- [ ] Block/transaction SCALE encoding
- [ ] Hashing rules
- [ ] `apply_block` validity rules
- [ ] Atomic `write_batch` requirement
- [ ] Storage trait semantics
- [ ] P2P wire formats (`SyncRequest`, `SyncResponse`, `SyncNotification`, `BlockNotifyAck`)
- [ ] Protocol negotiation strings
- [ ] RPC method names, params, or return types
- [ ] Frame encoding

---

## Non-Goals

What does this RFC explicitly NOT address? List related concerns that are out of scope and should be handled separately.

---

## Design

Technical specification. Must be unambiguous and implementable by someone who has not read the discussion thread.

Include:
- Data structure changes (before/after SCALE layouts if applicable)
- Algorithm changes (pseudocode or Rust signatures)
- Wire format changes (message definitions, encoding)
- State migration logic (if existing data must be transformed)

---

## Compatibility

How does this change affect:

- **Existing nodes:** Can old nodes communicate with new nodes? Is there a transition period?
- **Existing data:** Do on-disk databases require migration? Is the migration reversible?
- **Existing clients:** Do RPC consumers need updates? Is there a deprecation period?

If this is a breaking change, describe the migration path step by step.

---

## Security

- Does this change alter the attack surface?
- Are there new denial-of-service vectors?
- Does this affect block validation integrity?
- Are there implications for key management or signature verification?

State "No security implications" only if you can justify why.

---

## Testing

What tests are required before this RFC can move to Implemented?

- [ ] Unit tests (list specific cases)
- [ ] Integration tests (list scenarios)
- [ ] Devnet harness validation (describe expected outcome)
- [ ] Backward compatibility tests (if applicable)

Define acceptance criteria: what must pass for the implementation to be considered complete?

---

## Rollout

1. **Version bump:** State the exact version transition (e.g. protocol v0.2 → v0.3).
2. **Lock document update:** Which lock document sections change?
3. **Git tag:** What tag will be created?
4. **Coordination:** Do all nodes need to upgrade simultaneously, or is there a grace period?
5. **Rollback plan:** How do we revert if the change causes issues post-release?
