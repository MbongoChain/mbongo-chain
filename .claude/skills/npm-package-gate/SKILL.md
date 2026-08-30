---
name: npm-package-gate
description: Verify the npm package artifact rather than the source tree — that the tarball carries what it should, installs into an outside project, and is the same artifact that gets published. Use when changing packaging, package metadata, or anything on the release path for the TypeScript SDK.
---

# npm package gate

A green test suite proves the code works with `dist/` sitting in place. It
says nothing about what `npm pack` produced, or whether anyone can install
it. This is how to check the artifact.

Release policy lives in
[`docs/runbooks/RELEASE.md`](../../../docs/runbooks/RELEASE.md) and evidence
rules in [`docs/ENGINEERING_EVIDENCE.md`](../../../docs/ENGINEERING_EVIDENCE.md).
This skill does not restate either.

## When to use

Changing `package.json`, the lockfile, `files`, `exports`, the build output,
or anything the release path touches.

## Five surfaces, five kinds of evidence

Correctness at one layer does not carry to the next:

| Layer | Proven by |
|---|---|
| source | the repository |
| build output | `npm run build` |
| **packed tarball** | inspecting the archive |
| **installed consumer** | installing it and importing by package name |
| **published artifact** | the registry |

Packaging rules decide the third: `files` can exclude something the source
obviously contains. Entry-point resolution decides the fourth: every file can
be present while `exports` points at nothing.

## Procedure

### 1. Clean gates first

```bash
cd sdk/typescript
npm ci
npm run typecheck
npm test
npm run build
```

`npm ci` rather than `npm install`: it honours the lockfile and fails when
the lockfile and manifest disagree.

### 2. The consumer smoke does the artifact work

```bash
npm run test:consumer                        # packs its own tarball
node scripts/consumer-smoke.mjs --tarball <path>   # tests a supplied one
```

It packs into a temporary directory, builds a throwaway project **outside the
repository**, installs the `.tgz` by absolute path, and imports `@mbongo/sdk`
by package name from both JavaScript and TypeScript. It checks tarball
contents, `LICENSE`, installed metadata, the exports map, and that the
package came from a file rather than a registry.

**Do not reimplement those assertions.** Run the script.

### 3. On a release path, exactly one tarball

The artifact tested must be the artifact published, which is only provable
when there is one:

```bash
npm pack --json --pack-destination "$DIR"
find "$DIR" -maxdepth 1 -name '*.tgz' | wc -l    # assert 1
```

Then feed that file to the smoke with `--tarball`. The supplied mode asserts
internally that it packed nothing, so a repack cannot slip in unnoticed.

Never take the first match from an unbounded glob.

### 4. Digests, and what each answers

Both are computed over the same raw tarball bytes and are **never compared
with each other**:

| | Form | Answers |
|---|---|---|
| SHA-256 | hex | did the file survive a transfer intact? |
| SRI | `sha512-<base64>` | is this what the registry holds? |

```bash
node -e 'const c=require("crypto"),f=require("fs");const b=f.readFileSync(process.argv[1]);
console.log("sha256 "+c.createHash("sha256").update(b).digest("hex"));
console.log("sri    sha512-"+c.createHash("sha512").update(b).digest("base64"))' <tgz>
```

### 5. After a publication

```bash
npm view <pkg>@<version> version
npm view <pkg>@<version> dist.integrity     # must equal the SRI, exactly
```

Package existence alone does not prove *our* artifact is what shipped.
`dist.shasum` is a SHA-1 of the same bytes — a diagnostic, never a security
gate.

## Three traps

**`--dry-run` is not consumption.** It reports what would be sent. It does
not install anything, and it cannot tell you a consumer can import the
package.

**Inspecting a tarball is not installing it.** Both are worth doing; only the
second exercises `exports`, entry points and declaration resolution.

**`npm audit signatures` does not prove provenance of this package.** Run
from `sdk/typescript` it audits that project's *installed dependencies* —
`@noble/curves`, `@noble/hashes`, `typescript`. `@mbongo/sdk` is the project,
not one of its dependencies, and is never examined. A green result there is
evidence about the dependency tree.

Provenance of a published version comes from the registry's attestations for
that exact version, and integrity and provenance stay separate questions:
integrity asks *are these the same bytes*, provenance asks *what does the
registry attest about the build*. `RELEASE.md` §6.2 and §6.3 own the detail.

## Fail closed

- More or fewer than one tarball where one is required.
- The smoke reports a non-zero internal pack count in supplied mode.
- `LICENSE`, `README.md`, `package.json` or `dist/` missing from the archive.
- Sources, tests, config or sourcemaps present in it.
- `exports` or `types` resolving to a file that is not there.
- A registry `dist.integrity` that differs from the local SRI — **do not
  retry a publish on that**.
- `npm audit` reporting vulnerabilities on a release path.

## Forbidden

- Claiming installability from `--dry-run` alone.
- Packing twice on a release path.
- Presenting a dependency audit as this package's provenance.
- Turning an unpublished package's registry 404 into evidence about scope
  ownership — a 404 shows a name is unregistered and nothing more.

## Expected output

The gates run and their results; the tarball count asserted against what was
expected; the file list; the consumer smoke's check count and internal pack
count; both digests where a release path is involved; and, after a
publication, the registry comparison stated as an exact equality.
