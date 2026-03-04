import { GITHUB } from "@/app/constants";

export const metadata = {
  title: "Documentation",
};

const docCategories = [
  {
    title: "Getting Started",
    items: [
      {
        label: "README",
        href: GITHUB.readme,
        path: "README.md",
        desc: "Project overview, prerequisites, and build instructions.",
      },
      {
        label: "Getting Started Guide",
        href: GITHUB.docs.gettingStarted,
        path: "docs/getting_started.md",
        desc: "Quick-start guide for setting up the development environment.",
      },
      {
        label: "Onboarding",
        href: GITHUB.docs.onboarding,
        path: "docs/onboarding.md",
        desc: "Step-by-step guide for new contributors joining Phase 2.",
      },
    ],
  },
  {
    title: "Contributing",
    items: [
      {
        label: "Contributing Guide",
        href: GITHUB.contributing,
        path: "CONTRIBUTING.md",
        desc: "Branch policy, PR workflow, and contribution guidelines.",
      },
      {
        label: "Recruitment Circles",
        href: GITHUB.docs.recruitment,
        path: "docs/recruitment.md",
        desc: "Contribution circles, skill areas, and role definitions.",
      },
      {
        label: "Public Bounty Ledger",
        href: GITHUB.docs.bountyLedger,
        path: "docs/BOUNTY_LEDGER_PUBLIC.md",
        desc: "Append-only record of earned bounties for all contributors.",
      },
    ],
  },
  {
    title: "Architecture",
    items: [
      {
        label: "Vision",
        href: GITHUB.docs.vision,
        path: "docs/vision.md",
        desc: "Project mission, design pillars, and consensus model overview.",
      },
      {
        label: "Architecture Overview",
        href: GITHUB.docs.architecture,
        path: "docs/architecture_master_overview.md",
        desc: "Full system architecture, crate structure, and module design.",
      },
    ],
  },
];

export default function DocsPage() {
  return (
    <article className="mx-auto max-w-5xl px-4 sm:px-6 lg:px-8 py-16 sm:py-24">
      <header className="mb-16">
        <h1 className="text-3xl sm:text-4xl font-bold text-slate-900 dark:text-slate-100 mb-4">
          Documentation
        </h1>
        <p className="text-lg text-slate-600 dark:text-slate-400 max-w-2xl">
          Documentation lives in the repository. All links point to the main
          branch on GitHub.
        </p>
      </header>

      <section className="space-y-12">
        {docCategories.map((category) => (
          <div key={category.title}>
            <h2 className="text-xs font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500 mb-4">
              {category.title}
            </h2>
            <div className="grid gap-3">
              {category.items.map((doc) => (
                <a
                  key={doc.href}
                  href={doc.href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="group block rounded-lg border border-slate-200 dark:border-slate-800 p-5 hover:border-mbongo-500/50 dark:hover:border-mbongo-500/30 hover:bg-slate-50 dark:hover:bg-slate-800/30 transition-colors"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <span className="font-medium text-slate-900 dark:text-slate-100 group-hover:text-mbongo-600 dark:group-hover:text-mbongo-400 transition-colors">
                        {doc.label}
                      </span>
                      <span className="block text-sm text-slate-500 dark:text-slate-500 mt-1">
                        {doc.desc}
                      </span>
                    </div>
                    <span className="shrink-0 text-xs font-mono text-slate-400 dark:text-slate-600 bg-slate-100 dark:bg-slate-800 px-2 py-1 rounded hidden sm:block">
                      {doc.path}
                    </span>
                  </div>
                </a>
              ))}
            </div>
          </div>
        ))}
      </section>

      <section className="mt-16 rounded-lg border border-slate-200 dark:border-slate-800 p-6 bg-slate-50 dark:bg-slate-900/50">
        <p className="text-sm text-slate-500 dark:text-slate-500">
          The full documentation index is available at{" "}
          <a
            href={`${GITHUB.repo}/blob/main/docs/INDEX.md`}
            target="_blank"
            rel="noopener noreferrer"
            className="text-mbongo-600 dark:text-mbongo-400 hover:underline"
          >
            docs/INDEX.md
          </a>
          . The repository contains 70+ documentation files covering
          architecture, consensus, economics, tooling, and more.
        </p>
      </section>
    </article>
  );
}
