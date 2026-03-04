export const SITE = {
  name: "Mbongo Chain",
  description:
    "A Rust-native Layer-1 focused on deterministic verification of off-chain compute. Mbongo Chain verifies AI inference and other off-chain computation using cryptographic receipts and deterministic replay. No hype. Just verifiable compute.",
  domain: "mbongochain.org",
};

export const GITHUB = {
  repo: "https://github.com/MbongoChain/mbongo-chain",
  issues: "https://github.com/MbongoChain/mbongo-chain/issues",
  issuesPhase2:
    "https://github.com/MbongoChain/mbongo-chain/issues?q=is%3Aissue+label%3Aphase-2",
  contributing:
    "https://github.com/MbongoChain/mbongo-chain/blob/main/CONTRIBUTING.md",
  readme: "https://github.com/MbongoChain/mbongo-chain/blob/main/README.md",
  docs: {
    recruitment:
      "https://github.com/MbongoChain/mbongo-chain/blob/main/docs/recruitment.md",
    onboarding:
      "https://github.com/MbongoChain/mbongo-chain/blob/main/docs/onboarding.md",
    bountyLedger:
      "https://github.com/MbongoChain/mbongo-chain/blob/main/docs/BOUNTY_LEDGER_PUBLIC.md",
    vision:
      "https://github.com/MbongoChain/mbongo-chain/blob/main/docs/vision.md",
    architecture:
      "https://github.com/MbongoChain/mbongo-chain/blob/main/docs/architecture_master_overview.md",
    gettingStarted:
      "https://github.com/MbongoChain/mbongo-chain/blob/main/docs/getting_started.md",
  },
};

export const NAV_LINKS = [
  { href: "/", label: "Home" },
  { href: "/roadmap/", label: "Roadmap" },
  { href: "/contribute/", label: "Contribute" },
  { href: "/bounties/", label: "Bounties" },
  { href: "/docs/", label: "Docs" },
] as const;
