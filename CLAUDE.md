# Mbongo Chain — Agent Instructions

A Rust blockchain (`crates/`) with a TypeScript SDK (`sdk/typescript/`).
Protocol v0.3, frozen. PRs target `dev`; `main` is reserved for audited
milestones.

## Start here

- [`README.md`](README.md) — what runs today, and how to start it.
- [`docs/INDEX.md`](docs/INDEX.md) — the **authority map**: the one document
  that decides each subject, and which parts of `docs/` are current.

Read the authority map before trusting anything else under `docs/`. Most of
that directory predates the current protocol and describes designs that were
never built. Documents that contradict the running system carry a banner
saying so; others may be wrong without having been checked.

## Sources of truth

Do not restate protocol facts from this file or from memory. Look them up:

| Subject | Where |
|---|---|
| Frozen surfaces, versioning | [`docs/specs/PROTOCOL_LOCK_v0.3.md`](docs/specs/PROTOCOL_LOCK_v0.3.md) |
| RPC contract | [`docs/specs/rpc_v0.2.md`](docs/specs/rpc_v0.2.md) |
| Receipts, anchoring | [`docs/specs/RECEIPT_SPEC_v0.1.md`](docs/specs/RECEIPT_SPEC_v0.1.md), [`docs/rfcs/0002-receipt-anchoring-v0.3.md`](docs/rfcs/0002-receipt-anchoring-v0.3.md) |
| Changing any of the above | [`docs/RFC_PROCESS.md`](docs/RFC_PROCESS.md), [`docs/CONTRIBUTION_TIERS.md`](docs/CONTRIBUTION_TIERS.md) |
| Contributing | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| **What counts as proof here** | [`docs/ENGINEERING_EVIDENCE.md`](docs/ENGINEERING_EVIDENCE.md) |

## Engineering rules

These are summaries. [`docs/ENGINEERING_EVIDENCE.md`](docs/ENGINEERING_EVIDENCE.md)
is the authority for all of them, and explains why each exists.

- **Assert expected cardinality before comparing.** State how many things you
  expect to extract, then check you got that many. A comparison over zero, or
  one of seven, or fifteen where fourteen were expected, is a failed check —
  not a passing one.
- **Prove committed file identity from Git objects**, not from files on disk.
  Checkout filters and line-ending conversion make working-tree comparison
  answer a different question.
- **Bind CI evidence to an exact SHA.** If the source changed after review,
  earlier checks describe code you are no longer shipping. Re-run them.
- **A skipped check is skipped, never successful.** Say which jobs ran.
  Pull-request CI and post-merge CI are separate evidence.
- **Absence is not proof.** No search result, or a 404 from a registry, proves
  nothing about existence, ownership, or control.
- **Say what you proved.** *Declared* and *tested* are different claims. So
  are *observed* and *proven*.
- **Do not use issue-closing keywords** (`Closes`, `Fixes`, `Resolves`) unless
  closing that issue is the intent. Use `Refs #N` for a slice of tracked work.
- **Do not widen scope silently.** If the work is growing past what was asked,
  stop and re-scope explicitly.
- **Do not implement changes to locked surfaces before the RFC is accepted.**
  Check the tier rules first.

## Environment

Development happens on Windows. Both a POSIX shell and PowerShell are
available, and they need different syntax; some tooling behaves differently
here than on the CI runners, which are Linux.

Verify platform-specific behaviour rather than assuming it, and check the
canonical evidence when a result looks surprising. Detailed Windows procedures
are not recorded yet — see #110.

## Skills

None are committed yet. When reusable procedures are added they will live in
`.claude/skills/`. Do not assume a skill exists because a workflow feels
repetitive; check the directory.
