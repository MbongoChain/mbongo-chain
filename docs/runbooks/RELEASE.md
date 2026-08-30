# Release Runbook — TypeScript SDK

**Status: NORMATIVE for how `@mbongo/sdk` is released.** This is the contract
the release pipeline must satisfy. No pipeline exists yet; #101 slice D will
implement one, and may only implement what §11 permits.

Scope is the TypeScript SDK under `sdk/typescript/`. Node releases and
protocol versioning are out of scope.

Evidence rules referenced throughout come from
[`../ENGINEERING_EVIDENCE.md`](../ENGINEERING_EVIDENCE.md), which is not
restated here.

Each statement below is marked:
**[repo]** a fact about this repository ·
**[decision]** a project decision ·
**[external]** a prerequisite outside the repository ·
**[platform]** current npm or GitHub behaviour, which can change — always
re-check the linked documentation before relying on it.

---

## 1. Current state

**[repo]** As of this document:

| | |
|---|---|
| Package | `@mbongo/sdk`, version `0.1.0`, Apache-2.0, ESM only |
| Registry | none declared; the public npm registry by default |
| `publishConfig` | `{"access": "public"}` |
| Git tags | `v0.2-devnet-stable`, `v0.3-devnet-stable` — devnet milestones, no SDK tags |
| GitHub releases | none |
| GitHub environments | none |
| Repository secrets | none |
| Release automation | none |

**[external]** The package is **not published**. `@mbongo/sdk` and
`/-/org/mbongo` both return 404 from the registry, and the npmjs.com org page
returns 403.

Those are **observations, not proof**. A 404 shows a name is unregistered. It
does not show that the scope is available, claimable, or controlled by anyone
in particular. Scope control is `NOT VERIFIED` and can only become `PROVEN` by
positive authenticated evidence from npm.

---

## 2. What makes a release authoritative

A release is authoritative when all nine of these are answerable from durable
evidence:

| Question | Answer comes from |
|---|---|
| What version? | `version` in `sdk/typescript/package.json` |
| What commit? | the commit the release tag points at |
| What tag? | `sdk-typescript-v<semver>` |
| What artifact was tested? | the `.tgz` produced by the release run |
| What artifact was published? | **the same `.tgz`** — see §6 |
| What proves publication? | the registry returning that version |
| What proves provenance? | `npm audit signatures`, and the package page |
| Who may initiate it? | a maintainer, through the approval gate in §8 |
| What must already exist? | the bootstrap prerequisites in §9 |

If any is unanswerable, the release is not authoritative and must not proceed.

---

## 3. Version authority

**[decision]** `version` in `sdk/typescript/package.json` is the single source
of truth. Nothing else defines the released version.

**[repo]** `sdk/typescript/package-lock.json` is lockfileVersion 3 and records
the package version at its root. It must equal `package.json`; `npm ci` fails
when the two disagree, so this is enforced before any gate runs.

**[decision]** SDK SemVer is independent of protocol versioning. The SDK is
`0.1.0` while the protocol is in the v0.3 family, and they are not expected to
converge. Nothing may infer a protocol version from an SDK version, or the
reverse.

---

## 4. Tag policy

**[decision]** Release tags are `sdk-typescript-v<semver>`, for example
`sdk-typescript-v0.1.0`.

**[repo]** No tag matching `sdk-typescript-v*` exists, so the namespace is
free. The two existing tags use a `v<version>-devnet-stable` form for devnet
milestones and are not affected.

The prefix matters because this is a monorepo: an unprefixed `v0.1.0` would be
ambiguous against node and protocol milestones.

**Required equality.** The semver in the tag, the `version` in
`package.json` at the tagged commit, and the version published must all be the
same string. **Any mismatch fails the release before packing.** A tag reading
`0.1.1` over a manifest reading `0.1.0` is exactly the case this catches.

---

## 5. Trigger

**[decision]** The primary trigger is **pushing a release tag**.

| Option | Why not chosen |
|---|---|
| GitHub Release publication | couples authoring release notes to publishing; the Release is editable after the fact, so it is a weak binding to a revision |
| `workflow_dispatch` | binds to no version and no commit by itself; a mistyped input becomes a release |
| **Tag push** | **chosen** — a tag names exactly one commit, is visible in the repository, and cannot be produced by a pull request from a fork |

