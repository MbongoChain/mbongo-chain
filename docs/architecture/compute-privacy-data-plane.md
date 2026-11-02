# Compute privacy and the private data plane

> **Document type:** Architecture — prescriptive boundary for off-chain systems
> **Status:** Architectural authority for how off-chain Compute components
> handle customer and model data. Defines no consensus rule, no RPC method and
> no SDK type; where anything here appears to conflict with a normative source
> below, the normative source wins.
> **Normative sources:** [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md)
> (Accepted), [RFC 0002](../rfcs/0002-receipt-anchoring-v0.3.md),
> [`PROTOCOL_LOCK_v0.3.md`](../specs/PROTOCOL_LOCK_v0.3.md) (FROZEN),
> [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md),
> [`rpc_v0.2.md`](../specs/rpc_v0.2.md) (FROZEN),
> [`VISION_v1.md`](../VISION_v1.md)
> **Descriptive companion:** [`compute-receipts.md`](compute-receipts.md) —
> what the chain does today with receipts

This document answers one question:

> **Where may customer and model data exist throughout the Mbongo Compute
> lifecycle, and which subsystem is responsible for protecting it?**

It exists because RFC 0005 is now Accepted and implementation of `ComputeTask`
may begin, and because the first reference worker would otherwise define
these boundaries by accident. Nothing below is implemented. The chain-side
facts are drawn from the accepted protocol; the off-chain planes are
constraints that any implementation of them must satisfy.

---

## 1. The short version

- The blockchain **commits** to private data. It does not **carry** it.
- No plaintext prompt, document, CV, dataset, model input, model output,
  private model weight or encryption key is required on-chain by any accepted
  protocol field. That is a property of the protocol as designed. It is **not**
  a promise that the protocol prevents anyone from placing such bytes in an
  opaque public field — see §4.
- Keeping data off-chain keeps it away from the **chain**. It does not keep it
  away from the **compute provider**. Ordinary execution needs plaintext in
  the provider's memory. Confidentiality *from the provider* is a separate,
  stronger property that requires confidential execution (§10).
- The chain is not a GPU scheduler and not a marketplace
  ([`VISION_v1.md`](../VISION_v1.md) §2). Everything that discovers, matches,
  negotiates or transfers lives off-chain.
- A receipt proves who claimed what, over which committed input, with whose
  authority. It does not prove the output is correct. Nothing in this document
  changes that.

---

## 2. The four-plane model

| Plane | Responsibility | Trust and visibility |
|---|---|---|
| **1. Public chain** | Task commitment, executor authorisation, receipt anchoring, settlement. Consensus-validated, replicated to every node, effectively permanent. | Everything here is public to every observer, forever. |
| **2. Compute control plane** | Provider discovery, capability and policy matching, negotiation, endpoint discovery, attestation verification and key-release policy. Off-chain. Holds descriptive and policy data, not workload content. | Sees who is asking for what kind of work, from whom; should not need workload content. |
| **3. Private data plane** | Raw inputs, raw outputs, private model weights, encrypted workload packages, keys, temporary execution artifacts. Off-chain. Owns confidentiality, retention and deletion. | Confidential to the client and to the parties the client explicitly admits. |
| **4. Execution environment (worker)** | Receives an authorised workload through plane 3, executes, produces the result, constructs and signs the `Receipt`, anchors it through plane 1. | Ordinary execution sees plaintext. Confidential execution constrains what the operator can see (§10). |

Planes 2 and 3 are deliberately separate. A control plane that also transports
workload content becomes a party to every customer's data; a data plane that
also performs discovery becomes a scheduler. Keeping them apart is what lets
each be trusted for exactly one thing.

---

## 3. Public chain boundary

Every field below is public, replicated and permanent once included in a
block, and readable by anyone through `get_block_by_height`
([`rpc_v0.2.md`](../specs/rpc_v0.2.md) §2.6). The population is exactly the
accepted protocol's: the v0.3 transaction and receipt
([`PROTOCOL_LOCK_v0.3.md`](../specs/PROTOCOL_LOCK_v0.3.md) §1–2) and the
RFC 0005 task envelope (RFC 0005 §2.1).

