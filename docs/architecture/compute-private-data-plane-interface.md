# Private data plane interface: the handoff contract

> **Document type:** Architecture — interface contract for off-chain components
> **Status:** Architectural authority for how a client, the control plane, the
> private data plane and a worker hand private inputs and results to one
> another. Defines no consensus rule, no RPC method, no SDK type and no wire
> format; where anything here appears to conflict with a normative source
> below, the normative source wins.
> **Normative sources:** [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md)
> (Accepted), [RFC 0002](../rfcs/0002-receipt-anchoring-v0.3.md),
> [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md),
> [`PROTOCOL_LOCK_v0.3.md`](../specs/PROTOCOL_LOCK_v0.3.md) (FROZEN),
> [`rpc_v0.2.md`](../specs/rpc_v0.2.md) (FROZEN)
> **Parent architecture:** [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md)
> — the four-plane model this contract implements. This document refines
> that architecture's planes 2, 3 and 4 and yields to it on any conflict.

This is Workstream F of the Compute vertical epic
([#126](https://github.com/MbongoChain/mbongo-chain/issues/126)). Its purpose
is that a developer writing the first reference worker can answer every
question in §17 without inventing architecture — and that the answers keep
every byte of private content off the chain, every reusable credential off
the chain, and every vendor out of the protocol.

Nothing here is implemented. No data plane, control plane or worker exists on
`dev`; `crates/mbongo-compute` is an empty placeholder whose module comments
predate the current vision and are not authority.

---

## 1. Fixed points from existing authority

The contract is built on facts that already hold and that this document may
not change:

| Fact | Source |
|---|---|
| A `ComputeTask` names exactly one authorised executor; consensus rejects a receipt from anyone else (rule s). | RFC 0005 §2.5, §3 |
| Consensus checks **equality** between the task's and the receipt's `input_commitment`; how either was derived is not a consensus matter. | RFC 0005 §2.4 |
| `input_commitment` and `output_commitment` are semantically opaque to the chain. | `RECEIPT_SPEC_v0.1` §5 |
| Changing the executor changes `task_id`; a task is immutable once registered. | RFC 0005 §2.6, §8 |
| Step 5 of the end-to-end sequence — how input reaches the executor — is deliberately "off-protocol". This document is that step. | RFC 0005 §10 |
| The chain is not a scheduler, marketplace or object store; a worker never sources input from a chain payload field (P14, P15). | `compute-privacy-data-plane.md` §8, §9, §16 |
| A blinded input commitment is compatible with the accepted protocol. | `compute-privacy-data-plane.md` §12.5 |
| Ordinary execution exposes plaintext to the provider; only attested confidential execution changes that, and it is not required for the first worker. | `compute-privacy-data-plane.md` §7, §10, §22 |

One consequence deserves stating up front because it shapes the whole
authorization model: **under RFC 0005 the worker is chosen before the task
is committed.** The client names `task.executor`, and nothing off-chain can
later assign the committed task to a different worker. "Worker selection" in
the control plane therefore happens *before submission* — the client uses the
control plane to choose whom to name — and "reassignment" is not a transfer:
it is a **new task** with a new `task_id` (RFC 0005 §2.6, "executor
different → allowed — a client may ask two executors the same work"). Every
authorization below is bound to the executor identity the chain has
committed, and inherits that immutability.

---

## 2. Roles

| Role | Holds | Responsible for |
|---|---|---|
| **Client / submitter** | the private input; the task's `submitter` key; the `input_commitment` and, for blinded commitments, the blinding randomness | placing the input in the data plane; committing the task; being the root of authority over its own private objects |
| **Control plane** | provider directory, policy, negotiation state; may hold delegated issuing authority from the client | helping the client select an executor *before* submission; issuing or relaying capabilities *for the executor the task names*; never holding workload content |
| **Private data plane** | private objects, their metadata, their access state | storing, serving and deleting private objects; **enforcing** every capability presented to it; refusing everything else |
| **Worker** | the executor's private key (it *is* the executor identity in protocol terms); temporary plaintext during ordinary execution | presenting a capability with proof of key possession; verifying the fetched input against the task; executing; storing the result; building, signing and anchoring the receipt |
| **Chain** | tasks and receipts | the single authority on *who may answer a task* and on *which commitment a task carries* — nothing else in this document |

The **executor** is a protocol identity: an `Address`, the public half of an
Ed25519 key (RFC 0005 §2.1). The **worker** is whatever process holds the
matching private key. This document binds authorization to the executor
identity, so the question "which worker?" always has the answer the chain
already gives.

---

## 3. Five things that must never be confused

| Concept | What it is | Secret? | May appear on-chain? |
|---|---|---|---|
| **Identifier** | a name for a private object, unique within the data plane; opaque; carries no rights | no | never required; nothing in the protocol references it |
| **Locator** | where the object can be reached — an endpoint, path or address | not secret, but **confidential by default** (§13); grants nothing | never required |
| **Commitment** | the 32-byte `input_commitment` / `output_commitment` of the accepted protocol | no — it is public by design | yes — that is its purpose |
| **Authorization capability** | a short-lived, bound grant that lets one executor perform one operation on one object for one task | the capability's proof material is secret until presented; the grant itself must be treated as sensitive | **never** |
| **Secret** | content-encryption keys, blinding randomness, capability proof material, private keys | yes | **never** |

Knowing an identifier is not knowing where the object is. Knowing where it
is is not being allowed to read it. Being allowed to read it is not knowing
the key that decrypts it. A design that lets any of these stand in for
another has a leak.

---

## 4. Private input: creation and reference

### 4.1 Creation

The client produces a private input object in five steps, all off-chain:

1. **Represent.** Choose the byte representation of the input the model will
   consume. This is an application decision (§8); the chain has no opinion.
2. **Commit.** Compute `input_commitment` over those bytes, either by the
   RFC 0005 §2.4 interoperability convention or, for sensitive inputs, as a
   blinded commitment under client-controlled randomness
   (`compute-privacy-data-plane.md` §12.4). The commitment is what the task
   will carry; it is public.
3. **Protect.** Encrypt the bytes for the intended recipient class — for
   ordinary execution, a key the executor will receive; for a future
   confidential profile, a key released only to an attested environment (§9).
   Encryption is recommended at rest even where the provider will decrypt.
4. **Store.** Place the protected object in the data plane. The data plane
   returns an **identifier** and a **locator**. The object is `created` (§12)
   and owned by the client.
5. **Describe.** Assemble the private input reference (§4.2), keep the
   blinding randomness and content key in the client's own secret store, and
   only then submit the `ComputeTask` naming the chosen executor.

Ordering matters: the object should exist before the task does, so that an
accepted task never points at nothing (§10).

### 4.2 The private input reference

The reference is the descriptor that lets the parties talk about one private
object. Conceptually it associates:

| Element | Meaning | Notes |
|---|---|---|
| task identity | the `task_id` the object serves | derived per RFC 0005 §2.2; a reference may exist before the task is submitted, in which case the client fills this in at submission |
| commitment | the `input_commitment` the task carries | identical bytes to the on-chain field; the data plane may use it to refuse a wrong object (§7) |
| identifier | the data plane's name for the object | opaque |
| locator | how the worker reaches the data plane for this object | confidential by default |
| representation | how to read the bytes: a versioned application format tag | the same convention RFC 0005 §2.12 asks applications to adopt for `execution_spec` |
| lifecycle | `expires_at` and current state (§12) | data-plane authority |
| encryption metadata | *optional*: algorithm identifier, key identifier, wrapped content key for the intended recipient | the key itself is never here; a wrapped key for a recipient other than the presenter is useless to the presenter |

**What the reference is not.** It is not a credential (§5). It is not on the
chain. It is not secret in the cryptographic sense, but it names a locator
and so is treated as confidential and shared only with the control plane and
the authorised worker. Its exact field names and serialisation are **not
fixed here**; nothing in repository authority requires a wire format for it,
and freezing one now would be a premature protocol.

---

## 5. Authorization

### 5.1 Who authorises what

| Question | Answered by |
|---|---|
| Which executor may answer this task? | **the chain** — `task.executor`, immutable (RFC 0005 rule s) |
| When, for which object, for how long, and under which policy may that executor fetch the input? | **the control plane**, or the client directly — by issuing a capability |
| Is this presented capability valid, unexpired, unrevoked, unconsumed, and presented by the executor it names? | **the private data plane** — enforcement |
| Is the fetched content the content the task committed to? | **the worker** — verification (§7) |

The data plane trusts no single issuer blindly. Before serving, it checks that
the capability's executor **equals the executor the chain committed for that
`task_id`**. It may learn that fact from the client's registration of the
object, from a control-plane relay, or by reading the task from the chain
through the existing block RPC; the first worker may rely on the client's
registration, but an implementation should prefer the chain when it can read
it, because then even a compromised control plane cannot authorise a worker
the client did not name (§15).

### 5.2 Minimum invariants

| Invariant | Meaning |
|---|---|
| worker-scoped | a capability names one executor identity and is honoured only when presented with proof of possession of that identity's key |
| task-scoped | a capability names one `task_id` and one object; it cannot retrieve another task's object even if the same worker is authorised for both |
| least privilege | one operation per capability: fetch-input, put-result, or get-result — never a general grant |
| bounded | every capability has a not-after time; the data plane refuses after it regardless of any other check |
| revocable | the issuer can revoke before expiry; the data plane consults revocation before serving |
| consumable | fetch-input is single-use by default: the data plane records consumption and refuses a second presentation of the same capability. A bounded-use variant for retry (§10) must be explicit, not implied. |
| replay-resistant | each capability carries a unique identifier and a nonce; presentation binds a fresh challenge to the executor's key so that an observed presentation cannot be replayed |
| non-transferable | a capability observed by anyone other than the named executor is useless without that executor's private key |
| never on-chain | no capability, capability identifier, proof, or locator is ever written to `execution_spec`, `metadata` or any chain field (§14) |

A `task_id` is public the moment the task is in a block. **It is an
identifier and nothing else.** Presenting a `task_id` to the data plane
without a capability yields nothing (F3).

### 5.3 The capability, conceptually

A capability binds together, so that changing any one invalidates the whole:

```
capability {
  capability_id        unique; the unit of consumption and revocation
  task_id              the task this grant serves
  executor             the Address the task names; the only party who may present it
  operation            fetch-input | put-result | get-result
  resource             the object identifier the operation applies to
  not_before, not_after
  issuer               the client, or a control-plane service the client delegated
  issuer_signature     over all of the above
}
presentation {
  capability
  challenge            fresh, data-plane-supplied or time-bound
  executor_signature   over (capability_id || challenge), by the executor key
}
```

What must be bound *cryptographically*: the executor identity (via the
required proof of possession), the task, the resource, the operation and the
expiry (all under the issuer's signature). What may be bound by *policy* in
the data plane's state: revocation and consumption. The exact encoding is
not fixed here.

| Property of the capability | Answer |
|---|---|
| may be public? | **no** — treat as sensitive; the grant plus the executor key is access |
| must remain secret? | the executor's proof key must; the grant itself should be confined to issuer, data plane and named executor |
| may appear on-chain? | **never** (F8) |
| may be logged? | only its `capability_id` and outcome (§13) |
| may be reused? | fetch-input: no by default; get-result: bounded by policy; never across tasks or executors |

No reusable secret is placed on-chain, and no reusable secret is required
anywhere in this design: every grant is short-lived and bound (F8).

---

## 6. Fetch lifecycle

```
1  client stores the object; data plane: created → available
2  client submits ComputeTask naming executor E; chain accepts (rules k–p)
3  client (or delegated control plane) issues fetch-input capability C
   bound to (task_id, E, object); data plane: available → authorized
4  worker holding E's key presents C with proof of possession
5  data plane validates: signature of issuer; E == chain's task.executor;
   task_id and resource match; time window; not revoked; not consumed
6  data plane returns the protected object and marks C consumed;
   object: authorized → consumed
7  worker decrypts (ordinary profile) and verifies the content against
   input_commitment (§7)
8  execution begins only after step 7 succeeds
```

Failure behaviour — the data plane refuses without revealing *why* beyond
the class needed for the caller to act, and never returns partial content:

| Condition | Where detected | Behaviour |
|---|---|---|
| unknown task | data plane (step 5) or chain read | refuse; nothing served |
| unknown object | data plane | refuse; do not reveal whether the identifier ever existed to an unauthorised caller |
| expired capability | data plane | refuse; a new capability is required |
| revoked capability | data plane | refuse; revocation is final for that `capability_id` |
| wrong executor (key does not match `executor`) | data plane | refuse; record the attempt (§13) |
| wrong task (capability's `task_id` ≠ object's task) | data plane | refuse; this is a cross-task substitution attempt |
| commitment mismatch | **worker** (step 7); optionally the data plane at store time (§7) | worker **must not execute**; reports to the client; no receipt is produced for that task |
| corrupt payload (decryption or integrity failure) | worker | same as mismatch: no execution |
| temporary storage failure | data plane | transient error; capability not consumed; worker may retry within the window |
| authorization replay (same `capability_id` presented again) | data plane | refuse; already consumed |
| one-time capability already consumed by a crash after fetch | data plane | refuse; the worker must obtain a fresh capability (§10) — a consumed grant is not silently reopened |

No transport status codes are specified; none exist in repository authority.

---

## 7. Commitment verification

**What is committed.** The bytes over which the client computed
`input_commitment` — either the RFC 0005 §2.4 interoperability convention
over the input bytes, or a blinded commitment over the same bytes plus
client randomness. The task carries the result; the chain checks only that
the receipt repeats it (rule r).

**Who verifies, and when.** The **worker**, after decryption and before
execution (§6 step 7). It recomputes the commitment from the bytes it holds —
using the randomness delivered with the object when the commitment is
blinded — and compares it to the `input_commitment` of the task it is
answering. The data plane **may** additionally refuse to store an object
whose declared commitment does not match its content, as a courtesy check;
that does not relieve the worker of its own verification, because the worker
is the party that signs the receipt.

**On mismatch.** The worker does not execute and does not anchor. Executing
anyway would produce a receipt whose `input_commitment` either fails rule (r)
— if the worker writes what it actually received — or lies about what was
computed — if it writes the task's value. Neither is acceptable (F7).

**No second protocol.** This document does not define a new commitment. It
uses the accepted `input_commitment`, honours both derivations the parent
architecture already sanctions, and adds only *who checks it and when*.

---

## 8. Canonicalization boundary

Repository authority defines canonical encodings for the **task envelope**
(RFC 0005 §2.1–2.2), the **receipt** (`RECEIPT_SPEC_v0.1` §3) and the
**transaction** (`PROTOCOL_LOCK_v0.3` §1). It defines **no canonical
representation of private compute input**: RFC 0005 §2.4 commits to
`input_bytes` and says nothing about what those bytes are.

That is correct and is preserved. Two things must not be confused:

- **Protocol commitment semantics** — consensus checks equality of 32-byte
  values. Fixed by RFC 0005. Not touched here.
- **Application canonicalization** — how a prompt, a document, a tensor or a
  dataset becomes the exact bytes that were committed, so that client and
  worker compute the same commitment. **An application convention**, carried
  by the `representation` tag in the reference (§4.2), versioned by the
  application, and invisible to the chain.

Left deliberately unresolved: no repository-wide canonical input format is
chosen by this gate. Choosing one is not needed by the first worker, which
serves one application it can agree a representation with, and choosing one
casually would create a consensus-adjacent encoding no RFC has accepted.

---

## 9. Worker trust boundary and the confidential extension point

The same contract serves both execution profiles of the parent architecture.

**Ordinary profile (first worker).** The worker receives a content key it can
use — delivered with the capability, wrapped to a key the worker holds — and
decrypts in ordinary memory. **The provider can see the plaintext.** This is
stated, not hidden (F12). No attestation is involved and no TEE is required
(F14).

**Confidential profile (future).** The contract changes at exactly one point:
**key release becomes conditional.** The content key is not delivered with
the capability; the environment first produces attestation evidence, a
verifier (control plane or client) checks it against policy, and only then is
the key released to that environment. Everything else — object, reference,
capability, fetch, verification, result, receipt — is unchanged. The
extension points are therefore:

| Extension point | Ordinary profile | Confidential profile |
|---|---|---|
| attestation verification | none | inserted between capability presentation and key release; performed off-chain (`compute-privacy-data-plane.md` §11) |
| conditional key release | key wrapped to the executor key | key wrapped to a key the attested environment proves it holds |
| encrypted input | recommended | required |
| encrypted output | recommended | required, to a client key |

Because `ComputeTask` never carries a key, a locator or a capability, none of
this touches the envelope. The task/data-plane separation does not need to be
redesigned for confidential execution (F13); it needs a key-release policy.

---

## 10. Private result: creation, reference and lifecycle

The result flow mirrors the input flow with the roles of owner and consumer
exchanged.

```
worker
  ├─ compute output bytes; compute output_commitment (RFC 0005 §2.4
  │  convention, or blinded — the same choice the client made for input,
  │  agreed through the application representation)
  ├─ encrypt output to the client (or to the key the client designated)
  ├─ present put-result capability; data plane stores the object,
  │  bound to (task_id, executor)                              ── data plane
  └─ only after the store is durable: build Receipt, sign, anchor ── chain
client
  └─ present get-result capability; fetch; verify against the anchored
     receipt's output_commitment; decrypt
```

**The private result reference** associates the same elements as §4.2 with
the roles inverted: task identity, `output_commitment`, identifier, locator,
representation, lifecycle, optional encryption metadata for the client.

| Concern | Position |
|---|---|
| ownership | the result object belongs to the **client**; the worker is its author, not its owner, and loses access when its put-result capability is consumed |
| authorization to retrieve | a get-result capability issued to the client (or the client's designated key) by the data plane or the issuing service; bounded and revocable like any other (F15) |
| result commitment | `Receipt.output_commitment`, exactly as the accepted protocol carries it; verification by the client after retrieval; the chain never sees the output (RFC 0005 §2.4) |
| expiry and deletion | data-plane lifecycle; the anchored receipt outlives the object (§12) |
| retry after failed upload | the put-result capability is bounded-use or re-issuable within its window; the worker retries the store, not the computation |
| duplicate publication | a second put for the same `(task_id, executor)` is refused once a result is durable; the data plane is authoritative for "a result exists" |
| worker failure after compute, before upload | no result object, no receipt; the task is still open on-chain; the worker recomputes or the client submits a new task |
| worker failure after upload, before anchoring | result exists; no receipt; the worker may anchor later; the client can observe the result exists but has no on-chain attribution until the receipt lands |
| receipt before result | **not permitted by this contract.** The receipt is permanent and the result is deletable; anchoring a permanent claim to a result that may never exist is the wrong order. Anchor only after the store is durable. |
| receipt after result | the normal path; a receipt whose result was later deleted (§12) is a permanent record that the work was claimed, and nothing more |

---

## 11. Ordering, atomicity and who is authoritative

The chain, the data plane and the worker are three systems with no shared
transaction. The contract does not pretend otherwise. For each window, one
subsystem is the source of truth about the state:

| Window | Observable state | Authority | Resolution |
|---|---|---|---|
| input stored, task never submitted | object `available`, no task | data plane | object expires by its own lifecycle; nothing on-chain |
| task accepted, input unavailable (never stored, expired, deleted) | task registered, no object | chain for the task; data plane for the object | executor cannot fetch; task stays open forever unless a receipt is anchored; the client may store the object late, or abandon the task |
| worker authorised, capability delivery fails | task registered, object `authorized`, worker holds nothing | control plane / issuer | re-issue; the undelivered capability expires |
| worker fetches, then crashes before execution | capability `consumed`, object unchanged | data plane | fresh capability required; the object is not lost; plaintext held by the crashed worker is that worker's cleanup obligation (§14) |
| result computed, upload fails | no result object | data plane | retry the store within the put window; the receipt is **not** anchored yet, by construction (§10) |
| result uploaded, receipt/anchor fails | result object `available`, no receipt | chain | the worker retries anchoring; first-anchored-wins on `task_id` (RFC 0002 rules i, j) means a retry cannot double-anchor |
| receipt anchored, result later expires or is deleted | receipt permanent, object gone | chain for the claim; data plane for the content | the receipt remains true — the work was claimed under that commitment — and the content is unavailable; deletion never touches the receipt (F24) |

Two of these windows are worth a sentence each because they are the ones a
worker author will meet first. A **consumed one-time capability after a
crash** is not reopened; the worker must obtain another. That is deliberate:
silently reopening consumed grants is how replay resistance quietly
disappears. And **receipt before result** is not a race to be handled; it is
an ordering the contract forbids.

---

## 12. Lifecycle states, retention and deletion

Conceptual states of a private object, named for clarity rather than fixed:

```
created ──► available ──► authorized ──► consumed ──► expired / deleted
                │              │                          ▲
                └──────────────┴──── revoked ─────────────┘
```

| State | Meaning |
|---|---|
| created | stored; commitment recorded; not yet referenced by any capability |
| available | eligible for authorization |
| authorized | at least one unexpired, unrevoked capability exists for it |
| consumed | the single-use fetch happened; further fetches need a new grant |
| revoked | the owner withdrew access; no capability for it will be honoured |
| expired | its lifetime elapsed; the data plane may delete it |
| deleted | content and keys destroyed; the identifier may be retained as a tombstone |

**What deletion means.** The data plane no longer holds the content, and the
keys that could decrypt any surviving copy are destroyed. Copies the worker
made during execution are governed by the worker's own destruction
obligations (§14), not by the data plane.

**What deletion does not mean.** It does not erase the `ComputeTask` in its
block, the `input_commitment`, the `Receipt`, the `output_commitment`, or
any transaction. Those are permanent by construction
(`compute-privacy-data-plane.md` §15). A deleted object referenced by a
permanent receipt is the expected end state of every task, not an anomaly
(F11, F24).

---

## 13. Logging and observability

Operational logs must not become a second, unprotected copy of the private
data plane. Default rules for every component in this contract:

| Item | May appear in logs? |
|---|---|
| `task_id` | yes — it is public on-chain |
| executor identity (`Address`) | yes — public on-chain |
| object identifier | yes — it is opaque and grants nothing |
| locator | **no** by default; if operationally necessary, only in access-controlled diagnostic logs, never in shared or long-retained logs |
| capability | **no**; only `capability_id` and the validation outcome |
| capability proof material, challenge signatures | **no** |
| raw payload, plaintext input or output, decrypted fragments | **never** |
| content-encryption keys, wrapped keys, blinding randomness | **never** |
| `input_commitment`, `output_commitment` | yes — they are public on-chain |
| error details | the class (expired, revoked, wrong executor, mismatch) — yes; any content of the object that caused it — no |

Failed presentations should be logged (class, `capability_id`, presenting
key) because they are the signal that a capability leaked. Successful fetches
should be logged without the locator (F19).

---

## 14. Secret handling

| Secret | Holder | Lifetime | Destruction |
|---|---|---|---|
| capability proof material (executor private key) | worker | the executor identity's lifetime | the worker's key-management concern; not a per-task secret |
| capability grants | issuer, data plane, named executor | until `not_after`, consumption or revocation | discard after use; never persist beyond the window |
| content-encryption key (input) | client; released to worker (ordinary) or attested environment (confidential) | one task | worker destroys it after execution; client retains only if it must re-serve the object |
| content-encryption key (output) | worker (to encrypt); client (to decrypt) | one task | worker destroys after upload |
| blinding randomness | client; delivered to worker with the object | one task | worker destroys after verification and receipt construction |
| temporary plaintext | worker memory and scratch | execution | overwrite or discard at completion; a crashed worker's operator is responsible for the residue |
| worker-local artifacts (caches, intermediate tensors, logs) | worker | execution | same; must not be retained across tasks by default |
| credentials to the data plane or control plane (service identity) | each component | operational | ordinary credential hygiene; not a per-task matter |

No secret manager, HSM, KMS or vault is chosen. Any of them may satisfy the
lifetimes above.

---

## 15. Threat model

| Threat | Result | How |
|---|---|---|
| malicious client publishes private bytes in `execution_spec` or `metadata` | **accepted** — protocol permits, architecture forbids | stated in §16; the client has published its own data; no guarantee is offered against self-harm |
| malicious worker anchors a wrong `output_commitment` | **accepted** at this layer | RFC 0005 §9.2 — no correctness claim exists; verification is future work ([#52](https://github.com/MbongoChain/mbongo-chain/issues/52)) |
| curious ordinary provider reads plaintext during execution | **accepted** for the ordinary profile; **future capability** to mitigate | §9; confidential profile |
| compromised worker host exfiltrates plaintext and keys | **accepted** for the ordinary profile; mitigated by per-task keys and destruction (§14); **future capability** via confidential execution | the blast radius is the tasks that worker was authorised for, never other executors' tasks (F22) |
| compromised control-plane component issues a capability to the wrong worker | **prevented** when the data plane checks `executor` against the chain's `task.executor` (§5.1); **mitigated** to the client-registered executor otherwise | the chain, not the control plane, is the authority on who may answer |
| stolen capability | **mitigated** — useless without the executor's private key; bounded by expiry; single-use | §5.2, §5.3 |
| capability replay | **prevented** — `capability_id` consumption plus a fresh challenge under the executor key | §5.2 |
| cross-task reference substitution (capability for task X presented for task Y's object) | **prevented** — the capability binds `task_id` and `resource`; the data plane matches both | §5.2, §6 |
| cross-worker authorization substitution (worker B presents worker A's grant) | **prevented** — proof of possession of A's key is required | §5.2 |
| tampered private payload in the data plane | **detected** — commitment verification by the worker before execution; execution refused | §7 |
| wrong payload under the correct locator | **detected** — same mechanism; a locator is not a promise of content | §7 |
| stale result retrieval (result replaced or expired) | **detected** — the client verifies against the anchored `output_commitment`; **prevented** at the store by refusing duplicate puts | §10 |
| logs leaking locators, capabilities or content | **mitigated** by §13 defaults; **accepted** that a component ignoring §13 leaks — this is a conformance test target (epic workstream I) | §13 |
| public-chain metadata abuse (linkage by `submitter`/`executor`/timing/commitment equality) | **accepted** — not addressed by this contract | `compute-privacy-data-plane.md` §13 |
| deleted object still referenced on-chain | **accepted and expected** — the receipt is a permanent claim; the content is gone | §12 |

---

## 16. Public-chain exclusion proof

Field by field, what the first reference worker needs to write on-chain, and
whether any private content or reusable credential is among it:

| Field | Protocol allows arbitrary bytes? | Architecture requires private content or a credential here? |
|---|---|---|
| `ComputeTask.version` | no (must be 1) | no |
| `ComputeTask.submitter` | no (an `Address`) | no |
| `ComputeTask.executor` | no (an `Address`) | no — it is the *public* identity authorization is later bound to, not a credential |
| `ComputeTask.salt` | 32 opaque bytes | no — task-identity uniqueness only |
| `ComputeTask.input_commitment` | 32 bytes | no — a commitment, public by design |
| `ComputeTask.execution_spec` | **yes**, ≤ 1024 bytes | **no** — the worker learns *what* to do from it (a representation tag, a model identifier, parameters); it learns *the input* from the data plane. No locator, capability or key belongs here. |
| `Receipt.version`, `task_id`, `input_commitment`, `output_commitment`, `executor` | no / 32-byte values | no |
| `Receipt.metadata` | **yes**, ≤ 4096 bytes | **no** — nothing in this contract writes to it; an application-layer pointer is permitted by the receipt spec, and if used it must be an identifier or commitment, never a locator, capability or key |
| `Receipt.signature`, `Transaction.signature` | no | no |
| `Transaction.amount`, `nonce`, `receiver`, `tx_type` | no | no |
| events | no event mechanism exists in the accepted protocol (RFC 0005 §11 defers the event model) | not applicable |
| memo-like fields | none exist in the transaction (`PROTOCOL_LOCK_v0.3` §1) | not applicable |

Two fields **allow** arbitrary bytes. **None requires** private content, a
locator, a capability, a key or any reusable credential (F1, F2, F8). That a
client *can* put a CV into `execution_spec` is a fact about the protocol's
opacity; it is not a privacy feature, not a guarantee, and not something this
contract relies on or protects against. It is the conformance target of epic
workstream I.

---

## 17. What the first worker's author can now answer

| Question | Answer |
|---|---|
| Where is the private input? | in the private data plane, as an object the client stored before submitting the task (§4.1) |
| How is it referenced? | by a private input reference: task identity, commitment, identifier, locator, representation, lifecycle, optional encryption metadata (§4.2) |
| Who can authorise access? | the client, or a control-plane service the client delegated — for the executor the chain names, and no one else (§5.1) |
| How does the worker prove authorization? | by presenting a bound, expiring, single-use capability together with proof of possession of the executor key over a fresh challenge (§5.3) |
| How is replay prevented? | `capability_id` consumption, challenge binding, expiry, and task/executor/resource binding (§5.2) |
| How does the worker verify the input? | by recomputing `input_commitment` over the received bytes (with the blinding randomness, if any) and refusing to execute on mismatch (§7) |
| Where does the result go? | to the private data plane under a put-result capability, encrypted to the client, **before** the receipt is anchored (§10) |
| How is the result later retrieved? | by the client presenting a get-result capability and verifying against the anchored `output_commitment` (§10) |
| What happens when any step fails? | §6 for fetch, §10 for result, §11 for every window in between — with the authoritative subsystem named |
| What can be deleted? | off-chain objects, keys and worker residue (§12, §14) |
| What remains permanently on-chain? | the task, both commitments, the receipt, every transaction (§12, §16) |
| What changes later for confidential compute? | only key release becomes conditional on attestation (§9); nothing in the task or the contract's shape changes |

---

## 18. Invariants

| # | Invariant | Status |
|---|---|---|
| F1 | raw private input is never required on-chain | ALREADY_TRUE (RFC 0005 §2.1; §16) |
| F2 | raw private output is never required on-chain | ALREADY_TRUE (RFC 0005 §2.4; §16) |
| F3 | `task_id` alone grants no private-data access | DEFINED_BY_THIS_GATE (§5.2) |
| F4 | a worker authorization is bound to the intended worker | DEFINED_BY_THIS_GATE (§5.2, §5.3) |
| F5 | a worker authorization is bound to the intended task and resource | DEFINED_BY_THIS_GATE (§5.2, §5.3) |
| F6 | private input integrity can be checked against `input_commitment` | DEFINED_BY_THIS_GATE (§7) — the commitment already exists; the check is placed |
| F7 | commitment mismatch prevents execution | DEFINED_BY_THIS_GATE (§7) |
| F8 | reusable data-plane credentials are not required on-chain | ALREADY_TRUE (§16) and DEFINED_BY_THIS_GATE (§5.3: none required anywhere) |
| F9 | a public locator is not automatically an authorization credential | DEFINED_BY_THIS_GATE (§3, §5.2) |
| F10 | expiration and revocation are data/control-plane concepts | DEFINED_BY_THIS_GATE (§5.2, §12) |
| F11 | deletion applies only to off-chain private content | ALREADY_TRUE (`compute-privacy-data-plane.md` §15); restated §12 |
| F12 | ordinary execution may expose plaintext to the provider | ALREADY_TRUE (parent §7); acknowledged §9 |
| F13 | confidential execution can later constrain key release by attestation | DEFINED_BY_THIS_GATE as an extension point (§9); FUTURE_CAPABILITY as behaviour |
| F14 | confidential compute is not required for the first worker | ALREADY_TRUE (parent §22); §9 |
| F15 | private result access has an explicit authorization model | DEFINED_BY_THIS_GATE (§10) |
| F16 | receipt correctness claims are not expanded | ALREADY_TRUE (RFC 0005 §9.2); §15 |
| F17 | the public chain does not become scheduler or object store | ALREADY_TRUE (parent P14); §1, §16 |
| F18 | no vendor becomes protocol authority | ALREADY_TRUE; this document names none normatively |
| F19 | logs must not contain raw private payload or reusable secrets by default | DEFINED_BY_THIS_GATE (§13) |
| F20 | task reassignment cannot implicitly transfer old worker authorization | ALREADY_TRUE structurally (RFC 0005 §2.6: a different executor is a different task) and DEFINED_BY_THIS_GATE (§1, §5.2) |
| F21 | cross-task capability replay is rejected | DEFINED_BY_THIS_GATE (§5.2, §15) |
| F22 | cross-worker capability replay is rejected | DEFINED_BY_THIS_GATE (§5.2, §15) |
| F23 | result persistence and receipt anchoring failure windows are explicit | DEFINED_BY_THIS_GATE (§10, §11) |
| F24 | deleting an object does not erase on-chain history | ALREADY_TRUE; restated §12 |

Every DEFINED_BY_THIS_GATE item is also REQUIRES_IMPLEMENTATION: the data
plane, issuer and worker behaviour it describes do not yet exist. Conflicts
with existing authority: **none.**

---

## 19. Unresolved by this gate, deliberately

- **Application canonicalization of input bytes** (§8) — an application
  convention, not chosen repository-wide.
- **Capability wire format and transport** — conceptual binding only; the
  first worker and its data plane may pick any encoding that preserves §5.3.
- **Delegation model between client and control plane** — both direct client
  issuance and client-delegated control-plane issuance are permitted; which
  the first deployment uses is an implementation choice.
- **How the data plane reads the chain's `task.executor`** — the block RPC
  suffices today; a task lookup by `task_id` does not exist and is not
  proposed here (reserved compute RPC names stay reserved, RFC 0005 §7).

None of these is required for the first reference worker to be specified,
and none touches consensus.

---

## 20. Relationship to other authority

- **RFC 0005** remains normative for the envelope, task identity, commitment
  binding, executor authorisation, storage and activation. This document
  implements its step 5 off-protocol, exactly as it asks. If any sentence
  here conflicts with RFC 0005, RFC 0005 wins.
- **`RECEIPT_SPEC_v0.1` and RFC 0002** remain normative for the receipt.
  Nothing here redefines a receipt field or its validation.
- **`compute-privacy-data-plane.md`** remains the parent architecture. This
  document is the handoff contract it calls for in §22 item 4, and defers to
  it on every boundary question.
- **`rpc_v0.2.md`** is unchanged; this contract needs no RPC method beyond
  `submit_transaction` and `get_block_by_height`.

---

## See also

- [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md) — parent architecture
- [RFC 0005 — Compute Task Commitment](../rfcs/0005-compute-task-commitment-v1.md) — normative, Accepted
- [RFC 0002 — Receipt Anchoring](../rfcs/0002-receipt-anchoring-v0.3.md) — normative
- [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md) — receipt structure
- [`compute-receipts.md`](compute-receipts.md) — what the chain does with receipts today
- [#126](https://github.com/MbongoChain/mbongo-chain/issues/126) — the Compute vertical epic (this is Workstream F)
- [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) — verification research (future)
