import { GITHUB } from "@/app/constants";

export const metadata = {
  title: "Roadmap",
};

const phases = [
  {
    id: 1,
    name: "Phase 1 — Foundation",
    status: "completed" as const,
    description:
      "Core protocol primitives. Merged to main and frozen. Read-only.",
    items: [
      "Transaction structure, signing, and verification",
      "Cryptography (BLAKE3 hashing, Ed25519 signatures)",
      "Account model (balance, nonce, storage)",
      "Deterministic state transitions",
      "Replay protection and address format",
      "Block structure and serialization",
    ],
  },
  {
    id: 2,
    name: "Phase 2 — Developer Tooling & Infrastructure",
    status: "active" as const,
    description:
      "Active development targeting the dev branch. Open for contributions.",
    items: [
      "CLI tooling for node management and wallet operations",
      "Testing framework and QA pipeline",
      "CI/CD and infrastructure automation",
      "Testnet deployment and orchestration",
      "AI / GPU compute exploration and PoUW prototyping",
      "SDK scaffolding (TypeScript, Python)",
    ],
  },
  {
    id: 3,
    name: "Phase 3 — Consensus & Security",
    status: "planned" as const,
    description:
      "Consensus mechanism implementation and security hardening.",
    items: [
      "PoUW (Proof of Useful Work) consensus finalization",
      "Validator set management and rotation",
      "Fork choice rule and finality gadget",
      "TEE integration for compute attestation",
      "Security audits and formal verification",
    ],
  },
  {
    id: 4,
    name: "Phase 4 — Network & Testnet",
    status: "planned" as const,
    description:
      "Node infrastructure, syncing, and public testnet launch.",
    items: [
      "Full node and guardian node implementation",
      "Block sync and state sync protocols",
      "Public devnet with explorer and faucet",
      "Telemetry and monitoring infrastructure",
      "RPC and API endpoints",
    ],
  },
];

function StatusBadge({ status }: { status: "completed" | "active" | "planned" }) {
  if (status === "completed") {
    return (
      <span className="inline-flex items-center gap-1.5 rounded-full bg-mbongo-100 dark:bg-mbongo-950/40 text-mbongo-700 dark:text-mbongo-400 px-3 py-1 text-xs font-medium">
        <svg
          className="h-3 w-3"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={3}
        >
          <polyline points="20 6 9 17 4 12" />
        </svg>
        Completed
      </span>
    );
  }
  if (status === "active") {
    return (
      <span className="inline-flex items-center gap-1.5 rounded-full bg-mbongo-100 dark:bg-mbongo-950/40 text-mbongo-700 dark:text-mbongo-400 px-3 py-1 text-xs font-medium">
        <span className="relative flex h-2 w-2">
          <span className="animate-pulse-dot absolute inline-flex h-full w-full rounded-full bg-mbongo-500 opacity-75" />
          <span className="relative inline-flex h-2 w-2 rounded-full bg-mbongo-500" />
        </span>
        Active
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400 px-3 py-1 text-xs font-medium">
      Planned
    </span>
  );
}

export default function RoadmapPage() {
  return (
    <article className="mx-auto max-w-5xl px-4 sm:px-6 lg:px-8 py-16 sm:py-24">
      <header className="mb-16">
        <h1 className="text-3xl sm:text-4xl font-bold text-slate-900 dark:text-slate-100 mb-4">
          Roadmap
        </h1>
        <p className="text-lg text-slate-600 dark:text-slate-400 max-w-2xl">
          Development phases for Mbongo Chain. No fixed dates beyond completed
          milestones. Scope is defined by the repository.
        </p>
      </header>

      <section className="relative">
        {/* Timeline line */}
        <div
          className="absolute left-[15px] top-2 bottom-2 w-px bg-slate-200 dark:bg-slate-800 hidden sm:block"
          aria-hidden="true"
        />

        <div className="space-y-12">
          {phases.map((phase) => (
            <div key={phase.id} className="relative sm:pl-12">
              {/* Timeline dot */}
              <div
                className={`absolute left-0 top-1 hidden sm:flex h-[31px] w-[31px] items-center justify-center rounded-full border-2 ${
                  phase.status === "completed"
                    ? "border-mbongo-500 bg-mbongo-50 dark:bg-mbongo-950/40"
                    : phase.status === "active"
                      ? "border-mbongo-500 bg-white dark:bg-[#0b1120]"
                      : "border-slate-300 dark:border-slate-700 bg-white dark:bg-[#0b1120]"
                }`}
              >
                {phase.status === "completed" ? (
                  <svg
                    className="h-4 w-4 text-mbongo-600 dark:text-mbongo-400"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={3}
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                ) : phase.status === "active" ? (
                  <span className="relative flex h-2.5 w-2.5">
                    <span className="animate-pulse-dot absolute inline-flex h-full w-full rounded-full bg-mbongo-500 opacity-75" />
                    <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-mbongo-500" />
                  </span>
                ) : (
                  <span className="h-2 w-2 rounded-full bg-slate-300 dark:bg-slate-600" />
                )}
              </div>

              <div
                className={`rounded-lg border p-6 ${
                  phase.status === "active"
                    ? "border-mbongo-500/30 dark:border-mbongo-500/20 bg-mbongo-50/30 dark:bg-mbongo-950/10"
                    : "border-slate-200 dark:border-slate-800"
                }`}
              >
                <div className="flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 mb-3">
                  <h2 className="text-xl font-semibold text-slate-900 dark:text-slate-100">
                    {phase.name}
                  </h2>
                  <StatusBadge status={phase.status} />
                </div>
                <p className="text-sm text-slate-500 dark:text-slate-500 mb-4">
                  {phase.description}
                </p>
                <ul className="space-y-2">
                  {phase.items.map((item) => (
                    <li
                      key={item}
                      className="flex items-start gap-2 text-sm text-slate-600 dark:text-slate-400"
                    >
                      <span
                        className={`mt-1.5 h-1.5 w-1.5 rounded-full shrink-0 ${
                          phase.status === "completed"
                            ? "bg-mbongo-500"
                            : phase.status === "active"
                              ? "bg-mbongo-400"
                              : "bg-slate-300 dark:bg-slate-600"
                        }`}
                      />
                      {item}
                    </li>
                  ))}
                </ul>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="mt-16 rounded-lg border border-slate-200 dark:border-slate-800 p-6 bg-slate-50 dark:bg-slate-900/50">
        <p className="text-sm text-slate-500 dark:text-slate-500">
          This roadmap reflects the current development plan. Timelines for
          Phase 3+ will be published when scope is finalized. For details, see
          the{" "}
          <a
            href={`${GITHUB.repo}/blob/main/docs/roadmap.md`}
            target="_blank"
            rel="noopener noreferrer"
            className="text-mbongo-600 dark:text-mbongo-400 hover:underline"
          >
            full roadmap document
          </a>{" "}
          in the repository.
        </p>
      </section>
    </article>
  );
}
