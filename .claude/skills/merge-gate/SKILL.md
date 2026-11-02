---
name: merge-gate
description: Verify a pull request before and after merging it — source SHA is unchanged, CI belongs to that exact revision, scope is what was reviewed, and the merge landed what was approved. Use before merging any PR in this repository, and again after the merge to confirm the push CI.
---

# Merge gate

A procedure, not an enforcement mechanism. It tells you what to check and
when to stop; nothing here prevents a command from running.

Evidence rules come from [`docs/ENGINEERING_EVIDENCE.md`](../../../docs/ENGINEERING_EVIDENCE.md).
This skill is how to apply them to a merge. It does not restate them.

## When to use

Before merging any pull request, and again immediately after.

## Preconditions

- You know the PR number and the SHA that was actually reviewed.
- You know which issues the PR references, and whether any should close.

## Procedure

### 1. Lock the source

Get the PR's head SHA and compare it with the revision you reviewed.

```bash
gh pr view <N> --json state,headRefOid,mergeable,mergeStateStatus,baseRefName
```

**If the head moved, every earlier result describes code you are no longer
merging.** Re-review and re-run CI at the new SHA. Do not carry forward a
green check from the previous revision.

### 2. Bind CI to that SHA

```bash
gh pr checks <N> --json name,state
```

Count the states separately:

```
SUCCESS = …   SKIPPED = …   FAILURE = …
```

**A skipped job is skipped.** Say so. If a gate needs evidence that a skipped
job would have produced, get it from a run that executed the job. Note why it
skipped — in this repository the Devnet Convergence Harness carries
`if: github.event_name == 'push'`, so it is expected to skip on a PR and to
run on the push after merge.

### 3. Check the scope you expect

State the expected file list **before** looking, then compare.

```bash
git diff <baseline>..<source> --name-only
git diff <baseline>..<source> --numstat
git diff <baseline>..<source> --numstat --ignore-all-space
```

If the two numstat outputs disagree, the difference is whitespace — usually
re-indentation, sometimes a line-ending rewrite. Find out which before
proceeding; see
[`git-canonical-file-verification`](../git-canonical-file-verification/SKILL.md).

### 4. Prove what did not change

List the files you expect to be unchanged, **with a count**, then compare
canonical blobs — not files on disk.

```bash
git rev-parse <baseline>:<path>
git rev-parse <source>:<path>
```

A file that does not exist at either revision is **absent**, which is a
different finding from **unchanged**. Report it as absent.

### 5. Check the issue references

```bash
gh pr view <N> --json body -q .body | grep -icE '^(closes?|fixes?|resolves?) #'
```

`Closes #N`, `Fixes #N` and `Resolves #N` in a merged PR body close that
issue. For one slice of a larger tracked effort that is almost never what you
want — use `Refs #N`. Check this **before** merging; afterwards the issue is
already closed.

### 6. Merge

```bash
gh pr merge <N> --squash --subject "<subject>" --body-file <file>
gh pr view <N> --json state,mergedAt,mergeCommit,headRefOid
```

Record `mergedAt`, the merge SHA, and the strategy. A squash produces one
parent:

```bash
git log -1 --format='%P' <merge-sha> | wc -w   # expect 1
```

### 7. Confirm dev

```bash
git checkout dev && git fetch origin dev && git reset --hard origin/dev
```

Require `HEAD == origin/dev == merge SHA`, a clean worktree and `0 0`
divergence. Then confirm the merged blobs equal the source blobs — a squash
should change nothing about content.

### 8. Wait for the push CI

The PR run and the push run are different evidence. Find the run whose head
is the **merge SHA**:

```bash
gh run list --commit <merge-sha> --json name,event,conclusion,databaseId
gh run view <id> --json conclusion,headSha,jobs
```

Report successes and skips separately. Do not substitute the PR run.

### 9. Record it

Comment on the tracked issue with the merge SHA, the push CI run, and what
was verified. State what was **not** proven as plainly as what was.

## A test is not proven by its exit code

This is the rule that costs the most when ignored.

A negative test that exits non-zero has proven nothing until you know **why**
it exited. A syntax error, a missing file, a wrong `argv` layout, an
unparsed argument — each exits non-zero and each can be mistaken for the
check working.

Distinguish two outcomes, always:

| | |
|---|---|
| **HARNESS_FAILURE** | the test itself broke. The result is **invalid**, not a pass. |
| **SYSTEM_UNDER_TEST_REJECTED** | the code under test reached its intended rejection and reported it. |

So require each case to show **which branch it reached**, not merely its
status:

```
sur  version absente     code=1  branche=PROVENANCE_ATTESTATION_MISSING
sur  digest different    code=1  branche=PROVENANCE_SUBJECT_MISMATCH
```

and screen for harness breakage explicitly:

```bash
echo "$out" | grep -qE 'ENOENT|SyntaxError|Cannot find module|is not defined' \
  && echo "INVALID: harness failed, not a result"
```

The same applies to positive cases: require the success marker the code emits
after its assertions, not merely exit 0.

## Fail closed

Stop and resolve, rather than judging the discrepancy harmless:

- the head SHA is not the one you reviewed
- CI is green for a different SHA
- a job was skipped and a gate needs its evidence
- the changed-file set is not what you expected
- a count does not match what you stated before extracting
- a closing keyword would close an issue that should stay open
- a negative test's rejection branch is unknown

## Forbidden

- Merging on CI attached to a superseded SHA.
- Reporting a skipped job as successful.
- Using worktree comparison for a claim about committed identity.
- Fixing unrelated findings inside the PR under review — report them.

## Expected output

Baseline, source SHA and drift; merge SHA, strategy, parent count; the file
list and both numstat readings; expected and actual counts for every
population compared; PR CI and push CI reported separately; and what remains
unproven.