| Field | Type | Class | What it reveals |
|---|---|---|---|
| `Transaction.sender` / `receiver` | `Address` | identity | who submitted; zero address for tasks and anchors |
| `Transaction.amount` / `nonce` | `u128` / `u64` | settlement | value moved (0 for tasks and anchors); ordering |
| `Transaction.signature` | 64 bytes | authorisation evidence | that the sender signed |
| `ComputeTask.submitter` | `Address` | identity | who asked (must equal `sender`) |
| `ComputeTask.executor` | `Address` | identity | who is authorised to answer |
| `ComputeTask.salt` | 32 bytes | task identity input | nothing by itself; distinguishes repeated tasks |
| `ComputeTask.input_commitment` | 32 bytes | commitment | a hash of the input — see §12 for what a hash does and does not hide |
| `ComputeTask.execution_spec` | opaque, ≤ 1024 bytes | **bounded public execution description** | whatever the client put there — see §4 |
| `task_id` (derived) | 32 bytes | task identity | links task and receipt |
| `Receipt.input_commitment` / `output_commitment` | 32 bytes each | commitment | hashes; equality with the task is what consensus checks |
| `Receipt.executor` | `Address` | identity | who answered |
| `Receipt.metadata` | opaque, ≤ 4096 bytes | **bounded public application data** | whatever the executor put there — see §4 |
| `Receipt.signature` | 64 bytes | attribution evidence | that the executor signed |

**The blockchain is not customer data storage.** No field above is a prompt,
a document, a dataset, a model input, a model output, a model weight or a key,
and no consensus rule requires any of those to be present. The two opaque
fields are the only places arbitrary bytes *can* go, and both are bounded
precisely so that the chain is a poor place to put data (RFC 0005 §2.10).

Two claims must not be confused:

- **NOT REQUIRED ON-CHAIN** — true of every category of customer data. This is
  what the protocol design guarantees.
- **IMPOSSIBLE TO PLACE ON-CHAIN** — false. A client can write 1024 bytes of
  anything into `execution_spec`; an executor can write 4096 bytes of anything
  into `metadata`. Consensus commits to those bytes and never interprets them.

The first is a protocol property. The second would require content filtering
in consensus, which this document does not propose and which would not work.

---

## 4. Public data warning

Every byte deliberately placed in a blockchain field must be treated as:

- **PUBLIC** — readable by anyone with RPC access, with no access control;
- **REPLICATED** — held by every full node;
- **EFFECTIVELY PERMANENT** — there is no deletion, and a later protocol
  version cannot retract what earlier blocks contain.

This applies with particular force to the two opaque fields.

**`Receipt.metadata`** is, by its normative definition, "opaque, never
interpreted by the protocol" ([`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md)
§2), capped at 4096 bytes by RFC 0002 §3. Its intended use is an
application-layer pointer or commitment ([`compute-receipts.md`](compute-receipts.md)
§1). It is **not a private-data transport.**

**`ComputeTask.execution_spec`** is "opaque, bounded description of what was
requested" (RFC 0005 §2.1), capped at 1024 bytes, which the RFC sized
deliberately so that "anything larger belongs off-chain behind
`input_commitment`" (RFC 0005 §2.10). It is **not a private-data transport.**

Neither statement changes either field's consensus semantics. This is an
architecture and application invariant (P4, P5 in §16), enforced by the
systems that construct tasks and receipts, not a new rejection rule.

---

## 5. Private data plane

The private data plane holds every artifact whose disclosure would harm the
client or the model owner:

- raw prompts and completions
- documents, CVs, records, datasets
- raw model inputs and raw model outputs
- private or proprietary model weights, where the model is not open
- encrypted workload packages and their wrapping keys
- session keys, key-release tokens, credentials
- temporary execution artifacts: scratch files, caches, logs that could
  contain any of the above

Its lifecycle, stated without choosing a technology:

```
client
  │  package: canonicalise input, compute commitment(s), encrypt for the
  │  intended execution environment
  ▼
protected transfer            (authenticated, encrypted transport to the
  │                            endpoint the control plane resolved)
  ▼
execution environment         (plaintext exists here for ordinary execution;
  │                            see §7 and §10)
  ▼
protected result return       (encrypted to the client; output commitment
  │                            computed over the plaintext result)
  ▼
retention / deletion          (policy-driven; the only plane where deletion
                               is possible at all)
```

