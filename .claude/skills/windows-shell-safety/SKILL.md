---
name: windows-shell-safety
description: Avoid shell patterns that fail or mislead when working on this repository from Windows, where both a POSIX shell and PowerShell are available and some tooling behaves differently from the Linux CI runners. Use when a command produces a surprising result, hangs, or reports zero matches.
---

# Windows shell safety

Development here happens on Windows; CI runs on Linux. Most differences are
harmless. The ones below have produced wrong results in this repository, and
the failure mode is usually a plausible-looking answer rather than an error.

Nothing here is enforced. It is a list of what to reach for and what to
distrust.

## When to use

A command hangs, returns zero matches, reports a whole-file diff after a
small edit, or gives an answer that does not match what you can see.

## Git revision syntax and MSYS

MSYS rewrites arguments that look like paths. `HEAD:docs/INDEX.md` and
`origin/dev:.github/CODEOWNERS` are rewritten into backslashed nonsense, and
the command fails with a confusing message.

```bash
export MSYS2_ARG_CONV_EXCL='*'
git rev-parse HEAD:docs/INDEX.md
```

Set it for the command or the block that needs it. **It is not free:** with
conversion disabled, GNU `tar` given a Windows path such as `C:/tmp/x.tgz`
reads `C:` as a remote host and fails with a resolve error. Under that
setting, pass POSIX paths (`/c/tmp/x.tgz`) to `tar`, or avoid `tar` and use
Node's `zlib` and a small header walk instead.

## Counting carriage returns

```bash
tr -cd '\r' < file | wc -c        # counts real CR bytes
```

`grep -c $'\r'` strips CR before matching under MSYS and reports **0** on a
CRLF file. It will tell you a file is clean when it is not.

## Node path resolution differs from the shell's

Node resolves `/tmp/x` as `C:\tmp\x`, not the MSYS `/tmp`. A file written by
bash into `/tmp` is not where a Node script will look for it.

Use an explicit Windows-style absolute path for anything crossing between
them, or keep the file in a directory both agree on.

## Writing files

Editing tools in this repository have repeatedly converted a tracked file to
CRLF, turning a one-line change into a whole-file rewrite. **After every edit
to a tracked text file, check:**

```bash
git diff --numstat
git diff --numstat --ignore-all-space
```

If they disagree, normalise — see
[`git-canonical-file-verification`](../git-canonical-file-verification/SKILL.md).

For substantial multi-line content, prefer a file-writing tool over a shell
heredoc. Prose containing apostrophes has repeatedly broken heredocs here,
and PowerShell here-strings are worse: `@'…'@` requires the closing marker at
column 0, and an unterminated quote drops the shell into a continuation
prompt that looks like a hang.

## Searching

Prefer targeted search over recursive scans:

```bash
git grep -n '<pattern>'            # tracked files only
git ls-files '<glob>'
```

A broad recursive text search will read `target/`, `node_modules/` and
binaries. It is slow, noisy, and can produce matches inside compiled output
that mean nothing.

**A zero-match result proves nothing until the query is known good.** Check
the pattern against a case you know matches before concluding an absence.

## Paths with spaces

Quote every path. `C:\Program Files\nodejs\…` appears in real command lines
here — for instance when locating the bundled npm CLI. When spawning
processes from Node, pass an argv array with `shell: false` so the path is
never re-parsed:

```js
spawnSync(process.execPath, [cliPath, "pack", "--pack-destination", dir],
  { shell: false });
```

Recent Node refuses to spawn `npm.cmd` without a shell, so run the npm CLI as
a JavaScript file through `process.execPath` rather than reaching for
`shell: true`.

## Distinguish a broken command from a real answer

Before treating any result as evidence, ask whether the command worked.

| Symptom | Likely |
|---|---|
| `ENOENT`, `SyntaxError`, `Cannot find module` | the harness broke |
| a resolve error naming `C:` | MSYS path conversion |
| zero matches from a search | check the pattern first |
| a whole-file diff after a small edit | line-ending rewrite |
| a hang after a quote or heredoc | unterminated continuation |

An expected exit code from a broken command is **not** a passing test. That
distinction belongs to [`merge-gate`](../merge-gate/SKILL.md).

## Fail closed

- A command's result depends on which shell interpreted it and you have not
  established which.
- A search returns zero and the query has not been validated.
- numstat and `--ignore-all-space` disagree.
- A path containing a space was passed unquoted.

## Forbidden

- `grep -c $'\r'` to count carriage returns.
- Assuming `/tmp` means the same thing to bash and to Node.
- Long here-strings or heredocs for generated prose.
- Concluding absence from an unvalidated search.

## Not universal

These are properties of this environment, not engineering principles. CI runs
on `ubuntu-latest`, where none of the MSYS or PowerShell items apply. Do not
carry them into workflow files.

## Expected output

Which shell ran the command, whether MSYS conversion was disabled, the raw
byte counts where line endings are in question, and — when a result was
surprising — what established that the command itself worked.
