# Engineering Evidence Standard

**Status: NORMATIVE.** This document answers one question: *what evidence is
sufficient to make an engineering claim about Mbongo Chain?*

It is tool-neutral. It applies to a human maintainer, a script, or a coding
agent equally, and it does not depend on any particular editor or assistant.

It is **not** a protocol specification, a Git tutorial, a shell guide, a CI
workflow specification, or an incident log. Protocol truth lives in
[`specs/`](specs/) and [`rfcs/`](rfcs/); the process for changing it lives in
[`RFC_PROCESS.md`](RFC_PROCESS.md) and [`CONTRIBUTION_TIERS.md`](CONTRIBUTION_TIERS.md).
Start at [`INDEX.md`](INDEX.md) to find any of them.

---

## 1. The evidence model

Four levels, in increasing strength:

| Level | Meaning |
|---|---|
| **Observation** | Something you saw. A command printed a number; a page rendered; a test appeared to pass. |
| **Evidence** | An observation tied to a specific, named subject — this file, this revision, this artifact. |
| **Canonical evidence** | Evidence drawn from the artifact that *defines* the answer, rather than from a rendering or copy of it. |
| **Conclusion** | A claim, stated in language that matches the evidence you actually have. |

The gap that produces wrong claims is between observation and canonical
evidence. A comparison can succeed on the wrong objects; a check can be green
for a revision you are no longer shipping.

There is no single universal canonical source. It depends on the question:

| Question | Canonical evidence |
|---|---|
| Are two committed versions of a file identical? | The Git objects (blob ids) |
| What is in the tree right now? | A specific commit or tree object |
| Did CI pass for this code? | The run or check attached to that exact SHA |
| Does the runtime behave this way? | Executing it, at that exact source |
| What does the distributed package contain? | The packed artifact itself |
| Does a consumer work against it? | Installing and using that packed artifact |
| Do we control an external account or namespace? | Positive authenticated evidence from that service |

Match the source to the question. Answering "are these files identical?" by
comparing what is on disk answers a different question than the one asked.

---

## 2. Assert expected cardinality before comparing

**This is the first rule, because it fails silently.**

Whenever a check extracts a set of things — fields, rows, links, files,
symbols, matches — and then compares or validates them:

1. State the expected count or expected membership **before** extracting.
2. Extract.
3. Assert the actual count or membership matches.
4. Only then compare contents.

A comparison is not evidence until the population being compared is itself
proven to be the intended population.

Reject all of these, regardless of what the comparison then reports:

- **Empty population.** Zero extracted, zero mismatches, "identical". This is
  the most common vacuous pass.
- **Under-extraction.** Seven fields expected, one extracted, "identical".
- **Over-extraction.** Fourteen data rows expected, fifteen captured because
  the parser took the table header too. An over-count is as much a defect as
  an under-count: it means the extractor does not understand its input.
- **Silent filtering.** Deduplication, `sort -u`, or a `filter` step that
  quietly discards items and hides an extraction bug.

When the count does not match, the check has failed. Fix the extractor and
re-run. Do not reason about whether the discrepancy is benign.

---

## 3. Git identity is a property of objects, not files on disk

Three distinct things carry a version of a file:

- the **working tree** — bytes on disk, produced by checkout
- the **index** — what is staged, including cached stat information
- the **object database** — commits, trees and blobs; the repository's record

For a claim of the form *"this committed file is identical between these two
revisions"*, canonical evidence comes from the object database. Resolve the
path at each revision and compare the blob ids; if they are equal, the content
is equal by construction.

Useful for that: `git rev-parse <commit>:<path>` to get a blob id,
`git cat-file blob <object>` to read one, `git hash-object <file>` to compute
the id a file on disk would have. No single command is mandatory — the
requirement is that the evidence comes from objects.

**Why the working tree cannot answer this.** Checkout can transform content on
the way to disk: line-ending conversion under `core.autocrlf` or `.gitattributes`,
platform representation, and other filters. Two checkouts of the same blob can
differ on disk, and two different blobs can be made to look alike. A stale
index entry can also make `git status` and `git diff` report a file as
unmodified when its bytes no longer match the recorded blob, because the cached
size and timestamp are consulted before the content is.