The plane is implementation-neutral by design. No object store, queue,
database, cloud vendor or transport is prescribed here; any of them may
satisfy the constraints in §6, and none is a protocol concern.

---

## 6. Data states

Protection is not one property. The three states below are protected by
different mechanisms, and each mechanism has a limit that must be stated.

| State | Mechanism | What it achieves | What it does not achieve |
|---|---|---|---|
| **At rest** | encryption of stored artifacts; storage and retention policy | protects data on disk from parties without the key; makes deletion meaningful | nothing about who holds the key |
| **In transit** | authenticated, encrypted transport between client, control plane and worker | protects data from network observers and from endpoints other than the intended one | nothing about what the intended endpoint does with it |
| **In use** | ordinary execution: none. Confidential execution: hardware-isolated memory plus attestation (§10) | ordinary: none. Confidential: constrains the host operator's access | absolute isolation — see the limits in §10 |

**Encryption at rest plus encrypted transport does not provide
confidentiality from the compute provider during ordinary execution.** The
model must read the plaintext to run. For an ordinary GPU provider that
plaintext is in memory the operator controls, and no amount of encryption
before or after changes that.

---

## 7. Ordinary provider privacy

This is the baseline, and it must be understood before any stronger profile is
discussed.

For ordinary compute:

- the client's data **can remain entirely off-chain** — the accepted protocol
  needs only commitments;
- **but** the compute provider's execution environment **can observe
  plaintext input and output** during execution, and nothing in the protocol
  or in this architecture prevents that.

Therefore:

> **OFF-CHAIN ≠ PRIVATE FROM THE PROVIDER.**

A client who needs confidentiality only from the chain and from the public is
served by ordinary compute. A client who needs confidentiality from the
provider is not, and must use the confidential profile (§10, §14) once it
exists — or trust the provider by other means.

---

## 8. Compute control plane

The control plane is off-chain. It answers *which provider, on what terms*,
and then gets out of the way. Its conceptual responsibilities:

| Function | Notes |
|---|---|
| provider discovery | who exists and is reachable |
| provider identity | an executor key is an identity in the protocol sense; a real-world identity is a control-plane attribute |
| capabilities and available models | which workloads a provider can run |
| hardware capabilities | GPU class, memory, throughput |
| pricing and latency | commercial terms; none of this is consensus |
| reputation | history, derived off-chain, possibly from on-chain receipts |
| privacy and attestation capabilities | whether a provider can offer confidential execution, and with what evidence |
| jurisdiction and residency attributes | asserted or certified; see §19 for what these can and cannot prove |
| retention and egress policy | what the provider promises to keep, delete and not send onward |
| verification strength | what independent checking, if any, a workload receives |
| job negotiation | agreeing terms for a specific task before or after it is committed |
| private endpoint discovery | where the encrypted workload is to be sent |

The blockchain does none of this. `VISION_v1.md` §2 rules it out
("not a cloud GPU network", "not an AI marketplace"), and RFC 0005 §6 keeps
assignment and discovery off-protocol. Transient scheduling state does not
belong in consensus merely because Compute settles on a blockchain.

---

## 9. Execution environment and the worker boundary

A worker:

1. learns of an authorised task — from the control plane, or by observing the
   chain for tasks naming its executor key;
2. **receives the workload through the private data plane**;
3. executes off-chain;
4. produces the result and returns it through the private data plane;
5. constructs a `Receipt` carrying the task's `task_id` and
   `input_commitment` and its own `output_commitment`, signs it over the raw
   receipt hash, and anchors it through Mbongo Chain (RFC 0005 §10).

**A worker must not obtain customer input from blockchain payload fields.**
Neither `execution_spec` nor `metadata` nor any commitment is an input
channel. A worker that reads its input from the chain has been handed a
design in which the client published that input, and this architecture
exists to make that design unreachable by default (P15).

Worker language, runtime and packaging are not specified here; no existing
authority requires a particular one. RFC 0005 §6 describes the reference
worker as "an external process with no consensus role", and that is the whole
of its protocol status.

---

## 10. Confidential compute

