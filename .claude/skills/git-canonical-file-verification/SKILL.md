---
name: git-canonical-file-verification
description: Prove what a version-controlled file actually is, using Git objects rather than the bytes on disk. Use whenever claiming two committed files are identical, that a file is unchanged between revisions, or when a diff looks larger than the edit that produced it.
---

# Canonical file verification

Files on disk are a *rendering* of what Git stores. On Windows especially,
the two can differ. This procedure answers claims about the repository from
the object database.

Why it matters is in
[`docs/ENGINEERING_EVIDENCE.md`](../../../docs/ENGINEERING_EVIDENCE.md) §3.
This is how to do it.

## When to use

- Claiming two committed files are byte-identical.
- Claiming a file is unchanged between two revisions.
- A diff shows a whole-file rewrite after a one-line edit.
- Copying a file and needing the copy to match canonically.

## Three different things

| | |
|---|---|
| **working tree** | bytes on disk, produced by checkout, possibly transformed |
| **index** | what is staged, including cached size and timestamp |
| **object database** | commits, trees, blobs — what the repository records |

A claim about *the repository* is answered by the third.

## Procedure

### Compare two committed files

```bash
git rev-parse <rev>:<path>          # blob id at a revision
```

Equal blob ids mean equal content, by construction. That is the whole proof —
no hashing of your own required.

```bash
git rev-parse HEAD:LICENSE
git rev-parse HEAD:sdk/typescript/LICENSE
```

### Read a committed file without checkout transformation

```bash
git cat-file blob <blob-id> > out
git cat-file -s <blob-id>            # size in bytes
```

`git show <rev>:<path>` applies the checkout filter. `git cat-file blob`
does not. Use the latter when line endings are part of the question.

### Copy a file canonically

```bash
git cat-file blob $(git rev-parse HEAD:<src>) > <dest>
git hash-object <dest>               # must equal the source blob id
```

`cp` copies the **rendered** file, which may not be what the repository
stores.

### Count raw carriage returns

```bash
tr -cd '\r' < file | wc -c
```

That counts bytes equal to `0x0D`. `grep -c` counts **matching lines**, which
is a different quantity, so `grep -c $'\r'` is not evidence of a CR byte
count — on a LF-only file here it reports the file's line count while the
byte count is 0. See
[`windows-shell-safety`](../windows-shell-safety/SKILL.md) for the
measurement.

### Tell a real change from a line-ending rewrite

```bash
git diff --numstat
git diff --numstat --ignore-all-space
```

A count near the file's total line count in the first, and a small count in
the second, means the content change is small and something rewrote the line
endings. Confirm against the blob:

```bash
git cat-file blob $(git rev-parse HEAD:<path>) | tr -cd '\r' | wc -c   # stored
tr -cd '\r' < <path> | wc -c                                            # on disk
```

Normalise before committing:

```bash
node -e 'const fs=require("fs"),f=process.argv[1];
fs.writeFileSync(f, fs.readFileSync(f,"latin1").replace(/\r\n/g,"\n"), "latin1")' <path>
```

`latin1` avoids re-interpreting bytes. Re-check numstat afterwards: the two
readings should now agree.

## Two questions that look alike

Be explicit about which you are answering:

- **What is on disk right now?** Inspect the file. Correct for builds,
  packaging, anything consuming the checkout.
- **What does the repository record?** Inspect the object. Required for any
  claim about committed identity.

Comparing two working-tree files with `cmp` or a checksum proves two
renderings agree. It does not prove the repository stores the same object —
and this repository has produced exactly that false claim: a licence file
copied with `cp` matched on disk while the stored blobs differed by line
endings, 11357 bytes against 11558.

A stale index entry can also make `git status` and `git diff` report a
modified file as clean, because cached size and timestamp are consulted
before content.

## Cardinality first

Before comparing a set of files, state how many you expect. Then extract,
assert the count, and only then compare. A comparison over an empty or
truncated population reports agreement and means nothing.

## Absent is not unchanged

A path that does not exist at a revision is **absent**. Report it that way.
Counting it among "unchanged" files inflates the number and hides the fact
that nothing was compared.

## Fail closed

- Blob ids differ when you claimed identity.
- The population count does not match what you stated.
- `git rev-parse <rev>:<path>` fails — the path is absent, not unchanged.
- numstat and `--ignore-all-space` disagree and you have not explained why.

## Forbidden

- `cp` for a copy that must match canonically.
- `git show <rev>:<path>` when line endings matter.
- `grep -c $'\r'` for counting carriage returns.
- Presenting worktree equality as canonical identity.

## On Windows

Git revision syntax such as `HEAD:path/to/file` can be mangled by MSYS path
conversion. See
[`windows-shell-safety`](../windows-shell-safety/SKILL.md).

## Expected output

The blob ids compared, their sizes where relevant, the expected and actual
population counts, raw CR counts when line endings are in question, and any
path reported as absent rather than unchanged.
