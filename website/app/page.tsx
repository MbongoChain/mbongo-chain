import Link from "next/link";
import { SITE, GITHUB } from "./constants";

export default function HomePage() {
  return (
    <>
      {/* Hero */}
      <section className="relative overflow-hidden">
        <div className="absolute inset-0 -z-10 bg-[radial-gradient(45%_50%_at_50%_0%,rgba(34,197,94,0.08),transparent)] dark:bg-[radial-gradient(45%_50%_at_50%_0%,rgba(34,197,94,0.04),transparent)]" />
        <div className="mx-auto max-w-5xl px-4 sm:px-6 lg:px-8 pt-20 pb-16 sm:pt-32 sm:pb-24 text-center">
          <div className="inline-flex items-center gap-2 rounded-full border border-mbongo-200 dark:border-mbongo-900 bg-mbongo-50 dark:bg-mbongo-950/30 px-4 py-1.5 text-sm text-mbongo-700 dark:text-mbongo-400 mb-8">
            <span className="relative flex h-2 w-2">
              <span className="animate-pulse-dot absolute inline-flex h-full w-full rounded-full bg-mbongo-500 opacity-75" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-mbongo-500" />
            </span>
            Phase 2 Active
          </div>

          <h1 className="text-4xl sm:text-5xl lg:text-6xl font-bold tracking-tight text-slate-900 dark:text-slate-100 mb-6 text-balance">
            {SITE.name}
          </h1>

          <p className="text-lg sm:text-xl text-slate-600 dark:text-slate-400 max-w-2xl mx-auto mb-10 leading-relaxed">
            {SITE.description}
          </p>

          <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
            <a
              href={GITHUB.repo}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 rounded-lg bg-slate-900 dark:bg-slate-100 text-white dark:text-slate-900 px-6 py-3 font-medium hover:bg-slate-800 dark:hover:bg-slate-200 transition-colors"
            >
              <svg
                className="h-5 w-5"
                viewBox="0 0 16 16"
                fill="currentColor"
                aria-hidden="true"
              >
                <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
              </svg>
              GitHub Repository
            </a>
            <Link
              href="/contribute/"
              className="inline-flex items-center justify-center rounded-lg border-2 border-slate-300 dark:border-slate-600 text-slate-900 dark:text-slate-100 px-6 py-3 font-medium hover:border-mbongo-500 hover:text-mbongo-600 dark:hover:text-mbongo-400 transition-colors"
            >
              Contribute
            </Link>
          </div>
        </div>
      </section>

      {/* Content sections */}
      <article className="mx-auto max-w-5xl px-4 sm:px-6 lg:px-8 pb-16 sm:pb-24">
        {/* What is Mbongo Chain */}
        <section className="mb-16">
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
            What is Mbongo Chain
          </h2>
          <p className="text-slate-600 dark:text-slate-400 leading-relaxed max-w-3xl">
            Mbongo Chain is a Rust-native Layer-1 blockchain designed for
            verifiable compute. Instead of executing heavy workloads directly
            on-chain, Mbongo verifies off-chain computation through deterministic
            receipts and replay validation. The protocol combines Proof of Stake
            with Proof of Useful Work concepts to secure verification of AI
            inference and other compute workloads.
          </p>
        </section>

        {/* Current Status */}
        <section className="mb-16">
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
            Current Status
          </h2>
          <div className="grid sm:grid-cols-2 gap-4">
            <div className="rounded-lg border border-slate-200 dark:border-slate-800 p-5">
              <div className="flex items-center gap-2 mb-2">
                <span className="inline-flex h-5 w-5 items-center justify-center rounded-full bg-mbongo-100 dark:bg-mbongo-950/40">
                  <svg
                    className="h-3 w-3 text-mbongo-600 dark:text-mbongo-400"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={3}
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                </span>
                <h3 className="font-medium text-slate-900 dark:text-slate-100">
                  Phase 1 — Foundation
                </h3>
              </div>
              <p className="text-sm text-slate-500 dark:text-slate-500">
                Complete. Core data structures, transactions, cryptography,
                accounts, and state storage are implemented and merged.
              </p>
            </div>
            <div className="rounded-lg border-2 border-mbongo-500/30 dark:border-mbongo-500/20 bg-mbongo-50/50 dark:bg-mbongo-950/10 p-5">
              <div className="flex items-center gap-2 mb-2">
                <span className="relative flex h-2.5 w-2.5">
                  <span className="animate-pulse-dot absolute inline-flex h-full w-full rounded-full bg-mbongo-500 opacity-75" />
                  <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-mbongo-500" />
                </span>
                <h3 className="font-medium text-slate-900 dark:text-slate-100">
                  Phase 2 — Active
                </h3>
              </div>
              <p className="text-sm text-slate-600 dark:text-slate-400">
                Tooling, testing, infrastructure, and verifiable compute. All
                contributions target the <code>dev</code> branch.
              </p>
            </div>
          </div>
        </section>

        {/* Why it exists */}
        <section className="mb-16">
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-6">
            Why It Exists
          </h2>
          <div className="grid sm:grid-cols-3 gap-6">
            {[
              {
                title: "Compute",
                desc: "AI inference and scientific compute are increasingly performed off-chain. Mbongo enables verifiable results by allowing independent nodes to verify computation receipts deterministically.",
                icon: (
                  <svg
                    className="h-5 w-5"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={1.5}
                  >
                    <rect x={4} y={4} width={16} height={16} rx={2} />
                    <rect x={9} y={9} width={6} height={6} />
                    <line x1={9} y1={1} x2={9} y2={4} />
                    <line x1={15} y1={1} x2={15} y2={4} />
                    <line x1={9} y1={20} x2={9} y2={23} />
                    <line x1={15} y1={20} x2={15} y2={23} />
                    <line x1={20} y1={9} x2={23} y2={9} />
                    <line x1={20} y1={14} x2={23} y2={14} />
                    <line x1={1} y1={9} x2={4} y2={9} />
                    <line x1={1} y1={14} x2={4} y2={14} />
                  </svg>
                ),
              },
              {
                title: "Openness",
                desc: "Anyone can contribute. The codebase, documentation, and bounty ledger are public and append-only. No gatekeeping.",
                icon: (
                  <svg
                    className="h-5 w-5"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={1.5}
                  >
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2z" />
                    <line x1={2} y1={12} x2={22} y2={12} />
                    <path d="M12 2c2.5 2.5 4 6 4 10s-1.5 7.5-4 10c-2.5-2.5-4-6-4-10s1.5-7.5 4-10z" />
                  </svg>
                ),
              },
              {
                title: "Correctness",
                desc: "Deterministic execution and receipt verification ensure that off-chain compute results can be independently validated.",
                icon: (
                  <svg
                    className="h-5 w-5"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={1.5}
                  >
                    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                    <polyline points="9 12 11 14 15 10" />
                  </svg>
                ),
              },
            ].map((item) => (
              <div
                key={item.title}
                className="rounded-lg border border-slate-200 dark:border-slate-800 p-5 hover:border-slate-300 dark:hover:border-slate-700 transition-colors"
              >
                <div className="inline-flex h-9 w-9 items-center justify-center rounded-lg bg-mbongo-50 dark:bg-mbongo-950/30 text-mbongo-600 dark:text-mbongo-400 mb-3">
                  {item.icon}
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
        </section>

        {/* How Verification Works */}
        <section className="mb-16">
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
            How Verification Works
          </h2>
          <p className="text-slate-600 dark:text-slate-400 leading-relaxed max-w-3xl mb-6">
            Mbongo Chain verifies off-chain computation using deterministic
            receipts and validator verification.
          </p>
          <div className="grid sm:grid-cols-2 gap-6">
            {[
              {
                step: "1",
                title: "Off-chain compute",
                desc: "AI inference or other compute workloads run off-chain. These workloads may include machine learning inference, simulations, or other deterministic compute tasks.",
              },
              {
                step: "2",
                title: "Compute receipt",
                desc: "The compute node generates a deterministic receipt describing the execution and its result.",
              },
              {
                step: "3",
                title: "Validator verification",
                desc: "Independent validators verify the receipt using deterministic replay or equivalent verification methods.",
              },
              {
                step: "4",
                title: "Settlement on-chain",
                desc: "Once verification succeeds, the result can be finalized and recorded on Mbongo Chain.",
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
        </section>

        {/* Tech summary */}
        <section>
          <h2 className="text-2xl font-semibold text-slate-900 dark:text-slate-100 mb-4">
            Technical Foundation
          </h2>
          <div className="rounded-lg border border-slate-200 dark:border-slate-800 p-6 bg-slate-50 dark:bg-slate-900/50">
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-6 text-center">
              {[
                { label: "core protocol implementation", value: "Rust" },
                { label: "hashing", value: "BLAKE3" },
                { label: "signatures", value: "Ed25519" },
                { label: "consensus and verification model", value: "PoS + Verifiable Compute" },
              ].map((item) => (
                <div key={item.label}>
                  <p className="text-lg font-semibold font-mono text-mbongo-600 dark:text-mbongo-400">
                    {item.value}
                  </p>
                  <p className="text-xs text-slate-500 dark:text-slate-500 mt-1">
                    {item.label}
                  </p>
                </div>
              ))}
            </div>
          </div>
        </section>
      </article>
    </>
  );
}
