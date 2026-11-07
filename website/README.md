# Mbongo Chain — Website

Developer-first website for [Mbongo Chain](https://github.com/MbongoChain/mbongo-chain). Static site built with Next.js and Tailwind CSS.

No hype. No fundraising language. Content aligns with the repository.

## Production

- **Official URL**: [https://mbongochain.org](https://mbongochain.org)
- **Hosting**: Cloudflare Workers (static export served via Workers)
- **DNS**: Managed via Cloudflare
- **Namecheap/cPanel**: Not used for website hosting. Domain registrar only.

The production deployment is the `out/` directory served by Cloudflare Workers. No server-side rendering, no backend, no API routes.

### The repository does not deploy the site

Nothing in this repository builds or publishes `website/`: no workflow deploys it, and
the Cloudflare Worker's configuration is not kept here. A change merged into `dev`
reaches mbongochain.org only when someone builds `out/` and deploys it by hand. **Each
deployment needs its own authorization.** Merging a pull request is not one.

So the live site can differ from the repository, and nothing records which commit it
was built from. Its pages carry a Next.js build identifier, which is random and does
not map to a revision.

As of 2026-11-07, `source-map-js` (#222) and `nanoid` (#226) are updated in the
repository, and no deployment of them is recorded. Both are build-time dependencies:
neither ships in `out/`, and the exported CSS is byte-identical with and without them.

## Tech Stack

- **Next.js 14** — React framework with App Router
- **Tailwind CSS 3.4** — Utility-first styling
- **TypeScript 5** — Strict type safety

Static export (`output: "export"`). No backend. Deploy to Vercel, Cloudflare Pages, or any static host.

## Features

- Dark mode toggle (persisted, respects system preference)
- Mobile responsive with hamburger menu
- Semantic HTML and accessible navigation
- Active route highlighting
- Content aligned with repository documentation

## Run Locally

```bash
cd website

# Install dependencies
npm install

# Start dev server
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## Build

```bash
npm run build
```

Output is in `out/`. Serve with any static file server:

```bash
npx serve out
```

## Deploy

### Cloudflare Workers (Production)

The official site at [mbongochain.org](https://mbongochain.org) is deployed via Cloudflare Workers serving the static `out/` directory.

```bash
cd website
npm install
npm run build
# Deploy out/ via Cloudflare Workers dashboard or wrangler CLI
```

### Vercel (Alternative)

1. Push the repo to GitHub.
2. Import the project in [Vercel](https://vercel.com).
3. Set **Root Directory** to `website`.
4. Deploy. No environment variables required.

### Manual

```bash
cd website
npm install
npm run build
# Upload contents of out/ to any static host
```

## Pages

| Route | Page | Description |
|---|---|---|
| `/` | Home | Project description, status, and core principles |
| `/roadmap` | Roadmap | Development phases with timeline visualization |
| `/contribute` | Contribute | Contribution circles, rules, and quick links |
| `/bounties` | Bounties | Bounty system, ledger guarantees, and status definitions |
| `/docs` | Documentation | Categorized links to all repository documentation |

## Structure

```
website/
├── app/
│   ├── page.tsx              # Home
│   ├── roadmap/page.tsx      # Roadmap
│   ├── contribute/page.tsx   # Contribute
│   ├── bounties/page.tsx     # Bounties
│   ├── docs/page.tsx         # Docs
│   ├── layout.tsx            # Root layout with ThemeProvider
│   ├── globals.css           # Global styles and CSS variables
│   └── constants.ts          # Site config and GitHub URLs
├── components/
│   ├── Header.tsx            # Navigation with mobile menu
│   ├── Footer.tsx            # Footer with nav + resources
│   ├── ThemeProvider.tsx     # Dark mode context provider
│   └── ThemeToggle.tsx       # Dark/light mode switch
├── package.json
├── tailwind.config.ts
├── next.config.js
├── tsconfig.json
├── postcss.config.js
└── README.md
```
