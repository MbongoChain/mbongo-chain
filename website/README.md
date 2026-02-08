# Mbongo Chain — Website

Developer-first website for [Mbongo Chain](https://github.com/MbongoChain/mbongo-chain). Static site built with Next.js and Tailwind CSS.

No hype. No fundraising language. Content aligns with the repository.

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

### Vercel

1. Push the repo to GitHub.
2. Import the project in [Vercel](https://vercel.com).
3. Set **Root Directory** to `website`.
4. Deploy. No environment variables required.

### Cloudflare Pages

1. Push the repo to GitHub.
2. Create a new Pages project in Cloudflare.
3. Set **Build command**: `cd website && npm install && npm run build`
4. Set **Build output directory**: `website/out`
5. Deploy.

### Manual

```bash
cd website
npm install
npm run build
# Upload contents of out/ to your host
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
