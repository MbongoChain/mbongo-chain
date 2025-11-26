# Mbongo Chain — Developer Workflow Guide

Complete guide to the development workflow for all contributors.

---

## 1. Purpose of This Workflow Guide

### Why Workflow Consistency Matters

A consistent development workflow ensures:

| Benefit | Description |
|---------|-------------|
| **Quality** | Standardized checks catch bugs before merge |
| **Collaboration** | Everyone follows the same process |
| **Traceability** | Clean history makes debugging easier |
| **Velocity** | Fewer surprises, faster reviews |
| **Onboarding** | New contributors ramp up quickly |

### Core Principles

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     WORKFLOW PRINCIPLES                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. BRANCH ALWAYS                                                           │
│     Never commit directly to main                                          │
│                                                                             │
│  2. CHECK ALWAYS                                                            │
│     Run fmt, clippy, build before every PR                                 │
│                                                                             │
│  3. REVIEW ALWAYS                                                           │
│     All code requires peer review                                          │
│                                                                             │
│  4. DOCUMENT ALWAYS                                                         │
│     Changes require clear commit messages                                  │
│                                                                             │
│  5. SCOPE ALWAYS                                                            │
│     Keep changes focused and minimal                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Branching Model (Git)

### Branch Types

| Branch | Purpose | Protected | Merge Target |
|--------|---------|-----------|--------------|
| `main` | Production-ready, stable | ✓ Yes | — |
| `feature/*` | New feature development | No | main |
| `fix/*` | Bug fixes | No | main |
| `docs/*` | Documentation updates | No | main |
| `refactor/*` | Internal improvements | No | main |
| `release/*` | Version preparation (future) | No | main |

### Branch Naming Convention

```
feature/mempool-priority-queue
fix/block-header-validation
docs/consensus-overview
refactor/runtime-state-machine
release/v0.1.0
```

**Rules:**
- Lowercase only
- Hyphen-separated words
- Descriptive but concise
- Include module name when relevant

### Branch Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     BRANCH FLOW                                             │
└─────────────────────────────────────────────────────────────────────────────┘

        ┌──────────────────────────────────────────────────────────────┐
        │                         main                                 │
        │  (protected, stable, production-ready)                       │
        └──────────────────────────────────────────────────────────────┘
               │                                           ▲
               │ branch                                    │ merge
               ▼                                           │
        ┌──────────────────────────────────────────────────────────────┐
        │                    feature/my-feature                        │
        │                                                              │
        │  commit → commit → commit → push                            │
        │                                                              │
        └──────────────────────────────────────────────────────────────┘
                              │
                              │ push
                              ▼
        ┌──────────────────────────────────────────────────────────────┐
        │                     Pull Request                             │
        │                                                              │
        │  CI checks → review → approval → merge                      │
        │                                                              │
        └──────────────────────────────────────────────────────────────┘


  DETAILED FLOW:

  main ─────┬───────────────────────────────────────────┬────────────▶
            │                                           │
            │ checkout -b                               │ merge PR
            ▼                                           │
  feature ──●────●────●────●──────────────────────────▶│
            │    │    │    │                            │
         commit commit commit push                      │
                           │                            │
                           └──▶ PR ──▶ CI ──▶ Review ──┘
```

---

## 3. Creating a New Feature Branch

### Standard Workflow

```powershell
# 1. Ensure you're on main and up-to-date
git checkout main
git pull origin main

# 2. Create and switch to feature branch
git checkout -b feature/my-feature-name

# 3. Make your changes...

