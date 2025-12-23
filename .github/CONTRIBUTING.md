# Contributing to Mbongo Chain

Thank you for considering contributing to Mbongo Chain!

This document describes the development process, how to propose changes, and how to work within the codebase.
## Development Status — Phase 1 (Locked)

Mbongo Chain is currently locked to **Phase 1: Foundation**.

Only protocol-definition work is accepted.

❌ Runtime execution  
❌ State machines  
❌ Storage engines  
❌ Networking / APIs  
❌ Tooling / CLI  

PRs outside Phase 1 scope will be closed without review.

---

## 1. Development Philosophy

Mbongo Chain is:
- Rust-native
- deterministic
- compute-first (PoS + PoUW)
- security-first
- fully open source

Contributions must maintain:
- code clarity
- determinism
- reproducibility
- performance
- safety

---

## 2. How to Contribute

### Bug fixes
Open an Issue and link a PR.

### New features
Create an Issue first so we validate design impact.

### Documentation
Docs are in `/docs`, contributions are welcome.

### Testing
All Rust code must include unit tests + integration tests where relevant.

---

## 3. Branching Model

- `main` → stable  
- `dev` → active development  
- feature branches → `feature/<feature-name>`

Pull Request Rules:
- include description
- reference Issue number
- pass Rust CI (fmt + clippy + tests)

---

## 4. Rust Toolchain

Required:
- stable Rust
- rustfmt
- clippy

Optional:
- cargo-audit
- cargo-watch
- rust-analyzer

---

## 5. Licensing

By contributing, you agree your code is released under:
**Apache License 2.0**

---

### 6. Bounty Policy

No bounties are paid on Phase N+1
until Phase N is merged and locked.

## 7. Security

For vulnerabilities, do NOT open a public issue.  
Use: security@mbongo.money (placeholder)


