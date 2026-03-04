import Link from "next/link";
import { GITHUB } from "@/app/constants";

export const metadata = {
  title: "Bounties",
};

export default function BountiesPage() {
  return (
    <article className="mx-auto max-w-5xl px-4 sm:px-6 lg:px-8 py-16 sm:py-24">
      <header className="mb-16">
        <h1 className="text-3xl sm:text-4xl font-bold text-slate-900 dark:text-slate-100 mb-4">
          Bounties
        </h1>
        <p className="text-lg text-slate-600 dark:text-slate-400 max-w-2xl">
          Bounties are denominated in MBO. The ledger is public, append-only,
          and is the single source of truth.
        </p>
      </header>

      <section className="space-y-12">
        {/* How it works */}
        <div>
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-6">
            How It Works
          </h2>
          <div className="grid sm:grid-cols-3 gap-4">
            {[
              {
                step: "1",
                title: "Claim an Issue",
                desc: "Pick a GitHub issue labeled phase-2 and submit a PR targeting the dev branch.",
              },
              {
                step: "2",
                title: "Merge = Earned",
                desc: "When your PR is merged, the bounty is recorded in the public ledger as \"Earned.\" No bounty before merge.",
              },
              {
                step: "3",
                title: "Settlement when the Mbongo token is introduced on-chain.",
                desc: "No payments before the token is live. All earned bounties are committed for settlement when MBO is introduced on-chain.",
              },
            ].map((item) => (
              <div
                key={item.step}
                className="rounded-lg border border-slate-200 dark:border-slate-800 p-5"
              >
                <div className="inline-flex h-7 w-7 items-center justify-center rounded-full bg-mbongo-100 dark:bg-mbongo-950/40 text-mbongo-700 dark:text-mbongo-400 text-sm font-semibold mb-3">
                  {item.step}
                </div>
                <h3 className="font-semibold text-slate-900 dark:text-slate-100 mb-2">
                  {item.title}
                </h3>
                <p className="text-sm text-slate-600 dark:text-slate-400 leading-relaxed">
                  {item.desc}
                </p>
              </div>
            ))}
          </div>
        </div>

        {/* Ledger details */}
        <div>
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
            Ledger Guarantees
          </h2>
          <div className="rounded-lg border border-slate-200 dark:border-slate-800 p-6">
            <ul className="space-y-3 text-slate-600 dark:text-slate-400">
              <li className="flex items-start gap-3">
                <svg
                  className="h-5 w-5 text-mbongo-500 shrink-0 mt-0.5"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
                <span>
                  <strong className="text-slate-900 dark:text-slate-100">
                    Append-only.
                  </strong>{" "}
                  No entries are edited or deleted. New earned bounties are
                  appended.
                </span>
              </li>
              <li className="flex items-start gap-3">
                <svg
                  className="h-5 w-5 text-mbongo-500 shrink-0 mt-0.5"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
                <span>
                  <strong className="text-slate-900 dark:text-slate-100">
                    Public.
                  </strong>{" "}
                  The ledger is committed to the repository and visible to
                  everyone.
                </span>
              </li>
              <li className="flex items-start gap-3">
                <svg
                  className="h-5 w-5 text-mbongo-500 shrink-0 mt-0.5"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
                <span>
                  <strong className="text-slate-900 dark:text-slate-100">
                    Dispute-ready.
                  </strong>{" "}
                  Disputes are resolved through GitHub issues. The ledger
                  includes contributor, issue, amount, PR link, and date.
                </span>
              </li>
            </ul>
          </div>
        </div>

        {/* Bounty statuses */}
        <div>
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
            Status Definitions
          </h2>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-200 dark:border-slate-800">
                  <th className="text-left py-3 pr-4 font-medium text-slate-900 dark:text-slate-100">
                    Status
                  </th>
                  <th className="text-left py-3 font-medium text-slate-900 dark:text-slate-100">
                    Meaning
                  </th>
                </tr>
              </thead>
              <tbody className="text-slate-600 dark:text-slate-400">
                <tr className="border-b border-slate-100 dark:border-slate-800/50">
                  <td className="py-3 pr-4">
                    <span className="inline-flex rounded-full bg-slate-100 dark:bg-slate-800 px-2.5 py-0.5 text-xs font-medium">
                      Spec
                    </span>
                  </td>
                  <td className="py-3">
                    Work specified but not yet merged. No bounty earned.
                  </td>
                </tr>
                <tr className="border-b border-slate-100 dark:border-slate-800/50">
                  <td className="py-3 pr-4">
                    <span className="inline-flex rounded-full bg-amber-100 dark:bg-amber-950/30 text-amber-700 dark:text-amber-400 px-2.5 py-0.5 text-xs font-medium">
                      Earned
                    </span>
                  </td>
                  <td className="py-3">
                    PR merged. Bounty recorded. Awaiting token introduction for settlement.
                  </td>
                </tr>
                <tr>
                  <td className="py-3 pr-4">
                    <span className="inline-flex rounded-full bg-mbongo-100 dark:bg-mbongo-950/40 text-mbongo-700 dark:text-mbongo-400 px-2.5 py-0.5 text-xs font-medium">
                      Settled
                    </span>
                  </td>
                  <td className="py-3">
                    MBO tokens delivered to the contributor.
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        {/* CTA */}
        <div className="flex flex-col sm:flex-row gap-4">
          <a
            href={GITHUB.docs.bountyLedger}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center justify-center gap-2 rounded-lg bg-slate-900 dark:bg-slate-100 text-white dark:text-slate-900 px-6 py-3 font-medium hover:bg-slate-800 dark:hover:bg-slate-200 transition-colors"
          >
            <svg
              className="h-4 w-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
              <line x1={16} y1={13} x2={8} y2={13} />
              <line x1={16} y1={17} x2={8} y2={17} />
            </svg>
            View Public Ledger
          </a>
          <Link
            href="/contribute/"
            className="inline-flex items-center justify-center rounded-lg border-2 border-slate-300 dark:border-slate-600 text-slate-900 dark:text-slate-100 px-6 py-3 font-medium hover:border-mbongo-500 hover:text-mbongo-600 dark:hover:text-mbongo-400 transition-colors"
          >
            Start Contributing
          </Link>
        </div>

        {/* Governance guardrails */}
        <div>
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
            Governance
          </h2>
          <ul className="space-y-2 text-sm text-slate-600 dark:text-slate-400">
            <li className="flex items-start gap-2">
              <span className="mt-0.5 text-slate-400 dark:text-slate-600 shrink-0">&bull;</span>
              Ledger updates occur only after a PR is merged. Open PRs, drafts, and claimed issues do not generate entries.
            </li>
            <li className="flex items-start gap-2">
              <span className="mt-0.5 text-slate-400 dark:text-slate-600 shrink-0">&bull;</span>
              No private balances, no DM confirmations. The ledger file is the only valid record.
            </li>
            <li className="flex items-start gap-2">
              <span className="mt-0.5 text-slate-400 dark:text-slate-600 shrink-0">&bull;</span>
              Disputes are resolved via GitHub Issues or Discussions, referencing the ledger row in question.
            </li>
            <li className="flex items-start gap-2">
              <span className="mt-0.5 text-slate-400 dark:text-slate-600 shrink-0">&bull;</span>
              No retroactive changes to bounty amounts after a PR is merged.
            </li>
          </ul>
        </div>

        {/* Disclaimer */}
        <div className="rounded-lg border border-slate-200 dark:border-slate-800 p-6 bg-slate-50 dark:bg-slate-900/50">
          <p className="text-sm text-slate-500 dark:text-slate-500">
            This page explains the bounty system. It does not constitute a
            promise of payment. Settlement occurs when the Mbongo token is introduced on-chain according to the
            contributor compensation framework documented in the repository.
            Only MBO-denominated work is covered. Contributions must target
            the <code>dev</code> branch.
          </p>
        </div>
      </section>
    </article>
  );
}
