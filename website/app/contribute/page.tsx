import Link from "next/link";
import { GITHUB } from "@/app/constants";

export const metadata = {
  title: "Contribute",
};

const circles = [
  {
    name: "Circle 1 — Rust Core",
    tag: "Rust",
    tagColor: "bg-orange-100 dark:bg-orange-950/30 text-orange-700 dark:text-orange-400",
    description:
      "Consensus, networking, runtime, state management, verification. Requires advanced Rust and distributed systems knowledge.",
    areas: [
      "TEE integration and compute attestation",
      "P2P networking and gossip protocols",
      "State persistence and Merkle proofs",
      "Consensus finality and fork choice",
    ],
  },
  {
    name: "Circle 2 — Technical Contributors",
    tag: "Multi-lang",
    tagColor: "bg-blue-100 dark:bg-blue-950/30 text-blue-700 dark:text-blue-400",
    description:
      "APIs, SDKs, tooling, and networking in JavaScript/TypeScript, Python, Go, or Rust.",
    areas: [
      "REST / JSON-RPC API endpoints",
      "TypeScript and Python SDK development",
      "CLI tooling and developer experience",
      "WebSocket systems and event streaming",
    ],
  },
  {
    name: "Circle 3 — Specialists",
    tag: "No Rust required",
    tagColor: "bg-purple-100 dark:bg-purple-950/30 text-purple-700 dark:text-purple-400",
    description:
      "AI/ML, security, economics, DevOps, documentation. No Rust knowledge required.",
    areas: [
      "AI inference and compute verification (CUDA, Python)",
      "Security audits and threat modeling",
      "CI/CD pipelines and infrastructure automation",
      "Technical documentation and guides",
    ],
  },
];

export default function ContributePage() {
  return (
    <article className="mx-auto max-w-5xl px-4 sm:px-6 lg:px-8 py-16 sm:py-24">
      <header className="mb-16">
        <h1 className="text-3xl sm:text-4xl font-bold text-slate-900 dark:text-slate-100 mb-4">
          Contribute
        </h1>
        <p className="text-lg text-slate-600 dark:text-slate-400 max-w-2xl">
          Mbongo Chain welcomes contributors across all skill levels.{" "}
          <strong className="text-slate-900 dark:text-slate-100">
            Rust is required only for core protocol work.
          </strong>
        </p>
      </header>

      {/* Contribution paths */}
      <section className="mb-16">
        <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-6">
          Contribution Paths
        </h2>
        <div className="grid gap-6">
          {circles.map((circle) => (
            <div
              key={circle.name}
              className="rounded-lg border border-slate-200 dark:border-slate-800 p-6 hover:border-slate-300 dark:hover:border-slate-700 transition-colors"
            >
              <div className="flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-3 mb-3">
                <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
                  {circle.name}
                </h3>
                <span
                  className={`inline-flex w-fit rounded-full px-2.5 py-0.5 text-xs font-medium ${circle.tagColor}`}
                >
                  {circle.tag}
                </span>
              </div>
              <p className="text-sm text-slate-600 dark:text-slate-400 mb-4">
                {circle.description}
              </p>
              <ul className="grid sm:grid-cols-2 gap-2">
                {circle.areas.map((area) => (
                  <li
                    key={area}
                    className="flex items-start gap-2 text-sm text-slate-500 dark:text-slate-500"
                  >
                    <span className="mt-1.5 h-1.5 w-1.5 rounded-full bg-mbongo-400 shrink-0" />
                    {area}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </section>

      {/* Rules */}
      <section className="mb-16">
        <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
          Branch &amp; Workflow Rules
        </h2>
        <div className="rounded-lg border border-slate-200 dark:border-slate-800 p-6 bg-slate-50 dark:bg-slate-900/50">
          <div className="grid sm:grid-cols-2 gap-6">
            <div>
              <h3 className="font-medium text-slate-900 dark:text-slate-100 mb-2">
                Branch Policy
              </h3>
              <ul className="space-y-2 text-sm text-slate-600 dark:text-slate-400">
                <li className="flex items-start gap-2">
                  <span className="mt-0.5 text-mbongo-500">&#x2192;</span>
                  All work targets the <code>dev</code> branch.
                </li>
                <li className="flex items-start gap-2">
                  <span className="mt-0.5 text-red-500">&#x2717;</span>
                  No direct commits to <code>main</code>.
                </li>
                <li className="flex items-start gap-2">
                  <span className="mt-0.5 text-mbongo-500">&#x2192;</span>
                  <code>main</code> is protected — stable milestones only.
                </li>
              </ul>
            </div>
            <div>
              <h3 className="font-medium text-slate-900 dark:text-slate-100 mb-2">
                Getting Started
              </h3>
              <ol className="space-y-2 text-sm text-slate-600 dark:text-slate-400 list-decimal list-inside">
                <li>Read the onboarding guide.</li>
                <li>
                  Pick an issue labeled{" "}
                  <code>phase-2</code>.
                </li>
                <li>Fork, branch from <code>dev</code>, and submit a PR.</li>
                <li>
                  PRs are reviewed by maintainers. Core protocol changes require
                  Circle 1 approval.
                </li>
              </ol>
            </div>
          </div>
        </div>
      </section>

      {/* Quick links */}
      <section>
        <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
          Quick Links
        </h2>
        <div className="grid sm:grid-cols-3 gap-4">
          {[
            {
              label: "GitHub Issues (phase-2)",
              href: GITHUB.issuesPhase2,
              desc: "Browse open tasks",
            },
            {
              label: "CONTRIBUTING.md",
              href: GITHUB.contributing,
              desc: "Contribution guidelines",
            },
            {
              label: "Onboarding Guide",
              href: GITHUB.docs.onboarding,
              desc: "Step-by-step setup",
            },
          ].map((link) => (
            <a
              key={link.href}
              href={link.href}
              target="_blank"
              rel="noopener noreferrer"
              className="group block rounded-lg border border-slate-200 dark:border-slate-800 p-5 hover:border-mbongo-500/50 dark:hover:border-mbongo-500/30 transition-colors"
            >
              <span className="font-medium text-slate-900 dark:text-slate-100 group-hover:text-mbongo-600 dark:group-hover:text-mbongo-400 transition-colors">
                {link.label}
              </span>
              <span className="block text-sm text-slate-500 dark:text-slate-500 mt-1">
                {link.desc}
              </span>
            </a>
          ))}
        </div>
      </section>
    </article>
  );
}