# 4. Push branch to remote
git push -u origin feature/my-feature-name
```

### Example: Adding Mempool Priority

```powershell
git checkout main
git pull origin main
git checkout -b feature/mempool-priority-queue
# ... implement feature ...
git add .
git commit -m "feat(mempool): add priority queue for transaction ordering"
git push -u origin feature/mempool-priority-queue
```

### Branch Naming Rules

| Rule | Good Example | Bad Example |
|------|--------------|-------------|
| Lowercase only | `feature/add-sync` | `feature/Add-Sync` |
| Hyphen-separated | `fix/block-validation` | `fix/block_validation` |
| Descriptive | `feature/mempool-eviction` | `feature/update` |
| Focused scope | `fix/header-timestamp` | `fix/all-the-bugs` |
| Include module | `refactor/runtime-gas` | `refactor/improvements` |

---

## 4. Conventional Commits

### Commit Type Reference

| Type | Description | Scope Examples |
|------|-------------|----------------|
| `feat:` | New feature or capability | Add new functionality |
| `fix:` | Bug fix | Correct incorrect behavior |
| `docs:` | Documentation only | README, guides, comments |
| `refactor:` | Code restructuring | No behavior change |
| `style:` | Formatting, whitespace | No logic change |
| `test:` | Test additions/changes | Unit, integration tests |
| `perf:` | Performance improvement | Optimization |
| `chore:` | Maintenance tasks | Dependencies, tooling |

### Format

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Mbongo Chain Examples

```powershell
# Runtime module
git commit -m "feat(runtime): implement state transition validation"
git commit -m "fix(runtime): resolve gas calculation overflow"
git commit -m "refactor(runtime): simplify execution context creation"

# PoW module
git commit -m "feat(pow): add compute task verification"
git commit -m "fix(pow): correct proof receipt signature check"
git commit -m "perf(pow): optimize hash computation for GPU"

# Networking module
git commit -m "feat(network): implement peer discovery protocol"
git commit -m "fix(network): handle connection timeout gracefully"
git commit -m "refactor(network): extract gossip logic to separate module"

# CLI module
git commit -m "feat(cli): add 'status' command for node info"
git commit -m "fix(cli): correct config file path resolution"
git commit -m "docs(cli): add help text for all commands"

# Node module
git commit -m "feat(node): implement block synchronization"
git commit -m "fix(node): resolve mempool race condition"

# Crypto module
git commit -m "feat(crypto): add BLS signature support"
git commit -m "test(crypto): add hash function test vectors"

# Documentation
git commit -m "docs: update architecture overview"
git commit -m "docs(readme): add installation instructions"

# Maintenance
git commit -m "chore: update dependencies"
git commit -m "chore(ci): add clippy check to workflow"
```

### Commit Message Guidelines

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     COMMIT MESSAGE RULES                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ✓ Use imperative mood: "add feature" not "added feature"                  │
│  ✓ Keep subject line under 72 characters                                   │
│  ✓ Capitalize first letter of description                                  │
│  ✓ No period at end of subject line                                        │
│  ✓ Separate subject from body with blank line                              │
│  ✓ Use body to explain what and why, not how                               │
│                                                                             │
│  ✗ Don't use vague messages: "fix bug", "update code"                      │
│  ✗ Don't mix multiple changes in one commit                                │
│  ✗ Don't commit generated files                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Required Pre-Commit Checks

### Check Commands

Run these commands before every commit/push:

```powershell
# 1. Check code formatting
cargo fmt --all -- --check

# 2. Run linter
cargo clippy --workspace --all-targets -- -D warnings

# 3. Build workspace
cargo build --workspace

# 4. Run tests (placeholder - expand as tests are added)
cargo test --workspace
```

### Why Each Check Matters

| Check | Purpose | What It Catches |
|-------|---------|-----------------|
| `cargo fmt` | Consistent formatting | Style inconsistencies, readability issues |
| `cargo clippy` | Lint for bugs/style | Common mistakes, unidiomatic patterns, potential bugs |
| `cargo build` | Compilation check | Syntax errors, type mismatches, missing dependencies |
| `cargo test` | Behavior verification | Regressions, logic errors, edge cases |

### Quick Pre-Commit Script

Save as `scripts/pre-commit.ps1`:

```powershell
Write-Host "Running pre-commit checks..." -ForegroundColor Cyan

