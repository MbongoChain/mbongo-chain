# Public Bounty Ledger — Mbongo Chain

This document is the **single source of truth** for all contributor bounties in the Mbongo Chain project. It is public, append-only, and committed to the repository.

No private balances exist. No DM confirmations are valid. If a bounty is not listed here, it is not recognized.

---

## Status Definitions

| Status | Meaning |
|--------|---------|
| **Spec** | Bounty specified in an open GitHub issue. Work not yet merged. No bounty earned. |
| **Earned** | PR merged. Bounty recorded. Settlement occurs at TGE. |
| **Settled** | MBO tokens delivered to the contributor after TGE. |

---

## Ledger

| Contributor | Issue # | Task | Phase | Bounty (MBO) | Status | PR | Date (UTC) |
|---|---|---|---|---|---|---|---|
| [@shivam123-dev](https://github.com/shivam123-dev) | #24, #28 | JSON-RPC Server + REST Endpoints | Phase 4 | 13,800 | Earned | [#42](https://github.com/MbongoChain/mbongo-chain/pull/42) | 2025-12-15 |

New earned bounties are appended to the bottom of this table.

---

## How to Track Your Bounties

Contributors can verify their accumulated bounties at any time by reading this file. There is no other mechanism.

- **This ledger is the only record.** No private dashboards, spreadsheets, or DMs serve as confirmation.
- **Search by your GitHub handle.** Your total earned MBO is the sum of all rows with your username and status `Earned` or `Settled`.
- **If your PR was merged and no row exists**, open a GitHub Issue referencing the PR and the original bounty issue. Maintainers will append the entry after verification.
- **Do not rely on verbal or written confirmations outside this file.** Only rows in this table are binding.

---

## Governance Guardrails

1. **Ledger updates occur only after PR merge.** No entry is added for open PRs, draft PRs, or claimed issues. The triggering event is merge to `dev` (or `main` for stable milestones).

2. **Append-only.** Existing rows are not edited or deleted. If a correction is needed (e.g., wrong amount, attribution error), a new correction row is appended with an explanation, and the original row is preserved.

3. **Disputes are handled via GitHub.** Open a GitHub Issue or Discussion referencing the ledger row in question. Disputes are resolved by maintainers based on PR history, issue scope, and merge records.

4. **No retroactive changes.** Bounty amounts are set when the issue is created. They are not adjusted after the PR is merged. No bonus or penalty is applied retroactively.

5. **No off-ledger payments.** All MBO-denominated contributor compensation goes through this ledger. Fiat, equity, and other arrangements (if any) are outside the scope of this document.

---

## Rules

1. **No payments before TGE.** All bounties marked `Earned` are committed for settlement when MBO is live. No disbursement occurs before token launch.

2. **Scope.** This ledger covers bounties denominated in MBO for merged contributions to the Mbongo Chain repository.

3. **Eligibility.** Only contributions to the `dev` branch (Phase 2+) or approved `main` merges are eligible. Phase 1 is frozen.

---

*Last updated: 2025-12-15. This file is append-only. New earned bounties are appended to the ledger table above.*