Comparing two working-tree files with `cmp`, a checksum, or a line count
therefore proves that two renderings agree. It does not prove the repository
stores the same object.

**These are two different questions, and both are legitimate:**

- *"What exactly is on disk right now?"* — inspect the file. Correct for
  build inputs, packaging, anything that consumes the checkout.
- *"What does the repository canonically record?"* — inspect the object.
  Required for any claim about committed identity.

Be explicit about which one you are answering.

---

## 4. Evidence is bound to an exact revision

A CI result proves something about the revision it tested, and nothing else.

Before treating a check as evidence, confirm the SHA it ran against is the SHA
you are about to act on. A pull request number, a branch name, or "the checks
are green" identifies no revision.

If the source changes after review — an amend, a rebase, a conflict
resolution, a fix pushed in response to a finding — earlier results describe
code that is no longer there. Re-anchor: wait for the checks at the new SHA
before merging.

The same rule applies to any evidence you carry forward: a test run, a
benchmark, a manual verification, a screenshot. Record which revision produced
it.

---

## 5. Pull-request CI and post-merge CI are different evidence

They test different things under different conditions:

- **Pull-request CI** proves the tested revision, under the pull-request
  event, with whatever path filters and conditions that event applies.
- **Post-merge (push) CI** proves the resulting commit on the target branch,
  under the push event.

Neither substitutes for the other when a gate calls for both. A job can be
filtered out of one and run in the other; that is normal and often
intentional.

Two consequences:

- **A skipped check is skipped, not successful.** Report it as skipped. A
  summary that says "all checks passed" when a job was filtered out is false,
  even if nothing was broken.
- **Absence of a job is not evidence about the thing it would have tested.**
  If the gate needs that evidence, get it from a run that actually executed
  the job.

This is not a requirement that every pull request run every workflow. It is a
requirement that reports say which jobs ran.

---

## 6. Absence of evidence is not positive proof

Failing to find something is not proof that it does not exist, and it is never
proof of ownership or control.

- A package registry returning 404 for a name shows the name is unregistered.
  It does not show the namespace can be claimed, or that anyone controls it.
  Control requires positive, authenticated evidence from the service.
- Zero search matches proves nothing until you have checked that the query,
  the search path, and the file set are what you intended. A malformed query
  returns zero, and so does a correct query over the wrong directory.
- A file not appearing in a change list proves nothing about semantics unless
  you know the change list covers everything relevant.

When a check comes back empty, the first question is whether the check works —
not what the emptiness implies.

---

## 7. Source, build and artifact are separate surfaces

For anything distributed, correctness at one layer does not carry to the next:

1. **Source** — what is committed.
2. **Build output** — what compilation produced.
3. **Packed artifact** — what packaging actually included.
4. **Installed consumer** — what a project gets when it installs and uses it.

Packaging rules decide layer 3, and they can exclude a file the source
obviously contains or include one it should not. A consumer resolves through
declared entry points, which can be wrong while every file is present.

A gate on a distributable therefore inspects the artifact, and where the claim
is about consumers, installs it and uses it. Procedure lives with the package
that is being gated; the principle is that each layer needs its own evidence.

---

## 8. Cross-implementation vectors

Shared test vectors exist to prove that two independent implementations agree.
Expected values derived by running the implementation under test prove only
that it agrees with itself.

Where a vector is meant to establish cross-language compatibility, derive the
expected values independently — from the specification, or from separate
tooling — rather than by encoding with the implementation the vector is meant
to check.

The methodology for this repository's fixtures is documented with them, in
[`../test-vectors/receipt/README.md`](../test-vectors/receipt/README.md) and
[`../test-vectors/transaction/README.md`](../test-vectors/transaction/README.md).
Those remain authoritative; nothing here restates the cryptography.

---

## 9. Say what you proved

Use language that matches the evidence:

| Word | Means |
|---|---|
| **Proven** | Canonical evidence exists and was checked. |
| **Observed** | Seen, but not from the canonical source for this question. |
| **Inferred** | Reasoned from other facts. Say from what. |
| **Not verified** | Plausible, unchecked. Say so rather than omitting it. |
| **Not applicable** | Out of scope for this change. |

Some words carry a specific claim and should not be used loosely:
*byte-identical*, *supported*, *compatible*, *owned*, *available*, *tested*,
*green*, *unchanged*. Each asserts that a particular check was done.

The distinction is often between what is declared and what is exercised.
"The package declares a minimum runtime of X" and "the package is tested on X"
are different statements, and only one of them is usually true.

---

## 10. Fail-closed rules

When any of these happens, stop and resolve it. Do not proceed on judgement
that the discrepancy is probably fine.

- **Cardinality does not match what you expected** → the check failed. Fix the
  extractor.
- **The source SHA changed** → previous evidence no longer applies. Re-run
  what the gate requires.
- **A claim is about committed file identity** → use Git object evidence.
- **A check was skipped** → report it as skipped, never as success.
- **A claim is about an external account or namespace** → require positive
  evidence, not absence.
- **A validation returns a suspiciously empty population** → treat as failure
  until the population is proven.
- **The work is growing beyond its stated scope** → stop, and either narrow
  back or re-scope explicitly. Do not widen silently.

---

## 11. Scope evidence, proportionally

A change should be able to show it stayed within its stated scope. What that
takes depends on what the change touches:

- a **changed-file inventory** for any change
- **canonical blob comparison** for files claimed unchanged, when the claim
  matters and line-ending or filter effects could confuse a surface check
- a **public API inventory** when the change is near an exported surface
- a **dependency inventory** when packaging or dependencies could move
- **specification blob checks** when the change is near, but not meant to
  touch, a locked surface

Not every check for every change. Evidence should be proportional to the claim
and to the surface at risk — a documentation change does not need a dependency
audit, and a packaging change does.

---

## 12. Repository operations

A small section for conventions specific to how this repository is operated on
GitHub. These are workflow hygiene, not engineering principles.

**Issue references.** GitHub closes an issue automatically when a merged pull
request body contains `Closes #N`, `Fixes #N` or `Resolves #N`. Use those forms
only when closing that issue is the intent. To reference an issue without
closing it — the normal case for one slice of a larger tracked effort — write
`Refs #N`.

This matters most for umbrella issues that track several pieces of work: a
closing keyword in one slice's pull request ends the whole tracking issue.

[`../.github/pull_request_template.md`](../.github/pull_request_template.md)
prints `Refs #` under **Linked Issue** for that reason. Replace it with a
closing keyword only when closing the issue is the intent.

---

## 13. Where these rules come from

Each rule below is traceable to something recorded in this repository or its
issue tracker, not to recollection:

| Rule | Recorded evidence |
|---|---|
| Git object vs working tree (§3) | Two blobs of the same licence text differ by line endings — 11357 and 11558 bytes. A claim of byte-identity made from working-tree comparison was published and later corrected on PR #105 and issue #101. |
| Cardinality, under-extraction (§2) | A field-by-field comparison reported agreement after extracting one field of seven; corrected on issue #101. |
| Cardinality, over-extraction (§2) | A table parser returned fifteen rows where fourteen were expected, having captured the header; corrected on issue #110. |
| Pull-request vs push CI (§5) | On PR #111 the Devnet Convergence Harness was skipped by path filtering; the post-merge push run on the same content executed it and succeeded. |
| Absence is not proof (§6) | The wording that a 404 on a package name does not prove a namespace is obtainable is recorded in the commit message on `dev` for the packaging change. |
| Exact-SHA evidence (§4) | Two pull requests had their head revision replaced after review, #105 and #111; in both cases the earlier CI described code that was no longer being merged. |

The issue-reference convention in §12 is a general safeguard, not a recorded
incident. It exists because the pull request template defaults to a closing
form. No accidental closure is known, but that is weak evidence rather than
proof: GitHub records a keyword closure and a deliberate one with the same
state, so the two cannot be told apart after the fact. Which is itself an
illustration of §6 — the absence of a distinguishable trace is not proof that
nothing happened.