Confidential compute is an **optional, stronger execution profile** for
workloads that require confidentiality from the provider. It is not required
for ordinary Compute to function, and it is not required before the first
reference worker (§22).

The architectural flow:

```
execution environment produces remote attestation evidence
        ↓
control plane (or the client) verifies the evidence against the expected
environment: measured code, configuration, hardware class
        ↓
policy authorises conditional key release to that environment only
        ↓
protected execution: workload decrypted and run inside hardware-isolated memory
        ↓
encrypted output returned to the client
        ↓
ephemeral key and data destruction at the end of the session
```

What this changes: the host operator no longer has ordinary access to the
plaintext in use. What it does not change: everything else in this document.
The chain still sees only commitments; the data plane still carries the
content; the receipt still proves nothing about correctness.

**Limits that must be stated rather than hidden.** Confidential execution is
not absolute confidentiality. Its guarantees depend on:

- the hardware and firmware being free of exploitable defects;
- side channels — timing, power, memory access patterns — being outside the
  attacker's reach or budget;
- correct configuration of the environment, including what it logs and where;
- the integration boundary with the host: drivers, accelerators, I/O paths;
- the attestation infrastructure — certificate chains, verification services —
  not being compromised.

**Vendor neutrality.** AMD SEV-SNP, Intel TDX, NVIDIA confidential computing
and technologies not yet released are *examples* of environments that can
produce attestation evidence. None of them is named by the protocol, none is
a protocol constant, and choosing among them is a control-plane and
deployment decision (P13).

---

## 11. Attestation boundary

Five things that are often conflated must stay separate:

| Step | Who | Where |
|---|---|---|
| attestation **generation** | the execution environment | plane 4 |
| attestation **verification** | the control plane, or the client directly | plane 2 |
| **policy evaluation** — is this environment acceptable for this workload? | the client's policy, evaluated by the client or a control-plane service it trusts | plane 2 / client |
| **key release** | the client or its delegated key service, conditional on the two steps above | plane 3 |
| optional **on-chain reference** | a commitment to attestation evidence, if a later protocol version justifies it | plane 1 — **not today** |

**Consensus validators do not parse, verify or interpret vendor-specific
attestation evidence.** Nothing in RFC 0005 or the protocol lock represents
attestation, and this document finds no requirement that it should. If a
future design concludes that an attestation commitment must be
consensus-validated, that is a normative protocol decision and requires its
own RFC under [`RFC_PROCESS.md`](../RFC_PROCESS.md). This document does not
make it.

---

## 12. Commitments and privacy

### 12.1 What RFC 0005 says, exactly

RFC 0005 §2.4 states that consensus checks **equality** between a receipt's
`input_commitment` and the committed task's, "does not, and cannot, check how
either was derived", and then gives a **non-normative interoperability
convention**:

```
input_commitment  = BLAKE3( DOMAIN_INPUT  || input_bytes )
output_commitment = BLAKE3( DOMAIN_OUTPUT || output_bytes )
```

That convention is unsalted and deterministic: the same input always yields
the same commitment. That is the right property for *interoperability* — two
parties can agree on what a commitment means — and the wrong property for
*privacy* of sensitive inputs.

### 12.2 What a hash does not hide

- **Hash ≠ encryption.** A commitment is one-way, not reversible; it is also
  not secret. BLAKE3 is sound; the issue is what is being hashed, not the
  function.
- **Equality leaks linkage.** Identical input → identical commitment, on a
  public chain, forever. Two clients who commit the same document are
  linkable. One client who resubmits the same document is linkable across
  time. Neither needs the plaintext.
- **Low-entropy inputs are guessable.** If the input space is small or
  predictable — a yes/no answer, a form drawn from a known template, a record
  built from public fields — an observer can enumerate candidates, hash each,
  and match. Preimage resistance does not help when the preimage is
  guessable.

### 12.3 Task salt is not commitment blinding

RFC 0005's `salt` is an input to `task_id` (RFC 0005 §2.2). It lets a client
repeat the same computation under a distinct task identity (§2.6). It **does
not enter `input_commitment`** and therefore does not hide commitment
equality. A task with a fresh salt and an unchanged input has a fresh
`task_id` and the same public `input_commitment`.