# Format check
Write-Host "`n[1/4] Checking formatting..." -ForegroundColor Yellow
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "FAIL: Run 'cargo fmt --all' to fix" -ForegroundColor Red
    exit 1
}

# Clippy
Write-Host "`n[2/4] Running Clippy..." -ForegroundColor Yellow
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "FAIL: Fix Clippy warnings" -ForegroundColor Red
    exit 1
}

# Build
Write-Host "`n[3/4] Building workspace..." -ForegroundColor Yellow
cargo build --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Host "FAIL: Fix build errors" -ForegroundColor Red
    exit 1
}

# Tests
Write-Host "`n[4/4] Running tests..." -ForegroundColor Yellow
cargo test --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Host "FAIL: Fix failing tests" -ForegroundColor Red
    exit 1
}

Write-Host "`n✅ All checks passed!" -ForegroundColor Green
```

---

## 6. Pull Request Workflow

### PR Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     PULL REQUEST LIFECYCLE                                  │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 1. PUSH BRANCH                                                           │
  │    git push -u origin feature/my-feature                                │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 2. CREATE PR                                                             │
  │    Open GitHub → New Pull Request → Select branch → Fill template       │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 3. CI CHECKS (Automatic)                                                 │
  │    ├── rustfmt check                                                    │
  │    ├── clippy check                                                     │
  │    └── cargo build                                                      │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 4. CODE REVIEW                                                           │
  │    Reviewer examines code, leaves comments, requests changes            │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 5. ADDRESS FEEDBACK                                                      │
  │    Make changes, push updates, respond to comments                      │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 6. APPROVAL                                                              │
  │    Reviewer approves PR                                                 │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 7. MERGE                                                                 │
  │    Squash and merge (preferred) or merge commit                         │
  └──────────────────────────────────────────────────────────────────────────┘
```

### Pre-PR Checklist

Before opening a PR, verify:

- [ ] All pre-commit checks pass (`fmt`, `clippy`, `build`)
- [ ] Code compiles without warnings
- [ ] Tests pass (if applicable)
- [ ] Commit messages follow conventional format
- [ ] Branch is up-to-date with main
- [ ] Changes are focused and minimal
- [ ] Public functions are documented
- [ ] No debug code or TODO comments left behind

### Required CI Checks

| Check | Command | Required |
|-------|---------|----------|
| Format | `cargo fmt --all -- --check` | ✓ Yes |
| Lint | `cargo clippy --workspace -- -D warnings` | ✓ Yes |
| Build | `cargo build --workspace` | ✓ Yes |
| Test | `cargo test --workspace` | ✓ Yes (when available) |

### Code Review Guidelines

**For Authors:**
- Keep PRs small (< 400 lines preferred)
- Write clear PR descriptions
- Respond to feedback promptly
- Don't take feedback personally

**For Reviewers:**
- Review within 24-48 hours
- Be constructive and specific
- Approve when satisfied
- Focus on correctness, not style (use automated tools)

### PR Size Expectations

| Size | Lines Changed | Review Time | Recommendation |
|------|---------------|-------------|----------------|
| XS | < 50 | < 1 hour | Ideal for quick fixes |
| S | 50-200 | 1-2 hours | Good for features |
| M | 200-400 | 2-4 hours | Acceptable |
| L | 400-800 | 4-8 hours | Consider splitting |
| XL | > 800 | 1+ days | Must split |

---

## 7. Keeping Branch Updated

### Sync Commands

```powershell
# Fetch latest changes from remote
git fetch origin

# Option 1: Merge main into your branch
git merge origin/main

# Option 2: Rebase your branch on main
git rebase origin/main
```

### Merge vs Rebase

| Approach | When to Use | Pros | Cons |
|----------|-------------|------|------|
| **Merge** | Shared branches, complex history | Preserves history, safe | Messy history |
| **Rebase** | Personal branches, clean history | Linear history, clean | Rewrites commits |

### Recommended Workflow

