# Control plane and worker interface: the coordination contract

> **Document type:** Architecture — interface contract for off-chain components
> **Status:** Architectural authority for what the Compute control plane may
> and must do between an accepted `ComputeTask` and an anchored `Receipt`,
> and for how a worker relates to it. Defines no consensus rule, no RPC
> method, no SDK type, no wire format and no transport; where anything here
> appears to conflict with a normative source below, the normative source
> wins.
> **Normative sources:** [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md)
> (Accepted), [RFC 0002](../rfcs/0002-receipt-anchoring-v0.3.md),
> [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md),
> [`PROTOCOL_LOCK_v0.3.md`](../specs/PROTOCOL_LOCK_v0.3.md) (FROZEN),
> [`rpc_v0.2.md`](../specs/rpc_v0.2.md) (FROZEN),
> [`VISION_v1.md`](../VISION_v1.md)
> **Parent architecture:** [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md)
> (the four-plane model) and
> [`compute-private-data-plane-interface.md`](compute-private-data-plane-interface.md)
> (the data-plane handoff contract, "F"). This document refines plane 2 and its
> relationship to plane 4, reuses F's capability model unchanged, and yields to
> both on any conflict.

This is Workstream E of the Compute vertical epic
([#126](https://github.com/MbongoChain/mbongo-chain/issues/126)) — the last
architecture prerequisite for the first reference worker. Completing it does
not make the worker runnable: workstreams A (protocol implementation) and D
(SDK types) remain open, and the epic's dependency graph governs.

Nothing here is implemented. The repository contains no control plane, no
worker, no lease, heartbeat or attempt abstraction of any kind; `crates/
mbongo-compute` is an empty placeholder whose comments predate the current
vision and are not authority.

---

## 1. The fact everything else follows from

RFC 0005 settles who executes a task, and settles it **before** the task is
committed:

- the client names `task.executor` in the envelope (§2.1), consensus binds
  the receipt to that executor (rule s, §3), and the envelope is immutable
  once registered (§8);
- a task with a different executor is a **different task** with a different
  `task_id` (§2.6) — "a client may ask two executors the same work".

So the control plane does **not** choose an executor after submission, does
not assign, and cannot reassign. Its job begins after a task exists and is
bounded by that task's `executor`. If work must move to another executor,
that is the client submitting a **new task** — an existing protocol
operation, not a control-plane feature — and every lease and capability for
the old task simply expires (E1, E2).

What the control plane *does* own, between an accepted task and an anchored
receipt: discovering that the task exists, confirming a worker belongs to
the named executor, deciding *when* execution may proceed, obtaining the
data-plane authorization F defines, watching the attempt, and coordinating
the result and receipt path. Each of those is operational coordination. None
of it is protocol truth (E5).

---

## 2. Identities that are not the same thing

| Concept | What it is | Authority |
|---|---|---|
| **executor identity** | an `Address` — the public half of an Ed25519 key named in `task.executor` | chain (RFC 0005 §2.1) |
| **provider** | the organisation operating hardware; may run many executors, or one executor across many machines | control-plane attribute; not protocol |
| **worker process** | a running program that holds, or can use, the executor's private key | operational |
| **worker instance** | one live incarnation of a worker process, distinguished across restarts | operational; the unit a lease is granted to |
| **machine** | where the instance runs | operational; never protocol |
| **control-plane session** | an authenticated relationship between one worker instance and the control plane | control plane |

A worker **proves it belongs to an executor** exactly as F requires for the
data plane: **proof of possession** of the executor's private key over a
fresh challenge (F §5.3). The control plane verifies the signature against
the `Address` the task names. No registry, no certificate, no new identity
scheme; the protocol identity is reused (E3). Many instances may hold the same
key — that is the provider's business — but every one of them proves the same
thing, and none can prove a different executor's key.

---

## 3. Three grants that must not be confused

| Grant | Issued by | Answers | Scope | Lifetime |
|---|---|---|---|---|
| **task authority** | the chain, by including the `ComputeTask` | *who may answer this task* — `task.executor` | one task, forever | permanent |
| **execution lease** | the control plane | *which worker instance is coordinating an attempt at this task right now* | one task, one executor, one instance, one attempt | short, renewable, revocable |
| **data-plane capability** | the client or its delegated issuer (F §5) | *may this executor fetch this object / store this result* | one task, one executor, one object, one operation | short, single-use by default |

A lease is not a capability and does not contain one (E6). Holding a lease
gives a worker no bytes; presenting a lease to the data plane yields nothing.
The lease's only power is coordination: it tells the control plane which
instance it is talking to about this task, and it is the handle under which
the control plane *obtains or relays* a capability. A lease is also not task
authority (E7): the chain never sees it, and a worker with a lease but
without the executor key still cannot anchor.

Whether the control plane issues capabilities itself (as the client's
delegate) or relays ones the client issued is an implementation choice F
already permits (F §19). Either way the composition is the same: **lease
first, then capability under that lease** (§7).

---

## 4. Task discovery

**A discoverable task** is a `ComputeTask` transaction included in a block
whose `executor` is one the control plane serves. **An accepted task** is the
same thing: under RFC 0005 there is no pending, assigned or activated
state — a task is either in the `tasks` column family (submitted) or not, and
either has a receipt (completed) or not (§4.1). Those two derived states are
the *only* task states, and this document defines no others (E4, E5).

**Evidence before offering work.** The control plane must have observed the
task in a block, through the existing block RPC (`get_block_by_height`,
`rpc_v0.2.md` §2.6), and must have verified from that block that
`task.executor` is the executor it is acting for. The reserved compute RPC
names stay reserved (RFC 0005 §7); no task-lookup or executor-index RPC
exists and none is proposed here. An implementation may keep a local index of
observed tasks; that index is a **cache** derived from chain state, never a
second source of truth (§16).

**Stale and unfinalised state.** The accepted protocol defines no finality
depth and no reorg handling — block inclusion is the fact the protocol
records. The control plane may apply a *confirmation depth* before acting as
operational policy; doing so is prudence, not protocol. If chain state the
control plane relied on later changes, every lease derived from it is stale
and must be re-validated against the chain before any capability is obtained
(§15).

**Discovery grants nothing** (E4). Observing a task, indexing it, or being
the first to see it confers no authority; only `task.executor` and the key
behind it do.

---

## 5. Worker session

The minimum relationship the first worker needs:

| Element | Requirement |
|---|---|
| authentication | the worker proves possession of the executor key over a control-plane challenge; the control plane authenticates itself to the worker by transport-level means the deployment chooses |
| session | short-lived, bound to one worker instance and one executor identity; a session cannot be used for another executor (E-threat, §12) |
| compatibility | the worker states the version of this contract and the application representation tags it can execute (F §4.2); the control plane refuses incompatible sessions |
| liveness | the session carries the heartbeat of §9 |

No on-chain registration exists or is proposed (RFC 0005 §6 keeps
assignment and discovery off-protocol). Registration is a session, not a
transaction.

### 5.1 Capability advertisement

A worker may advertise what it can run: runtime and model family, resource
availability, supported execution profile, and — in future — attestation
capability. These are **worker claims**, evaluated by **control-plane
policy**. Nothing about them is cryptographically proven today, and the
contract must not label them as verified hardware facts. The only proven
property in the ordinary profile is possession of the executor key. Attested
properties are a future extension (§14). No vendor identifier is a protocol
concept (E28).

---

## 6. Execution admission

Before a worker may begin an attempt, these checks pass. Each has exactly one
authority, and the control plane must not substitute its own opinion for a
chain-derived fact:

| Check | Authority |
|---|---|
| the task exists in a block | CHAIN-DERIVED |
| `task.executor` equals the executor this session proved | CHAIN-DERIVED identity, WORKER-PROVEN possession |
| no receipt for this `task_id` is anchored | CHAIN-DERIVED — completed is a derived chain state (RFC 0005 §4.1) |
| no other live lease for this task is held by another instance of this executor | CONTROL-PLANE POLICY (§8) |
| the task is still eligible under the control plane's own policy (confirmation depth, client cancellation of coordination, quota) | CONTROL-PLANE POLICY |
| a data-plane capability can be obtained for this task and executor | DATA-PLANE DERIVED — the issuer's answer, not the control plane's assumption |
| the worker supports the task's representation tag and execution profile | WORKER-LOCAL claim, CONTROL-PLANE POLICY |

If any chain-derived check fails, no lease is issued. If a policy check
fails, the control plane declines or defers — it never "fixes" the task.

---

## 7. Work acquisition and the data-plane handoff sequence

```
1  worker session established; executor possession proven          (§5)
2  control plane offers a discoverable, admissible task              (§4, §6)
3  control plane issues an execution lease L
     { lease_id, task_id, executor, worker_instance, attempt_id,
       not_after, renewable, issuer signature }
4  under L, control plane obtains (or relays) a fetch-input capability C
     bound to (task_id, executor, object) per F §5.3
5  worker presents C to the data plane with proof of possession;
   data plane consumes C; worker verifies input_commitment (F §6–7)
6  worker reports "execution started" under L; control plane records it
7  worker executes; heartbeats renew L (§9)
8  worker obtains put-result capability under L; persists result;
   receives durable confirmation (F §10)
9  worker reports "result ready" under L
10 receipt material is produced and submitted (§11); control plane
   observes the anchored receipt on-chain
11 L is released; the attempt is terminal
```

Races, each with the rule that resolves it:

| Race | Rule |
|---|---|
| lease issued, capability creation fails | the lease is useless without input; the worker reports the failure, the lease is released or expires; **no execution starts** |
| capability issued, worker never receives the lease (or the lease was lost) | the capability is bound to the executor and expires on its own; the worker cannot act without a lease and must acquire one; an unconsumed capability past expiry is simply dead |
| worker holds the capability, lease expires before fetch | the worker must renew or reacquire the lease **before** presenting the capability; presenting under an expired lease is a stale-worker event (§9) |
| capability consumed, worker crashes before execution | the capability is spent (F §6, §11) and is **never reopened**; the retry is a new attempt with a new lease and a **fresh** capability (E10, E11) |
| fresh retry | new `attempt_id`, new lease, new capability; same `task_id`, same executor — nothing on-chain changes |

The ordering is fixed: **lease, then capability, then fetch, then start.** A
capability obtained without a lease is not usable under this contract even if
the data plane would honour it; a lease obtained without a capability
executes nothing.

---

## 8. Attempt identity

An **attempt** is one worker instance's effort at one task under one lease.
Attempts need an identity because a task can see a crash, a timeout, a
control-plane restart, an upload failure or a receipt failure, and the next
try must be distinguishable from the last without touching anything
on-chain.

`attempt_id` is:

- **off-chain only** — it appears in leases, control-plane state, data-plane
  capability issuance references and logs, and nowhere on the chain;
- **not part of task identity** — `task_id` is derived from the six envelope
  fields (RFC 0005 §2.2) and no attempt changes it (E9);
- **not part of executor identity** — the executor is the same across every
  attempt;
- unique per `(task_id, executor)` across the control plane's lifetime, so
  that a stale worker citing an old attempt is distinguishable from a live
  one; unpredictable enough that an attacker cannot forge a plausible current
  attempt.

A receipt carries no `attempt_id`. If two attempts both reach the receipt
stage, the chain settles it by first-anchored-wins (RFC 0002 rules i, j) and
the control plane records which attempt landed (§10).

---

## 9. Lifecycle, liveness and leases

### 9.1 Operational lifecycle

Illustrative control-plane states for one attempt. **This is a coordination
record, not a state machine the protocol knows** (E5). The chain knows two
task states; the data plane knows object states (F §12); the worker knows
its own execution; these are four different things and this table is only
the second.

```
DISCOVERED → ADMISSIBLE → LEASED → INPUT_AUTHORIZED → INPUT_CONSUMED
  → EXECUTING → RESULT_PERSISTED → RECEIPT_SUBMITTED → COMPLETED
                                                     ↘ FAILED / EXPIRED / RELEASED
```

`COMPLETED` means *a receipt for this task is observed on-chain* — a
chain-derived fact the control plane records, not one it decides.

### 9.2 Heartbeats and lease renewal

The first worker needs liveness signalling, because the alternative is a
lease that outlives a dead worker. Semantics, transport-free:

- a lease has `not_after`; the worker renews it by heartbeat before then;
- a heartbeat is authenticated under the session and names the `lease_id`;
- the control plane records `last_seen`; a lease past `not_after` without
  renewal is **expired**, and the control plane may issue a new lease for the
  task to another instance;
- progress signals are optional and informational; no percentage or phase is
  normative.

### 9.3 Worker disappearance, by phase

| Worker vanishes… | Consequence |
|---|---|
| before input fetch | lease expires; capability expires unused; a new lease and capability go to the next instance; nothing was consumed |
| after input fetch, before execution | capability is consumed and stays consumed; new attempt needs a fresh capability (F §11); the vanished instance may hold plaintext — its operator's destruction obligation (F §14) |
| during compute | same as above; any partial artifacts are the vanished instance's residue |
| after result persistence, before receipt | the result exists under `(task_id, executor)`; a new attempt does **not** recompute — it discovers the durable result (F §10 refuses a second put) and proceeds to the receipt step; the receipt's `output_commitment` must be the commitment of *that* result |
| after receipt submission, before observing it | the control plane resolves by lookup (§11), never by blind resubmission |

A worker that reappears citing an expired lease is a **stale worker**: its
reports are refused, its lease is not revived, and if it still holds an
unconsumed capability it must not present it. Fencing is by lease and
attempt identity, not by trust.

---

## 10. Duplicate execution

Two instances of the same executor may come to believe they own the same
task — after a network partition, a control-plane restart that forgot a lease,
or a heartbeat that was late rather than dead. This contract **does not claim
exactly-once execution** (E12); it claims **effectively-once coordination
with at-least-once fallback**:

- leases minimise duplicates: the control plane issues one live lease per
  `(task_id, executor)` and refuses a second while the first is live;
- a lease that expires releases the task; the old holder becomes stale;
- if two attempts nonetheless both compute, the **data plane** refuses the
  second result put for the same `(task_id, executor)` (F §10) — so at most
  one durable result exists per executor per task;
- if two attempts nonetheless both anchor, the **chain** decides:
  first-anchored-wins on `task_id` (RFC 0002 rules i, j), the second is
  rejected with `TaskIdAlreadyAnchored`, and the control plane records the
  winner. This document does not, and may not, decide receipt validity
  differently (E27).

Wasted computation under a duplicate is an availability cost, accepted. It is
never a correctness or privacy failure, because the input each attempt used
was verified against the same `input_commitment` and neither attempt can
anchor twice.

---

## 11. Result handoff and receipt submission

F's ordering is preserved unchanged: **the result is durable in the data
plane before the receipt is anchored, never after** (F §10, E13). The
control plane must never treat *receipt exists* as *result is retrievable*;
the receipt is permanent and the result is deletable (F §12).

Sequence: worker persists the result under a put-result capability → data
plane confirms durability → worker computes `output_commitment` over the
plaintext result per RFC 0005 §2.4 (or the blinded form the application
agreed) → worker reports "result ready" → receipt material becomes eligible.

### 11.1 Who submits the receipt

Repository authority does not mandate an orchestration style: RFC 0005 §10
says the *executor* builds, signs and anchors; the SDK exposes both signing
and submission (`signAnchorReceiptTransaction`, `submitAnchorReceipt`). Two
styles are therefore permitted, and one rule governs both:

- **worker submits** — the worker signs the receipt and the `AnchorReceipt`
  transaction and calls `submit_transaction` itself, then reports the
  transaction hash to the control plane;
- **worker signs, control plane relays** — the worker signs the receipt and
  the transaction, hands the **complete, signed** `AnchorReceipt` transaction
  bytes to the control plane, and the control plane calls
  `submit_transaction` on the worker's behalf.

**The one rule:** what crosses the worker → control-plane boundary is signed
receipt material — a signed receipt and, where relayed, a signed transaction.
**The executor's private key never crosses it.** The control plane can relay a
signature; it cannot produce one, and this is what keeps a compromised
control plane unable to anchor on an executor's behalf (§12).

### 11.2 Receipt submission retry and lookup

`submit_transaction` returns the transaction hash (`rpc_v0.2.md` §2.3).
Re-submitting an identical transaction is idempotent at admission; anchoring
a receipt whose `task_id` is already anchored is rejected by the chain
(`TaskIdAlreadyAnchored`, `PROTOCOL_LOCK_v0.3.md` §3). So:

- a lost response is **not** a failed submission. Before any resubmission,
  **look up**: scan blocks from the height at which the task was observed for
  a receipt with this `task_id` (the SDK's `receiptsInBlock` is the existing
  tool). If found, the attempt is `COMPLETED`; record the height.
- if not found and the transaction is not in a block, resubmit the **same
  signed bytes**; a duplicate is either admitted idempotently or rejected as
  already anchored, and both outcomes are safe.
- never build and sign a *second, different* receipt for the same task while
  the first may still land: two receipts for one `task_id` is exactly what
  rules (i)/(j) exist to reject, and the loser is wasted work at best (E14).

---

## 12. Retry semantics

One rule per boundary, because they have different idempotency:

| Boundary | Retry class |
|---|---|
| control-plane request (discover, renew, report) | **safe retry** — requests are idempotent under `lease_id`/`attempt_id`; a repeated report is a no-op |
| worker command from the control plane | **safe under the same lease**; a command citing an expired or foreign lease is refused as stale |
| fetch-input capability | **requires a fresh capability** — a consumed one-time capability is never reopened (F §6, §11, E10); a *transient* data-plane error before consumption may be retried within the window |
| worker execution | **requires a new attempt** — new `attempt_id`, new lease, fresh capability |
| result upload | **safe retry within the put window** of the same attempt (F §10); after the window, a new attempt |
| receipt submission | **lookup first, then resubmit identical bytes** (§11.2); never sign a second receipt |
| anything after `COMPLETED` observed | **must not retry** |

---

## 13. Cancellation, expiry and ineligibility

RFC 0005 defines **no task expiry and no cancellation** (§2.9: "The first
envelope has no deadline"). A task, once registered, is open until a receipt
is anchored, forever. This contract invents no protocol cancellation.

What the control plane may do is stop *coordinating*: when a task becomes
ineligible under its policy — the client withdrew, a confirmation-depth
policy was not met, quota, or an operator decision — it:

- stops issuing new leases for the task (E16);
- revokes leases it issued and lets renewals fail;
- asks the issuer to revoke unconsumed data-plane capabilities (F §5.2);
- refuses new "execution started" reports;
- treats a running attempt by explicit policy — allow to finish, or signal
  stop — and records which;
- does not relay receipt material it now considers stale.

**Irreversible work still happens.** Distributed cancellation is not
instantaneous: a worker mid-execution may finish, persist a result and — if it
submits its own receipt — anchor it. Nothing off-chain can prevent an
executor with the key from anchoring a valid receipt for an open task. That is
the protocol working as designed; the control plane records the outcome and
does not pretend it could have stopped it.

---

## 14. Confidential-compute extension point

Unchanged from the parent architecture and F §9: **exactly one point moves.**
In the confidential profile, step 4 of §7 gains a precondition — the worker
environment produces attestation evidence, a verifier (control plane or
client) checks it against policy, and only then is the content key released
to that environment. The lease, the attempt, the session, the discovery, the
lifecycle, the receipt path: unchanged. `ComputeTask` and `task_id`:
untouched (E21).

Where attestation evidence flows in that future: worker → control plane (or
client) → verification → key-release decision → data plane. **Not** to
validators. Consensus parses no attestation and no vendor evidence (E22), and
no vendor is a protocol concept (E28). Until then, worker profile claims in
§5.1 remain claims.

---

## 15. Partitions, stale state and control-plane recovery

| Situation | Safe behaviour |
|---|---|
| worker reaches the data plane but not the control plane | it may consume a capability it already holds only if its lease is unexpired by the worker's own clock; it may not renew, so it should not start a long execution; anything after `not_after` is stale |
| worker reaches the control plane but not the chain | the worker relies on the control plane's chain observations for admission but must not *anchor* blind; receipt submission waits for a reachable node |
| control plane sees stale chain state | leases derived from stale state are re-validated against the chain before any capability is obtained; a task found completed on re-validation is released |
| task eligibility changes while a lease is live | §13 applies; the lease is revoked or allowed to run by explicit policy |
| control plane restarts and forgets local leases | leases it cannot recover are treated as **expired** at restart; every worker's next heartbeat for an unknown lease is refused as stale, and the worker reacquires; consumed capabilities stay consumed (the data plane remembers, not the control plane) |

**Recovery principle.** After any loss, reconstruct from the sources that are
authoritative for each fact (§16): the chain for tasks and receipts, the data
plane for objects and consumption, the worker for its own execution. Nothing
is assumed atomic across those three.

---

## 16. What is authoritative, and what the control plane must persist

| State | Authority | Control-plane role |
|---|---|---|
| task exists, `executor`, `input_commitment` | chain | **derived** — cache, re-derivable by block scan |
| task completed (receipt anchored) | chain | **derived** — cache |
| object exists, consumed, deleted | data plane (F §12) | observed |
| capability issued / consumed | issuer and data plane | **durable coordination state** — which capability was issued under which lease and attempt, so that a retry can be reasoned about; never the capability's proof material |
| lease, `attempt_id`, `last_seen` | control plane | **durable coordination state** — losing it costs a restart round-trip (§15), not correctness |
| worker session | control plane | cache — re-established by re-authentication |
| result ready | data plane (durability) reported by worker | observed; the data plane is the authority on whether a result exists |
| receipt submitted, tx hash, observed height | chain for the fact; control plane for the bookkeeping | **durable coordination state** — the tx hash and height enable §11.2 lookup without resubmission |

A control-plane database is never protocol truth (E5). Everything chain-
derived in it can be wrong and must be re-checked before it authorises
anything.

---

## 17. Threat model

### 17.1 Compromised or malicious control plane

| It cannot… | Why |
|---|---|
| change `task.executor` or make another executor valid | the envelope is on-chain and immutable (RFC 0005 §8); rule (s) is consensus (E17) |
| forge executor proof of possession | it never holds the executor key (§11.1, §2) |
| change `input_commitment` or rewrite the task | on-chain, immutable (E18) |
| make validators accept an invalid receipt | validation is consensus (RFC 0002 rules a–j, RFC 0005 q–s) |
| retrieve arbitrary private objects | it is not the data plane; capabilities are bound to the executor, and it does not hold executor keys (F §5); if it is the delegated issuer it can *issue* capabilities to the named executor, not *use* them |
| anchor a receipt on an executor's behalf | it can only relay bytes the executor signed (§11.1) |

| It can still… | Classification |
|---|---|
| withhold work, delay, deny service | accepted — availability |
| issue or revoke capabilities within its delegation, for the named executor | accepted — bounded by F's binding; the blast radius is the executor's own tasks |
| misreport operational status, attempt duplicate scheduling | mitigated by leases and by the chain and data plane refusing duplicates (§10) |
| observe task metadata, timing and which executor serves whom | accepted — metadata privacy is out of scope (parent §13) |

### 17.2 Malicious worker under a legitimate executor

| It can… | Classification |
|---|---|
| read and exfiltrate plaintext input and output | **accepted** in the ordinary profile (E20; parent §7); **future capability** to constrain via confidential execution |
| fabricate a result and anchor a receipt for it | **accepted at this layer** — the receipt proves attribution, not correctness (RFC 0005 §9.2, E19); verification is future work ([#52](https://github.com/MbongoChain/mbongo-chain/issues/52)) |
| fail to execute | accepted — the task stays open; the client may submit a new task to another executor |
| replay old commands | prevented — commands are bound to a live lease and attempt |
| replay a data capability | prevented by F — consumption and proof of possession |
| publish duplicate results | prevented — the data plane refuses a second put; the chain refuses a second receipt |
| lie about progress | accepted — progress is informational; only durability and anchoring are facts |
| retain plaintext after execution | accepted risk in the ordinary profile; the destruction obligation is stated (F §14) but not enforceable from outside |

Three guarantees kept apart: **privacy** — none from the provider in the
ordinary profile; **correctness** — none from the receipt; **availability** —
best-effort coordination, no exactly-once.

---

## 18. Observability

F §13 applies unchanged. In addition, for the control plane:

| Item | In logs by default? |
|---|---|
| `task_id`, executor identity | yes — public on-chain |
| worker instance / session identifier, `attempt_id`, `lease_id` | yes — operational identifiers that grant nothing |
| state transitions, failure classes | yes |
| capability identifier | yes; the capability itself, its proof material or any challenge signature — **never** |
| resource identifier | yes; locator — **no** by default (F §13) |
| raw private payload, content-encryption keys, result content | **never** |
| signed receipt bytes relayed under §11.1 | may be retained as coordination state (they are destined for a public chain); the executor key never exists to be logged |

---

## 19. Operations, described without transport

The contract consists of these conceptual operations. None is a REST path, a
gRPC method, a queue, a topic or a frame; those belong to an implementation
gate.

| Operation | Direction | Bound to |
|---|---|---|
| establish session (prove executor possession) | worker → control plane | executor |
| discover work | worker → control plane, or control plane → worker | session |
| acquire lease | control plane → worker | task, executor, instance, attempt |
| renew lease (heartbeat) | worker → control plane | lease |
| release lease | worker → control plane | lease |
| request private-input authorization | worker → control plane → issuer, per F | lease |
| report execution started | worker → control plane | lease, attempt |
| report result ready | worker → control plane | lease, attempt, commitment |
| submit or relay receipt material | worker → chain, or worker → control plane → chain | signed bytes only |
| report failure (with class) | worker → control plane | lease, attempt |
| observe completion | control plane ← chain | `task_id` |

---

## 20. Minimum contract for the first worker

| Question | Answer |
|---|---|
| How does a worker learn a task exists? | the control plane observed it in a block via `get_block_by_height` and offers it; or the worker scans blocks itself (§4) |
| How does it know the task is for its executor? | `task.executor` in the block equals the `Address` whose key it holds (§4, §6) |
| How does it authenticate to the control plane? | proof of possession of the executor key over a fresh challenge; a session bound to that executor and this instance (§5) |
| How does it acquire permission to execute? | an execution lease for `(task_id, executor, instance, attempt_id)` with `not_after` (§3, §7) |
| How does it obtain the F capability? | under the lease, from the issuer per F §5 — bound to task, executor and object; single-use (§7) |
| How does it know lease and capability are still valid? | lease: unexpired by `not_after` and renewable by heartbeat; capability: unexpired, unrevoked, unconsumed — the data plane is the authority (§9.2, F §5.2) |
| How does it report execution start? | an authenticated report under the lease, after commitment verification succeeds (§7 step 6) |
| How does it handle crash and retry? | a new attempt: new `attempt_id`, new lease, fresh capability; a consumed capability is never reopened; a durable result from a prior attempt is reused, not recomputed (§9.3, §12) |
| How does it persist a private result? | put-result capability under the lease; durability confirmed by the data plane before anything else (§11, F §10) |
| When may receipt handling begin? | only after result durability is confirmed and `output_commitment` computed (§11) |
| How does it report terminal failure? | a failure report with a class (input, execution, persistence, receipt) under the lease; the lease is released; the task remains open on-chain (§13, §19) |
| Which state is authoritative at each step? | chain for task and completion; data plane for objects, consumption and result existence; control plane for leases and attempts; worker for its own execution (§16) |

---

## 21. Invariants

| # | Invariant | Status |
|---|---|---|
| E1 | the control plane does not choose an executor different from `task.executor` | ALREADY_TRUE (RFC 0005 rule s); restated §1 |
| E2 | changing executor requires existing protocol semantics, not off-chain reassignment | ALREADY_TRUE (RFC 0005 §2.6); §1 |
| E3 | a worker must prove authority associated with `task.executor` | DEFINED_BY_THIS_GATE (§2, §5) — reusing F's proof of possession |
| E4 | task discovery does not create task authority | DEFINED_BY_THIS_GATE (§4) |
| E5 | control-plane observed state is not consensus state | DEFINED_BY_THIS_GATE (§9.1, §16) |
| E6 | an execution lease is distinct from a data-plane capability | DEFINED_BY_THIS_GATE (§3) |
| E7 | an execution lease is distinct from task authority | DEFINED_BY_THIS_GATE (§3) |
| E8 | stale execution authorization can expire or be revoked operationally | DEFINED_BY_THIS_GATE (§9.2, §13) |
| E9 | attempt identity does not alter `task_id` | DEFINED_BY_THIS_GATE (§8); ALREADY_TRUE that nothing off-chain can |
| E10 | retries do not silently reopen consumed one-time capabilities | ALREADY_TRUE (F §11); restated §7, §12 |
| E11 | worker crash after input consumption requires fresh data authorization | ALREADY_TRUE (F §11); §9.3 |
| E12 | duplicate execution is handled without claiming exactly-once | DEFINED_BY_THIS_GATE (§10) |
| E13 | result persistence precedes receipt anchoring | ALREADY_TRUE (F §10); §11 |
| E14 | receipt submission retry has an idempotency/lookup rule | DEFINED_BY_THIS_GATE (§11.2), on chain facts that are ALREADY_TRUE (rules i/j; `submit_transaction` semantics) |
| E15 | worker/control-plane restart recovery is defined | DEFINED_BY_THIS_GATE (§15) |
| E16 | task ineligibility stops new execution admission | DEFINED_BY_THIS_GATE (§13) |
| E17 | control-plane compromise cannot authorize another executor | ALREADY_TRUE (rule s) + DEFINED_BY_THIS_GATE (§2, §17.1) |
| E18 | control-plane compromise cannot rewrite accepted task commitments | ALREADY_TRUE (RFC 0005 §8) |
| E19 | worker compromise does not imply consensus correctness | ALREADY_TRUE (RFC 0005 §9.2) |
| E20 | ordinary execution may expose plaintext to the provider | ALREADY_TRUE (parent §7) |
| E21 | future confidential execution fits through an extension point | DEFINED_BY_THIS_GATE (§14); FUTURE_CAPABILITY as behaviour |
| E22 | validators do not parse worker/vendor attestation evidence today | ALREADY_TRUE |
| E23 | logs contain no payloads, keys or reusable capability secrets by default | DEFINED_BY_THIS_GATE (§18), inheriting F §13 |
| E24 | `task_id` alone is not worker execution authorization | DEFINED_BY_THIS_GATE (§4, §6); F3 for data access |
| E25 | the chain is not a scheduler | ALREADY_TRUE (`VISION_v1.md` §2; parent P14) |
| E26 | the control plane is not a private object store | ALREADY_TRUE (parent §2); §17.1 |
| E27 | the worker is not a consensus authority | ALREADY_TRUE (RFC 0005 §6) |
| E28 | no vendor is protocol authority | ALREADY_TRUE |
| E29 | no new RPC version is silently invented | ALREADY_TRUE — this contract uses only `submit_transaction` and `get_block_by_height` |
| E30 | no new consensus requirement is introduced | ALREADY_TRUE — none |

Every DEFINED_BY_THIS_GATE item is also REQUIRES_IMPLEMENTATION. Conflicts
with existing authority: **none.**

---

## 22. Unresolved by this gate, deliberately

- **Confirmation depth** before a task is treated as admissible — operational
  policy; the protocol defines no finality depth and this contract sets no
  number.
- **Lease duration and heartbeat interval** — deployment parameters.
- **Which submission style** (§11.1) the first deployment uses.
- **Transport and encoding** for every operation in §19 — an implementation
  gate.
- **Task discovery by block scan versus a maintained index** — an
  implementation choice; neither is protocol.

None blocks the first reference worker; none touches consensus.

---

## 23. Relationship to other authority

- **RFC 0005** remains normative for the envelope, `task_id`, executor
  authorisation, binding rules and activation. This document is the
  coordination around its step 5 and steps 9–10, and adds nothing to it. If
  any sentence here conflicts with RFC 0005, RFC 0005 wins.
- **RFC 0002 and `RECEIPT_SPEC_v0.1`** remain normative for the receipt and
  its validation; §11 uses them and redefines nothing.
- **F** (`compute-private-data-plane-interface.md`) remains authority for
  every capability, fetch, result and deletion rule; this document composes
  leases with F's capabilities and changes none of them.
- **The parent architecture** remains authority for the four planes; this
  document is its §22 item 5.
- **`rpc_v0.2.md`** is unchanged; the only methods this contract relies on are
  `submit_transaction` and `get_block_by_height`. The payload-shape follow-up
  recorded in the parent's §23 is unaffected.

---

## See also

- [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md) — parent architecture
- [`compute-private-data-plane-interface.md`](compute-private-data-plane-interface.md) — data-plane handoff contract (F)
- [RFC 0005 — Compute Task Commitment](../rfcs/0005-compute-task-commitment-v1.md) — normative, Accepted
- [RFC 0002 — Receipt Anchoring](../rfcs/0002-receipt-anchoring-v0.3.md) — normative
- [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md) — receipt structure
- [`compute-receipts.md`](compute-receipts.md) — what the chain does with receipts today
- [#126](https://github.com/MbongoChain/mbongo-chain/issues/126) — the Compute vertical epic (this is Workstream E)
- [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) — verification research (future)