### 12.4 Non-normative guidance for sensitive inputs

For inputs whose equality or guessability matters, a client should commit to
the input under **client-controlled randomness**, in a domain of its own:

```
commitment = HASH( domain_for_blinded_commitment
                   || private_randomness
                   || canonical_input )
```

where `private_randomness` is fresh per task, generated by the client, and
kept in the private data plane alongside the input so that the executor can
recompute the same commitment. The exact domain string and byte layout are
deliberately not fixed here; an application that adopts this pattern should
version its own layout, exactly as RFC 0005 §2.12 asks applications to do for
`execution_spec`.

### 12.5 Compatibility with RFC 0005

Because consensus checks only that the receipt's `input_commitment` **equals**
the task's, a blinded commitment is **fully compatible** with the accepted
protocol: the client commits the blinded value in the task, hands the input
and the randomness to the executor through the private data plane, and the
executor writes the same blinded value into the receipt. No consensus rule,
no codec and no RFC 0005 semantic changes. The interoperability convention
remains the default for inputs that are neither sensitive nor guessable.

This section does not create a second normative protocol. It records that the
accepted protocol already permits the privacy-preserving choice, and says when
to make it.

---

## 13. Metadata privacy is a separate problem

Even with no plaintext customer data on-chain, an observer of the chain can
learn:

- which `submitter` asked, and which `executor` answered;
- when, and how often — timing and frequency of tasks and anchors;
- task relationships — the same submitter–executor pair over time, tasks
  sharing an `input_commitment`;
- transaction values and, if introduced later, fees;
- whatever the client chose to make legible in `execution_spec`, and whatever
  the executor chose to make legible in `metadata`.

This is **traffic and metadata privacy**, and it is a different property from
content confidentiality. This architecture does not promise it, and no
current or accepted protocol mechanism provides it. It is recorded as a
future concern (P12), not solved.

---

## 14. Payment privacy is a separate problem

**Private compute data does not imply private payments.** Settlement on
Mbongo Chain is ordinary public transaction activity. If a workload's content
is confidential but its settlement is a public transfer between two visible
accounts, the economic activity remains visible. No payment-privacy mechanism
is designed or implied here (P17).

---

## 15. Retention and deletion

| Where | Retention | Deletion |
|---|---|---|
| **On-chain** | effectively permanent | impossible |
| **Private data plane** | policy-driven: expiration, retention windows, ephemeral execution | possible, and the only place it is |

Therefore any customer data that carries a deletion obligation, a retention
limit or a right to erasure **must not be placed on-chain**, and the
obligation attaches to the off-chain copies: the client's package, the
worker's working set, caches, logs and the result artifact (P10).

Model weights follow the same split. A **public or open-weight model** may be
cached and stored by a provider freely; that is a capacity question. A
**private or proprietary model** is confidential workload data: its weights
are a private-data-plane asset (P18), an ordinary untrusted provider that
loads them can read them, and protecting them requires the same confidential
profile that protects customer inputs. Confidential compute therefore
protects customer data, model-owner intellectual property, or both. Mbongo is
model-agnostic: no model, open or proprietary, is named by the protocol.

---

## 16. Architectural invariants

Eighteen invariants, each classified against the system as it exists on
`dev` today. "Already true" means the accepted protocol already has the
property; "compatible but unimplemented" means nothing prevents it and
nothing yet provides it.

