import Link from "next/link";
import { SITE, GITHUB, NAV_LINKS } from "@/app/constants";

export default function Footer() {
  return (
    <footer className="border-t border-slate-200 dark:border-slate-800 mt-24 bg-slate-50 dark:bg-[#080d1a]">
      <div className="mx-auto max-w-5xl px-4 sm:px-6 lg:px-8 py-12">
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-8">
          {/* Branding */}
          <div>
            <Link
              href="/"
              className="flex items-center gap-2 text-base font-semibold text-slate-900 dark:text-slate-100"
            >
              <span
                className="inline-block h-5 w-5 rounded bg-mbongo-500"
                aria-hidden="true"
              />
              {SITE.name}
            </Link>
            <p className="mt-3 text-sm text-slate-500 dark:text-slate-500 leading-relaxed">
              Open-source Layer-1 blockchain.
              <br />
              No hype. No promises.
            </p>
          </div>

          {/* Navigation */}
          <div>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500 mb-3">
              Navigation
            </h3>
            <ul className="space-y-2">
              {NAV_LINKS.map((item) => (
                <li key={item.href}>
                  <Link
                    href={item.href}
                    className="text-sm text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100 transition-colors"
                  >
                    {item.label}
                  </Link>
                </li>
              ))}
            </ul>
          </div>

          {/* Resources */}
          <div>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500 mb-3">
              Resources
            </h3>
            <ul className="space-y-2">
              <li>
                <a
                  href={GITHUB.repo}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100 transition-colors"
                >
                  GitHub Repository
                </a>
              </li>
              <li>
                <a
                  href={GITHUB.contributing}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100 transition-colors"
                >
                  Contributing Guide
                </a>
              </li>
              <li>
                <a
                  href={GITHUB.docs.bountyLedger}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100 transition-colors"
                >
                  Bounty Ledger
                </a>
              </li>
            </ul>
          </div>
        </div>

        <div className="mt-10 pt-6 border-t border-slate-200 dark:border-slate-800">
          <p className="text-xs text-slate-400 dark:text-slate-600 text-center">
            {SITE.name} is open-source software. Content aligns with the{" "}
            <a
              href={GITHUB.repo}
              target="_blank"
              rel="noopener noreferrer"
              className="underline hover:text-slate-600 dark:hover:text-slate-400 transition-colors"
            >
              repository
            </a>
            .
          </p>
        </div>
      </div>
    </footer>
  );
}
