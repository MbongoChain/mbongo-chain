# Phase 2 Governance Rules

This document supplements the root [CONTRIBUTING.md](../CONTRIBUTING.md) with Phase 2-specific governance.

---

## Branching Strategy

- **main:** Protected. Audited milestones only. No direct commits.
- **dev:** Active development. All Phase 2 PRs target `dev`.
- Feature branches: `feature/<name>` or `fix/<name>`. Branch from `dev`, merge back to `dev`.

---

## PR Requirements

- All PRs must target `dev`.
- PRs must pass CI (fmt, clippy, test).
- At least one maintainer approval required.
- PR template must be completed (scope, security checklist, AI disclosure).
- No PRs that modify Phase 1 frozen scope without explicit approval.

---

## Commit Conventions

Use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat(scope):` New feature
- `fix(scope):` Bug fix
- `docs(scope):` Documentation only
- `refactor(scope):` Code change, no behavior change
- `test(scope):` Test addition or update
- `chore(scope):` Build, CI, dependencies

Scope examples: `core`, `network`, `storage`, `api`, `cli`.

---

## RPC Freeze Rule

RPC methods defined in `docs/specs/rpc_v0.1.md` are FROZEN. Breaking changes require a version bump and a new spec document. Do not remove or change the signature of frozen methods without governance approval.

---

## Storage Invariant Protection Rule

Changes to storage logic must preserve invariants documented in `docs/architecture/storage_invariants.md`. PRs that touch storage must:

- Reference the invariants in the description
- Include tests that assert invariants hold
- Use `write_batch` for atomic multi-key updates

Violations will block merge.