| # | Invariant | Status today |
|---|---|---|
| P1 | No plaintext customer workload data is required on-chain. | ALREADY_TRUE |
| P2 | The chain commits to private data; it does not transport it. | ALREADY_TRUE |
| P3 | Raw inputs and raw outputs belong to the private data plane. | REQUIRES_OFFCHAIN_ARCHITECTURE |
| P4 | `Receipt.metadata` is public and is not a private-data transport. | ALREADY_TRUE as a property; COMPATIBLE_BUT_UNIMPLEMENTED as an enforced application invariant |
| P5 | `ComputeTask.execution_spec` is public and is not a private-data transport. | ALREADY_TRUE as a property; COMPATIBLE_BUT_UNIMPLEMENTED as an enforced application invariant |
| P6 | Settlement must not require revealing workload content. | ALREADY_TRUE |
| P7 | Provider selection may be constrained by privacy policy. | REQUIRES_OFFCHAIN_ARCHITECTURE |
| P8 | Ordinary provider execution does not protect plaintext from the provider. | ALREADY_TRUE (a limitation, stated) |
| P9 | Confidential execution requires successful attestation before sensitive key release. | FUTURE_CAPABILITY |
| P10 | Retention and deletion apply to off-chain data, never to blockchain history. | ALREADY_TRUE |
| P11 | Jurisdiction and residency are control-plane policy, unless future protocol authority explicitly changes that. | COMPATIBLE_BUT_UNIMPLEMENTED |
| P12 | Public metadata is itself a privacy surface. | ALREADY_TRUE (unacknowledged until now) |
| P13 | Confidential-compute integration is vendor-neutral at protocol level. | ALREADY_TRUE (nothing vendor-specific exists) |
| P14 | The blockchain does not schedule or route GPU workloads. | ALREADY_TRUE (`VISION_v1.md` §2, RFC 0005 §6) |
| P15 | Worker input is never sourced from public blockchain payload fields. | COMPATIBLE_BUT_UNIMPLEMENTED (no worker exists) |
| P16 | Receipt binding does not prove output correctness. | ALREADY_TRUE (RFC 0005 §9.2) |
| P17 | Private payment activity is a separate problem from private workload content. | ALREADY_TRUE |
| P18 | Private and proprietary model weights are private-data-plane assets. | REQUIRES_OFFCHAIN_ARCHITECTURE |

Conflicts with the current design: **none.**

---

## 17. Policy dimensions and product profiles

### 17.1 Dimensions

Implementation-neutral dimensions a control plane may match on. None of them
is a consensus field, and none should become one without a normative
decision under `RFC_PROCESS.md`.

| Dimension | Concern |
|---|---|
| privacy level required | CONTROL_PLANE (matching) + PRIVATE_DATA_PLANE (enforcement) — MIXED |
| provider identity and reputation | CONTROL_PLANE |
| hardware capability | CONTROL_PLANE |
| attestation capability | CONTROL_PLANE (verification) + EXECUTION_ENVIRONMENT (generation) — MIXED |
| jurisdiction and residency | CONTROL_PLANE |
| retention | PRIVATE_DATA_PLANE (enforcement) + CONTROL_PLANE (policy) — MIXED |
| egress and network policy | EXECUTION_ENVIRONMENT + CONTROL_PLANE — MIXED |
| verification strength | CONTROL_PLANE today; may acquire a CHAIN component only through a future verification RFC |
| price | CONTROL_PLANE |
| latency | CONTROL_PLANE |

### 17.2 Profiles

Profiles are **policy compositions** — product abstractions layered over the
dimensions above. They are **not consensus transaction types**, and none
exists operationally today.

| Profile | Meaning |
|---|---|
| **PUBLIC** | ordinary provider; content off-chain; no confidentiality from the provider is claimed |
| **VERIFIED** | PUBLIC plus stronger provider identity, reputation or independent verification of results |
| **CONFIDENTIAL** | attested protected execution with conditional key release; confidentiality from the provider within the limits of §10 |
| **SOVEREIGN** | CONFIDENTIAL or VERIFIED plus provider, jurisdiction and residency constraints certified off-chain (§19) |

---

## 18. Verifiability versus confidentiality

Stronger verification and stronger confidentiality pull in opposite
directions, and the architecture must say so rather than promise both.

- **Redundant execution** improves confidence in a result by having several
  providers compute it — and hands the plaintext to each of them.
- **Fraud proofs** may require disclosing input, output or intermediate state
  to a challenger or to the chain.
- **Attestation-based verification** can reduce provider exposure by
  constraining the environment rather than repeating the work, at the cost of
  hardware trust assumptions (§10).