A tag push cannot be triggered by an untrusted contributor: creating a tag in
this repository requires push access. That addresses the "a hostile pull
request triggers a publish" case structurally rather than by configuration.

`workflow_dispatch` may additionally exist for re-running verification steps,
provided it cannot reach the publish step.

---

## 6. Pack once, publish what was tested

**[decision]** The release packs **one** tarball, runs every gate against that
tarball, and publishes **that same file**.

**[platform]** This is possible because `npm publish` accepts a package
specifier that may be "a gzipped tarball (`.tar.gz` or `.tgz`)", not only a
directory — see
[npm-publish](https://docs.npmjs.com/cli/v11/commands/npm-publish).

The alternative — test the directory, then publish the directory — packs
twice. The second pack is a different file that nothing tested, and any change
in the working tree between the two steps ships unverified. The identity claim
"the artifact tested is the artifact published" is only provable when there is
one artifact.

**Evidence of identity:** record the tarball filename and a digest computed
once, and assert that the file publish is invoked on is the same path with the
same digest. Per the cardinality rule, assert exactly one tarball exists in
the pack destination before proceeding.

**[platform]** Whether `npm publish <tarball>` produces provenance
identically to publishing a directory is **not stated** in the documentation
consulted. #101D must establish this with `--dry-run` before relying on it,
and must not assume it. If tarball publishing and provenance turn out to be
incompatible, that is a real trade-off to decide then — not something to
paper over.

---

## 7. Pre-publish gates

Every gate below is mandatory, and each exists to prevent a specific failure:

| Gate | Prevents |
|---|---|
| `npm ci` | a build against dependencies that do not match the lockfile |
| tag/version equality | publishing a version nobody intended |
| `npm run typecheck` | shipping declarations that do not compile |
| `npm test` | shipping broken behaviour |
| `npm run build` | publishing a stale or absent `dist/` |
| `npm run test:consumer` | shipping a package that cannot be installed or imported — it packs, installs into an external consumer, and imports by package name |
| `npm audit` | knowingly shipping a vulnerable dependency tree |
| tarball content assertion | shipping without `LICENSE`, or with `src/`, tests or config |
| `exports`/`types` target check | shipping entry points that resolve to nothing |
| exactly one tarball present | publishing an artifact other than the tested one |

**[repo]** The consumer smoke already performs the pack, the install, the
JavaScript and TypeScript consumer checks, the `LICENSE` assertion and the
tarball content assertion — 28 checks in
`sdk/typescript/scripts/consumer-smoke.mjs`, run in CI on every change. The
release does not reimplement them; it reuses that script against the release
tarball.

`npm audit` failing is a stop, not a warning. A dependency inventory is not a
separate gate because `npm ci` against a committed lockfile already fixes the
tree.

---

## 8. The three boundaries

Keeping these apart is the point of this document. Automation may only act
inside the first.

### A. Repository-controlled

Decided or verified by code in this repository:

version, tag format and equality, build, tests, consumer smoke, packed
contents, the workflow definition, the request for provenance, release
metadata, and post-publish verification against the registry.

### B. GitHub settings, outside Git

**[external]** Configured by a repository administrator, not by a commit:

- the `npm-production` environment and its required reviewers
- any deployment branch or tag policy on that environment
- repository Actions permissions

**[platform]** Environments with required reviewers are available on public
repositories on GitHub Free. This repository is public and the organisation is
on the free plan, so no upgrade is required. Up to six reviewers may be
configured and **only one needs to approve**; the job waits until then. See
[managing environments](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments).

**[decision]** Adopt an `npm-production` environment with at least one
required reviewer. It provides the human approval gate in §10 and an audit
record, both outside anything a workflow change could remove on its own.

### C. npm account actions

**[external]** Requiring an authenticated human on npm:

- an npm account with 2FA
- control of the `@mbongo` scope
- ownership of the `@mbongo/sdk` package name
- configuring the trusted publisher for the package
- the first publication itself — see §9

None of these can be performed by this repository, and none may be attempted
by automation.

---

## 9. Trusted publishing, and why the first release is manual

**[decision]** Trusted publishing via GitHub Actions OIDC is the **preferred**
mechanism, and **blocked by an external prerequisite** for the first release.

**[platform]** From
[npm trusted publishers](https://docs.npmjs.com/trusted-publishers):

- Supported on GitHub Actions with **GitHub-hosted runners** only.
- Configured **at the package level on npmjs.com**, naming the organisation,
  the repository, the **workflow filename**, and optionally an environment.
- **Each package can have only one trusted publisher at a time.**
- Requires **npm CLI 11.5.1 or later and Node 22.14.0 or higher**.
- The workflow needs `id-token: write`.
- **No `NPM_TOKEN` is needed for publishing.**
- Provenance is generated automatically, without `--provenance`, when
  publishing via trusted publishing from a public repository to a public
  package.

**The bootstrap problem.** Trusted publishing is configured *on the package*.
The package does not exist. The npm documentation gives no procedure for
configuring a trusted publisher for a package that has never been published.

**[decision]** Therefore:

1. **The first publication is a manual, human action** — performed by a
   maintainer from their own machine, authenticated with 2FA, publishing the
   tarball produced and verified by a release run.
2. **Immediately afterwards**, the maintainer configures the trusted publisher
   on the now-existing package, naming this repository, the release workflow
   filename, and the `npm-production` environment.
3. **Every subsequent release publishes through the workflow** using OIDC.

This is deliberately not automated away. It means **no long-lived npm
credential ever needs to exist in this repository**, at any point, including
the bootstrap.

**[decision] Token fallback: none by default.** No `NPM_TOKEN` secret is to be
created or stored. If trusted publishing is ever unavailable, the recovery
path is the same manual publication as the bootstrap — a human, on their own
machine, with 2FA — not a token added to the repository. A token in CI is a
standing credential that outlives the incident it was added for.

---

## 10. Human approval boundary

**[decision]** These stay explicit human decisions and are never automated:

- choosing the release version
- creating the release tag
- approving the `npm-production` deployment
- authorising the first publication
- claiming or transferring the `@mbongo` scope
- changing the trusted publisher identity
- any recovery action after an ambiguous failure (§12)

The common thread is that each is either irreversible or reaches outside the
repository.

---

## 11. What #101D may implement

**Allowed:**

- one dedicated release workflow, triggered by a `sdk-typescript-v*` tag push
- least-privilege permissions: `contents: read` and `id-token: write`
- the `npm-production` environment reference
- the tag/version equality check, failing closed
- the pre-publish gates of §7, reusing the existing consumer smoke
- packing exactly one tarball and publishing that file
- trusted publishing via OIDC, on `ubuntu-latest`, once §9 step 2 is complete
- post-publish verification against the registry
- a GitHub Release created **after** a confirmed publish (§13)

**Forbidden:**

- creating an npm account, organisation, or claiming a scope
- creating, storing, or reading any npm token
- falling back to token authentication, silently or otherwise
- publishing the first release without the §9 human step
- granting `contents: write` unless §13 is adopted, and then only for the
  release-creation step
- changing the package public API, version, dependencies, or `tsconfig.json`
- any protocol, runtime or RFC change
- **weakening the tested-artifact identity of §6 to make the pipeline
  convenient.** If publishing the tested tarball turns out to be incompatible
  with provenance, or with anything else discovered during implementation,
  #101D must **stop and report**. It must not silently repack, publish from
  the directory instead, or drop the identity requirement to get a green run.
  That is a contract change, and it belongs to a decision rather than to an
  implementation detail.

**[platform]** `contents: read` plus `id-token: write` is the minimum for
OIDC publishing. Creating a GitHub Release needs `contents: write`, which is
a real privilege increase — see §13.

---

## 12. Failure, retry, and recovery

**[platform]** Two npm rules shape everything here, from the
[unpublish policy](https://docs.npmjs.com/policies/unpublish) and
[npm-publish](https://docs.npmjs.com/cli/v11/commands/npm-publish):

- Publishing a name and version that already exists **fails**.
- A name and version combination, once published, **can never be used again**
  — even after unpublishing.

So a release is not reversible by re-running it.

| Failure | Response |
|---|---|
| any gate fails | stop; nothing was published; fix and re-tag with a new version if the tag was already pushed |
| tag/version mismatch | stop before packing |
| more or fewer than one tarball | stop; the pack step is defective |
| OIDC unavailable | stop; do **not** fall back to a token |
| registry unreachable before the request | stop; retry the whole release is safe, because nothing reached npm |
| **publish outcome unknown** | **do not retry** — go to §12.1 |
| publish fails after the registry accepted | treat as published; verify, then §12.2 |
| GitHub Release step fails after a successful publish | the release is real; create the Release by hand |
| provenance missing | the package is published; do not republish. Record it and investigate |
| version already exists | stop; someone or something already published it. Investigate before doing anything else |

### 12.1 Ambiguous outcomes

A timeout or a dropped connection during publish does not tell you whether the
registry accepted the package. **Never blindly retry.** Query the registry for
that exact version first, and let the answer decide:

- version present → published; do not retry
- version absent → not published; retrying is safe
- registry unreachable → still unknown; wait, do not act

This is the absence-is-not-proof rule applied to the one place where guessing
wrong is unrecoverable.

### 12.2 Rolling back a bad release

**[platform]** Unpublishing is possible within **72 hours** of publishing only
if no package in the public registry depends on it. After 72 hours it also
requires fewer than 300 downloads in the last week and a single maintainer.
Unpublishing every version of a package blocks new versions of that package
for **24 hours**.

**[decision]** Do not plan around unpublish. The recovery for a bad release is
to **publish a corrected patch version and deprecate the bad one**.
`npm deprecate` attaches a warning shown on install and on the package page,
while leaving existing consumers working.

---

## 13. GitHub Release

**[decision]** A GitHub Release is a **result, not a trigger, and optional**.

It is created only **after** npm publication is confirmed, so there is one
authority for whether a release happened: the registry. Creating it first
would leave a public artifact claiming a release that may never occur.

Adopting it costs `contents: write` on the workflow, or a separate manual
step. If the release notes are worth that privilege increase, scope the
permission to a single job that runs after publish. Otherwise create the
Release by hand.

---

## 14. Evidence to retain

Durable, machine-checkable records — not an issue comment:

| Evidence | Where it lives |
|---|---|
| commit SHA | the release tag |
| tag | the repository |
| version | `package.json` at that commit |
| tarball filename and digest | the workflow run log |
| gate results | the workflow run, bound to that exact SHA |
| published version | the registry |
| provenance status | `npm audit signatures`, the package page |
| GitHub Release | its own URL, if created |

Per [`../ENGINEERING_EVIDENCE.md`](../ENGINEERING_EVIDENCE.md): bind every
result to the exact SHA, assert the expected count before comparing, and state
what was proven rather than what was assumed. In particular, distinguish
**provenance requested**, **provenance generated**, and **provenance verified**
— they are three different claims, and only the last one is evidence a
consumer can act on.

---

## 15. First release checklist

Nothing below may be automated. Work top to bottom; a `no` at any line stops
the release.

**[external]** Before any publication:

- [ ] an npm account exists, with 2FA enabled
- [ ] the `@mbongo` scope is controlled by that account or its organisation —
      **positively verified while authenticated**, not inferred from a 404
- [ ] the account may publish `@mbongo/sdk`
- [ ] the `npm-production` environment exists with at least one required
      reviewer
- [ ] npm CLI ≥ 11.5.1 and Node ≥ 22.14.0 on the publishing machine

**[repo]** Then:

- [ ] `sdk/typescript/package.json` holds the intended version
- [ ] a `sdk-typescript-v<version>` tag points at the release commit
- [ ] a release run passed every gate in §7 at that exact SHA
- [ ] exactly one tarball was produced, and its digest is recorded

**[external]** Then, by hand:

- [ ] publish that tarball, authenticated with 2FA
- [ ] confirm the version is present in the registry
- [ ] configure the trusted publisher on the package: this repository, the
      release workflow filename, the `npm-production` environment
- [ ] confirm the next release publishes through the workflow without any
      token

Only after the last line is `@mbongo/sdk` releasable by automation.