```powershell
# For personal feature branches (preferred)
git fetch origin
git rebase origin/main

# If conflicts occur
# Resolve conflicts in each file
git add <resolved-files>
git rebase --continue

# Force push after rebase (only for personal branches!)
git push --force-with-lease
```

### When NOT to Rebase

- Shared branches (multiple contributors)
- After PR is open (unless you're the only contributor)
- When unsure about conflict resolution

---

## 8. Resolving Merge Conflicts

### Conflict Resolution Workflow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     CONFLICT RESOLUTION                                     │
└─────────────────────────────────────────────────────────────────────────────┘

  1. IDENTIFY CONFLICTS
     git status
     # Shows files with conflicts

  2. OPEN CONFLICTING FILE
     # Look for conflict markers:
     <<<<<<< HEAD
     your changes
     =======
     their changes
     >>>>>>> origin/main

  3. RESOLVE CONFLICTS
     # Choose correct code, remove markers

  4. STAGE RESOLVED FILES
     git add <resolved-file>

  5. CONTINUE MERGE/REBASE
     git merge --continue
     # or
     git rebase --continue

  6. PUSH CHANGES
     git push
     # or (after rebase)
     git push --force-with-lease
```

### Using Cursor (Recommended)

1. Open the conflicting file in Cursor
2. Cursor highlights conflict sections
3. Use inline buttons: "Accept Current", "Accept Incoming", "Accept Both"
4. Or manually edit the file
5. Save and stage: `git add <file>`

### Using VS Code

1. Open conflicting file
2. VS Code shows conflict highlighting
3. Click "Accept Current Change", "Accept Incoming Change", or "Accept Both Changes"
4. Save and stage

### Manual CLI Resolution

```powershell
# 1. See conflicts
git status

# 2. Open file and find markers
# <<<<<<< HEAD
# your code
# =======
# their code
# >>>>>>> origin/main

# 3. Edit to keep correct code, remove markers

# 4. Stage resolved file
git add runtime/src/lib.rs

# 5. Continue
git rebase --continue
# or
git merge --continue
```

### Example Full Resolution

```powershell
# Start rebase
git fetch origin
git rebase origin/main

# Output:
# CONFLICT (content): Merge conflict in runtime/src/lib.rs
# error: could not apply abc1234... feat: add validation

# Check status
git status
# Shows: both modified: runtime/src/lib.rs

# Open in Cursor/VS Code, resolve conflicts
cursor runtime/src/lib.rs

# After resolving
git add runtime/src/lib.rs
git rebase --continue

# Push updated branch
git push --force-with-lease
```

---

## 9. Directory Ownership

### Module Review Assignments

| Directory | Team | Reviewers |
|-----------|------|-----------|
| `/runtime` | Runtime Team | State machine, execution engine experts |
| `/node` | Node Team | Node lifecycle, orchestration experts |
| `/pow` | Consensus Team | PoUW verification, compute proof experts |
| `/network` | Networking Team | P2P, gossip protocol experts |
| `/crypto` | Crypto Team | Cryptography, signature experts |
| `/cli` | CLI Team | User experience, command interface experts |
| `/docs` | Docs Team | Technical writers, documentation experts |
| `/spec` | Architecture Team | Protocol designers |

### Review Matrix

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     MODULE OWNERSHIP MATRIX                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PR touches...          Required reviewers...                              │
│  ──────────────────────────────────────────────────                        │
│                                                                             │
│  runtime/               → Runtime Team + 1 Crypto                          │
│  node/                  → Node Team + 1 Runtime                            │
│  pow/                   → Consensus Team + 1 Crypto                        │
│  network/               → Networking Team                                  │
│  crypto/                → Crypto Team (2 reviewers)                        │
│  cli/                   → CLI Team                                         │
│  docs/                  → Docs Team                                        │
│  spec/                  → Architecture Team (2 reviewers)                  │
│                                                                             │
│  Cross-module changes   → Lead Architect + relevant teams                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Review Expectations

| Module Type | Min Reviewers | Review Focus |
|-------------|---------------|--------------|
| Crypto | 2 | Security, correctness |
| Consensus | 2 | Safety, liveness |
| Runtime | 1 | Determinism, gas |
| Network | 1 | Performance, resilience |
| CLI | 1 | UX, error handling |
| Docs | 1 | Accuracy, clarity |

---

## 10. Workspace Build & Test Flow

### Standard Development Sequence

```powershell
# 1. (Optional) Clean previous build artifacts
cargo clean

# 2. Format all code
cargo fmt --all

# 3. Run linter
cargo clippy --workspace --all-targets -- -D warnings

# 4. Build all modules
cargo build --workspace

# 5. Run tests
cargo test --workspace
```

### Detailed Command Reference

```powershell
# CLEAN (when needed)
cargo clean                     # Remove all build artifacts
cargo clean -p runtime          # Clean specific package

# FORMAT
cargo fmt --all                 # Format all code
cargo fmt --all -- --check      # Check without modifying
cargo fmt -p runtime            # Format specific package

# LINT
cargo clippy --workspace --all-targets -- -D warnings   # Full lint
cargo clippy -p runtime                                  # Lint specific package
cargo clippy --fix                                       # Auto-fix some issues

# BUILD
cargo build --workspace         # Debug build
cargo build --workspace --release  # Release build
cargo build -p node             # Build specific package

# TEST
cargo test --workspace          # Run all tests
cargo test -p crypto            # Test specific package
cargo test -- --nocapture       # Show test output
cargo test test_name            # Run specific test
```

### Build Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     BUILD & TEST FLOW                                       │
└─────────────────────────────────────────────────────────────────────────────┘

   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
   │  clean  │───▶│   fmt   │───▶│ clippy  │───▶│  build  │───▶│  test   │
   │(optional)│    │         │    │         │    │         │    │         │
   └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘
        │              │              │              │              │
        ▼              ▼              ▼              ▼              ▼
   Remove old    Apply code     Check for     Compile all    Verify
   artifacts     formatting     warnings      modules        behavior
```

---

## 11. Recommended Cursor Workflow

### Best Practices for AI-Assisted Development

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     CURSOR WORKFLOW GUIDELINES                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ✓ DO                                                                       │
│  ────                                                                       │
│  • Use "Fix this file only" prompts for targeted changes                   │
│  • Specify module scope: "In runtime/src/lib.rs, fix..."                   │
│  • Ask inline questions for clarification                                  │
│  • Review all AI-generated changes before committing                       │
│  • Use module-specific agents when available                               │
│                                                                             │
│  ✗ DON'T                                                                    │
│  ──────                                                                     │
│  • Request global refactors across entire workspace                        │
│  • Accept changes without reviewing                                        │
│  • Let AI modify files outside your scope                                  │
│  • Use AI for security-critical code without review                        │
│  • Run workspace-wide commands without understanding them                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module-Specific Agents

Use specialized prompts for each module:

```
# Runtime Agent
"You are the Runtime Developer Agent. Focus exclusively on runtime/ module.
 Implement runtime state machine. Never modify other modules."

# Node Agent
"You are the Node Developer Agent. Focus exclusively on node/ module.
 Implement node orchestration. Depends on runtime, network, crypto."

# Crypto Agent
"You are the Crypto Developer Agent. Focus exclusively on crypto/ module.
 Maintain security standards. No workspace dependencies."
```

### Safe Cursor Prompts

| Task | Safe Prompt |
|------|-------------|
| Fix bug | "In `runtime/src/lib.rs`, fix the gas calculation bug on line 142" |
| Add feature | "In `cli/src/main.rs` only, add a 'status' command" |
| Refactor | "Refactor `validate_block()` in `node/src/lib.rs` for clarity" |
| Document | "Add documentation to public functions in `crypto/src/lib.rs`" |

### Avoid These Prompts

| Bad Prompt | Why It's Bad |
|------------|--------------|
| "Refactor the entire codebase" | Too broad, uncontrolled changes |
| "Fix all the bugs" | Vague, potentially destructive |
| "Update everything to latest style" | Mass changes, hard to review |
| "Make it work" | No clear scope or goal |

---

## 12. CI Integration Overview

### GitHub Actions Workflow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     CI PIPELINE                                             │
└─────────────────────────────────────────────────────────────────────────────┘

   PR Created/Updated
          │
          ▼
   ┌─────────────────────────────────────────────────────────────────────────┐
   │                         CI CHECKS                                       │
   │                                                                         │
   │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐               │
   │  │ rustfmt  │  │ clippy   │  │  build   │  │  test    │               │
   │  │  check   │  │  check   │  │  check   │  │  check   │               │
   │  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘               │
   │       │             │             │             │                      │
   │       ▼             ▼             ▼             ▼                      │
   │    ✓ Pass        ✓ Pass       ✓ Pass       ✓ Pass                     │
   │                                                                         │
   └─────────────────────────────────────────────────────────────────────────┘
          │
          ▼
   All Checks Pass → Ready for Review
```

### Current CI Checks

| Check | Status | Command |
|-------|--------|---------|
| Rustfmt | ✓ Active | `cargo fmt --all -- --check` |
| Clippy | ✓ Active | `cargo clippy --workspace -- -D warnings` |
| Build | ✓ Active | `cargo build --workspace` |
| Test | ✓ Active | `cargo test --workspace` |

### Future CI Checks (Planned)

| Check | Status | Purpose |
|-------|--------|---------|
| Docs Linting | Planned | Validate documentation |
| Security Scan | Planned | Detect vulnerabilities |
| Coverage | Planned | Track test coverage |
| Benchmarks | Planned | Performance regression |

### CI Configuration

Located in `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      
      - name: Format Check
        run: cargo fmt --all -- --check
      
      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      
      - name: Build
        run: cargo build --workspace
      
      - name: Test
        run: cargo test --workspace
```

---

## 13. Developer Rules Summary

### The 10 Professional Expectations

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     DEVELOPER RULES                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. NO DIRECT COMMITS TO MAIN                                               │
│     Always use feature branches and PRs                                    │
│                                                                             │
│  2. ALWAYS BRANCH                                                           │
│     Create feature/fix/docs branches for all work                          │
│                                                                             │
│  3. ALWAYS RUN FMT/CLIPPY/BUILD                                             │
│     Before every commit and push                                           │
│                                                                             │
│  4. ALWAYS WRITE CLEAN COMMITS                                              │
│     Follow conventional commit format                                      │
│                                                                             │
│  5. ALWAYS UPDATE PRS CLEANLY                                               │
│     Address feedback, don't abandon PRs                                    │
│                                                                             │
│  6. ALWAYS DOCUMENT CHANGES                                                 │
│     Public APIs require documentation                                      │
│                                                                             │
│  7. ALWAYS REPLY TO REVIEWS                                                 │
│     Respond within 48 hours                                                │
│                                                                             │
│  8. ALWAYS RESOLVE CONFLICTS CAREFULLY                                      │
│     Don't blindly accept changes                                           │
│                                                                             │
│  9. ALWAYS SYNC WITH MAIN BEFORE MERGE                                      │
│     Keep branch up-to-date                                                 │
│                                                                             │
│  10. ALWAYS KEEP SCOPE TIGHT                                                │
│      One feature per PR, minimal changes                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Quick Checklist

Before every commit:
- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo build --workspace`
- [ ] Clean commit message

Before every PR:
- [ ] Branch is synced with main
- [ ] All CI checks pass locally
- [ ] Changes are focused and minimal
- [ ] PR description is clear
- [ ] Checklist is complete

Before every merge:
- [ ] Approval received
- [ ] CI passes
- [ ] Conflicts resolved
- [ ] Branch is up-to-date

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