- **Zero-knowledge approaches** may offer verification without disclosure and
  are future work, tracked as research in
  [#52](https://github.com/MbongoChain/mbongo-chain/issues/52).

Consequently, verification strength and privacy level are not independent
dimensions: a client choosing one constrains the other, and a future
verification design must state which privacy class each mechanism requires.
None of these mechanisms is designed here.

**Output correctness.** Today's receipt, bound by RFC 0005 to a committed
task, establishes *task correspondence*, *input correspondence*, *executor
authorisation* and *executor attribution* — and, in the RFC's own words, not
*output correctness* (RFC 0005 §9.2). Nothing in this architecture upgrades
that claim, and no privacy mechanism described here should be read as doing
so (P16).

---

## 19. Jurisdiction and sovereign compute

Provider jurisdiction is **not a consensus property** and is not represented
anywhere in the accepted protocol. An IP address is not proof of physical
location. A provider's assertion is an assertion.

Sovereign compute — a requirement that work happen in a named region under
named constraints — would rest on off-chain provider certification and
control-plane policy, potentially combined with attestation evidence that
binds an environment to certified hardware. Mbongo does not currently prove
residency, and this document adds no jurisdiction field to consensus (P11).

---

## 20. End-to-end flows

### A. Ordinary compute

```
client
  ├─ canonicalise input; compute commitment (blinded if sensitive, §12.4)
  ├─ submit ComputeTask { submitter, executor, salt, input_commitment,
  │                       execution_spec }                       ── chain
  ├─ negotiate with the chosen provider                          ── control plane
  └─ transfer encrypted package to the provider's endpoint       ── data plane
provider / worker
  ├─ decrypt; plaintext exists in ordinary memory (§7)
  ├─ execute
  ├─ return encrypted result                                     ── data plane
  └─ build Receipt { task_id, input_commitment, output_commitment,
                     executor, metadata }; sign; anchor           ── chain
chain
  └─ validate (a)–(j) and (q)(r)(s); store receipt
```

### B. Confidential compute

```
client
  ├─ commitment; ComputeTask                                     ── chain
  ├─ select a provider advertising attestation capability         ── control plane
provider environment
  └─ produce attestation evidence                                 ── execution env
control plane / client
  ├─ verify evidence against expected environment
  └─ evaluate policy; release the workload key to that environment only
provider environment
  ├─ decrypt inside isolated memory; execute; encrypt output
  ├─ destroy session key and working set
  └─ Receipt; sign; anchor                                       ── chain
```

### C. Public chain observer

```
observer (any RPC client)
  sees:      submitter, executor, salt, commitments, execution_spec,
             task_id, receipt fields incl. metadata, amounts, nonces, timing
  never receives, from chain participation alone:
             raw input, raw output, model weights, keys
  may infer: linkage, frequency, relationships (§13)
```

---

## 21. Scenarios

| Scenario | Public chain data | Private data | Provider visibility | Profile | Not guaranteed |
|---|---|---|---|---|---|
| **Individual runs an open-weight model** on rented compute with a private document | task and receipt fields; a commitment to the document | the document, the output | provider reads the document during ordinary execution | PUBLIC | confidentiality from the provider; availability of any provider |
| **HR platform processes a CV** — "no plaintext CV on the public chain" | task and receipt fields; a *blinded* commitment (§12.4) so equal CVs are not linkable | the CV, the result | ordinary provider reads the CV; a CONFIDENTIAL provider does not, within §10 limits | CONFIDENTIAL for provider confidentiality; PUBLIC satisfies only the on-chain invariant | retention enforcement, residency, auditability beyond the receipt — all off-chain capabilities to be built |
| **Private model owner** offers inference on proprietary weights | task and receipt fields | the weights, the inputs, the outputs | ordinary provider can read the weights | CONFIDENTIAL | protection of weights from a provider that is not attested |
| **Regulated organisation** requires region, retention and audit constraints | task and receipt fields | everything else | depends on profile | SOVEREIGN | proof of physical location; any regulatory certification |

In every row the on-chain invariant — no plaintext customer data required on
the chain — holds **today**. Every other guarantee in the table is an
off-chain capability that does not yet exist.

---

## 22. Roadmap position

Two statements that are different and are both true:

- **Confidential compute is required for confidentiality from an untrusted
  provider.** Without it, an ordinary provider sees plaintext (§7).
- **Confidential compute is not required before the first reference
  worker.** The first vertical — accepted `ComputeTask`, off-chain execution,
  receipt anchoring — is complete and correct on the PUBLIC profile.

What must be true **before the first worker PR begins** — the entry contract:

1. RFC 0005 is Accepted (done — `docs/rfcs/0005-compute-task-commitment-v1.md`).
2. This architecture is merged and indexed.
3. The `ComputeTask` implementation and its release boundary
   (`PROTOCOL_LOCK_v0.4`, RPC version evolution, SDK wire types) are
   established to the extent the worker depends on them.
4. A minimal private-data-plane handoff contract is defined: how a worker
   receives an input package and returns a result, without reading either
   from the chain.
5. The control-plane / worker responsibility boundary is defined, even if the
   first control plane is a static configuration.
6. P4, P5 and P15 are stated as conformance requirements the worker is tested
   against, not left as intentions.
7. The receipt production and anchoring path (RFC 0005 §10) is understood
   end to end.

What is **not** required before the first worker: TEE or any confidential
execution, sovereign or residency constraints, payment privacy, zero-knowledge
or fraud-proof verification, a provider marketplace, and protection of
proprietary weights from an untrusted provider.

---

## 23. RPC follow-up

`rpc_v0.2.md` §4.1 pins the transaction `payload` as
`None | AnchorReceipt(Receipt)`. Implementing RFC 0005's `ComputeTask` payload
variant widens that result shape in `get_block_by_height`, and `rpc_v0.2.md`
§8 classifies a result-shape change as requiring a new RPC version. The
TypeScript SDK's wire types gain the same variant. Neither is a privacy
matter; both are recorded here so the release boundary in §22 item 3 is
complete, and both are tracked in the Compute implementation epic rather
than by editing the frozen RPC document from this architecture.

---

## 24. Compliance language

This architecture describes capabilities that professional users commonly
need: data residency constraints, access control, an auditable anchoring
trail, retention policy, confidential execution, and customer-controlled
encryption. It makes **no claim** that Mbongo, any provider, or any profile
is GDPR-, PIPEDA-, HIPAA- or SOC 2-compliant, or certified by any authority.
Such claims require evidence that does not exist in this repository, and
capability is not certification.

---

## 25. Non-goals

This document does not define or propose:

- GPU scheduling, routing or assignment in consensus
- storage of raw workload data on-chain
- vendor-specific TEE quote validation in consensus
- any proof of output correctness
- a zero-knowledge machine-learning implementation
- a fraud-proof implementation
- a provider marketplace implementation
- a payment-privacy mechanism
- tokenomics, fees, rewards or staking
- a model marketplace implementation
- any legal or regulatory compliance certification

---

## 26. Relationship to RFC 0005

[RFC 0005](../rfcs/0005-compute-task-commitment-v1.md) is and remains the
normative authority for the `ComputeTask` envelope, task identity, commitment
binding rules (q)(r)(s), executor authorisation, storage and activation
semantics. This document:

- **does not override** RFC 0005 in any respect;
- restates RFC 0005 facts only to locate them in the plane model, always with
  a section reference, never as a second definition;
- adds architectural constraints on the **off-chain** systems that consume
  the protocol, and non-normative guidance (§12.4) that the protocol already
  permits.

**If any sentence here conflicts with RFC 0005, RFC 0005 wins.** Changes to
consensus semantics discovered while implementing this architecture go
through [`RFC_PROCESS.md`](../RFC_PROCESS.md), not through an edit to this
file.

---

## See also

- [RFC 0005 — Compute Task Commitment](../rfcs/0005-compute-task-commitment-v1.md) — normative, Accepted
- [RFC 0002 — Receipt Anchoring](../rfcs/0002-receipt-anchoring-v0.3.md) — normative
- [`compute-receipts.md`](compute-receipts.md) — what the chain does with receipts today
- [`PROTOCOL_LOCK_v0.3.md`](../specs/PROTOCOL_LOCK_v0.3.md) — frozen surfaces
- [`rpc_v0.2.md`](../specs/rpc_v0.2.md) — the RPC surface (FROZEN)
- [`VISION_v1.md`](../VISION_v1.md) — what Mbongo is and is not
- [`ENGINEERING_EVIDENCE.md`](../ENGINEERING_EVIDENCE.md) — what counts as proof
- [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) — verification research (future)
- [#50](https://github.com/MbongoChain/mbongo-chain/issues/50) — developer tooling
