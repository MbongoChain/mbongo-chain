# S3: Transaction Service and Recovery Design

**Status: PROPOSED, design only.** Nothing here is implemented or authorized for
implementation. Stage S3 of [`MVP_v0.2_NETWORK_READINESS.md`](../product/MVP_v0.2_NETWORK_READINESS.md),
tracked in #207. The roadmap stays in #209, and this document holds no schedule.

**Covers:** forwarding to the producer, explicit failure, mempool bounds and eviction,
sync and recovery acceptance, operator-visible status.

**Not covered, still on #207:** peer limits, rate limits, release binaries, the operator
guide.

**Protected surfaces:** no change to P2P protocols, consensus rules, transaction encoding
or state transitions. Section 7 lists every surface touched.

## 1. Current behaviour

Read from the code at `dev` `23832d43`. Only the items marked *tested* have a test today.

| Fact | Where |
|---|---|
| `submit_transaction` has no producer check. A follower accepts a transaction, returns its hash and holds it forever | `backend.rs` `submit_transaction` |
| Only blocks are pushed to peers | `p2p.rs` |
| Admission reads committed state and the local mempool: exact expected nonce, balance less pending debits | `backend.rs` |
| Re-submitting a pending transaction, or an included plain one, returns its hash. An included `AnchorReceipt` or `ComputeTask` is refused | `backend.rs`, *tested* for pending |
| Per-sender cap 64. No global cap, no expiry, no eviction. `Mempool::remove` is unused | `backend.rs`, `mempool.rs` |
| A block takes the first 1000 entries in insertion order, and is all or nothing. A failed block leaves all of them pending, so the same block fails again | `peek_for_block`, `produce_block`, *tested* |
| An imported block does not clear the local mempool | `handle_incoming_block`, `main.rs` |
| The mempool is not persisted | `NodeBackend::new` |
| `GET /transactions/:hash` reads storage only: a pending transaction is 404, and `block_hash` and `block_height` are always null | `backend.rs` |
| Storage has no index from transaction to block | `mbongo-storage` |
| Bootnodes are dialed once, at startup. A closed connection is only logged. The orchestrator is not told | `main.rs`, `p2p.rs` |
| Each start creates a fresh P2P identity, so a `/p2p/<id>` bootnode address goes stale when that node restarts | `p2p.rs` |
| A failed outbound sync request leaves `in_flight` set (#146). PR #147 proposes the fix | `p2p.rs`, `main.rs` |
| `reqwest` is already a normal dependency of `mbongo-node` | `Cargo.toml` |

Consequence of the two rows on bootnodes: from the code, a follower should not rejoin a
restarted producer without being restarted itself. This has not been run. If it holds,
AC9 cannot pass until R3 is fixed.

## 2. Forwarding

**Transport: the existing JSON-RPC `submit_transaction`, over HTTP, from follower to
producer.** Gossip and a new P2P message would both need an RFC, and one producer needs
neither. Transaction gossip stays excluded.

```text
client ──RPC──> follower ──stateless checks──> POST /rpc ──> producer ──admission──> mempool
       <─ hash or explicit failure ─┘                                                   │
                                     follower <── block via existing P2P ── block <─────┘
```

**Configuration:** `--producer-rpc-url <url>`, `http` or `https`, with no credentials in
it. A node refuses to start with `--producer` and this flag together, or with a malformed
URL. A multi-authority schedule is out of scope: MVP v0.2 has one authority.

**A forwarding follower holds nothing.** It has no pending entry to expire, and the
producer's mempool stays the only one.

**A follower runs only checks that do not depend on state.** Its view lags the producer's.
If it applied nonce and balance rules, it would refuse a valid transaction, and a client
sending nonces `n` and `n+1` would be refused at `n+1`, since the follower has no pending
`n`.

| On a follower | Check |
|---|---|
| Runs | Type and form, field constraints, task envelope, receipt metadata size, signature |
| Runs, positive answer only | Already in storage. Included transactions never leave, so a lagging node can trust a yes, and answer as admission does today, without forwarding. A no proves nothing |
| Left to the producer | Nonce, balance, per-sender and global limits, task and receipt state |

Admission is split into a stateless function and a stateful one, in the same order as
today. Producer behaviour does not change.

**One hop.** The follower sends `x-mbongo-forwarded: 1`. A non-producer receiving it
refuses, so a wrong URL fails on first use rather than looping. The header has no effect
on a producer, and methods, params and results stay as they are.

**HTTP client:** connect timeout 2 s, total deadline 5 s (`--forward-timeout-ms`), no
redirects, proxy environment variables ignored, response read up to 64 KiB, at most 32
forwards in flight. Forwarding never holds the mempool guard. A batch is handled
sequentially, as today, so each entry gets its own result.

**Ordering:** a client chaining nonces waits for each result before sending the next.
Concurrent forwards may reach the producer out of order, and the later nonce is then
refused explicitly.

**Reachability:** the producer's RPC has to listen beyond loopback (`--rpc-host` exists).
That RPC has no authentication and includes `produce_block`, and its CORS policy allows
any origin. It stays on a private network, with access restricted to the followers by
firewall. See decision D3.

**After S1:** a follower compares `get_chain_info` with its own `chain_id` before its
first forward and refuses on a mismatch. The producer also refuses transactions signed
for another chain, because the signature no longer verifies.

## 3. Explicit failure

The error code stays `-32603`, and no failure is reported as success. Messages are not
contract, so the class is carried in the standard JSON-RPC `error.data`, as
`{"kind": "..."}`. Clients that ignore `data` see today's error shape. `BackendError` gets
a variant carrying the class.

| `kind` | When | Reached the producer? | Client action |
|---|---|---|---|
| `no_route` | Follower with no `--producer-rpc-url` | No | Operator fixes the configuration |
| `not_delivered` | Connection refused, DNS failure, or 32 forwards in flight | No | Retry |
| `outcome_unknown` | Timeout or reset after the request was sent | Unknown | Read the status, then resubmit the same bytes if it is not included |
| `rejected` | The producer answered with an error, passed through | Yes, refused | Fix the transaction |
| `misrouted` | The target is not a producer | No admission | Operator fixes the URL |
| `bad_response` | Not JSON-RPC, too large, a redirect, or a hash other than the one computed locally | Unknown | Same as `outcome_unknown` |

**No automatic retry.** Retrying is safe for a pending transaction, whose byte-identical
resubmission returns its hash. It is not uniform once included: an included `AnchorReceipt`
or `ComputeTask` is refused as a duplicate. That is why the client reads the status first.

**Behaviour change, stated plainly:** a follower with no route accepts today and will
refuse. Silent acceptance is the defect being removed, so no compatibility flag is
proposed.

## 4. Mempool bounds and eviction

All of this is node-local policy (`NON_PROTOCOL` in the Lock). Two nodes may differ without
any effect on consensus.

| Policy | Value |
|---|---|
| Global cap | 8192 entries, `--mempool-max-pending`, 1 to 65536 |
| Memory | Bounded by the cap: admission already limits entry size. A test asserts the worst case |
| Per-sender cap | 64, unchanged |
| Expiry | 3600 s from admission, `--mempool-ttl-secs`, on a monotonic clock |
| Sweep | After each committed block, and every 30 s |
| Deduplication | The four existing indexes, unchanged. No replacement of a pending nonce |

**When full, the new transaction is refused.** Nothing is displaced. With no fees there is
no fair priority, and displacing the oldest would let a flood remove honest transactions.
The cap check sits with the per-sender check, after the idempotent return, so a retry of a
pending transaction still gets its hash.

**Eviction removes a sender's suffix, never one entry.** Blocks follow insertion order. If
only nonce `n` were evicted and then resubmitted, it would sit after `n+1`, and every block
containing both would fail. Evicting `n` therefore evicts every pending entry of that sender
above `n`.

**Reasons:**

| Reason | Trigger |
|---|---|
| `included` | The transaction is in a committed block, produced or imported |
| `stale_nonce` | Its nonce is below the committed nonce |
| `expired` | Older than the TTL |
| `sender_chain` | A lower nonce of this sender was evicted |
| `block_failed` | See below |

**Block failure.** Today one transaction that passes admission and fails application stops
block production for as long as it stays pending. Proposed: when a produced block fails,
try a block holding only the oldest entry. If that succeeds, normal production resumes. If
it fails, evict that entry with reason `block_failed`, then continue. Every attempt goes
through the unchanged `apply_block`. This edits `produce_block`, a Tier 0 function, so it
is its own PR. It changes which transactions a producer proposes, not which blocks are
valid.

**Restart:** pending transactions are lost, as they are today. Persisting them is not
proposed. The status reports `unknown` and the client resubmits.

## 5. Sync and recovery

| ID | Case | Expected | Today |
|---|---|---|---|
| R1 | Outbound sync request fails | `in_flight` released, request retried | Stuck (#146); PR #147 proposes the fix |
| R2 | Follower restarts during sync | Resumes from committed height, same head | Expected to work, no test |
| R3 | Producer restarts | Followers reconnect with no operator action | Not expected to work, see section 1 |
| R4 | Follower restarts while the producer keeps producing | Catches up, same head | Covered in part by the devnet harness |
| R5 | Peer lost in the middle of a range | Next peer or redial, no duplicate apply | No test |
| R6 | Gap seen through a block announcement | Sync triggered, gap closed | Implemented, no test of failure paths |
| R7 | Peer serves a block that is refused | Block not applied, no stuck state, other peers usable | No test |
| R8 | Crash during block commit | Atomic batch: restart at the old or the new height | Storage batch is atomic; no crash test |
| R9 | All nodes restart | Same height, head hash and balances on every node | No test |

**Fix for R3:** redial configured bootnodes on `ConnectionClosed` and on failed dials, with
capped backoff, and tell the orchestrator that the peer is gone. Add `--node-key-file` for
a stable identity, in the same format as other seed files, so a bootnode address survives
a restart. Protocol identifiers and message shapes do not change.

PR #147 has been open since 2026-09-24 and its merge state is unknown, so it needs
re-reviewing against current `dev` before reuse.

## 6. Operator-visible status

**`GET /transactions/:hash`** keeps its fields and adds `status`. Lookup order: storage,
mempool, recent table.

| `status` | Meaning | Where |
|---|---|---|
| `included` | In a committed block on this node | All nodes |
| `pending` | In this node's mempool | Producer |
| `forwarded` | The producer accepted it, and this node has not yet seen the block | Follower |
| `forward_failed` | The last attempt through this node failed, with its `kind` | Follower |
| `evicted` | Removed, with its reason | Producer |

A hash found nowhere stays 404, meaning *unknown to this node*, not *never existed*.

**`block_hash` and `block_height`** come from a bounded in-memory map filled at commit, 16384
entries. No storage index is added, because that would be a schema change. They stay null
for older blocks and after a restart, and `status` stays `included`. A scan of recent
blocks was rejected: it makes each REST read cost hundreds of storage reads.

The recent table holds 4096 entries, oldest out first, and is lost on restart.

**`GET /status`**, new, read-only, JSON on the REST port: role, height, head hash, peer
count, mempool size and cap, evictions by reason, forwards by outcome, time of the last
successful forward. This is the "equivalent health data" of #207. Prometheus format is
not proposed.

## 7. Protected surfaces

| Surface | Effect |
|---|---|
| `apply_block`, validity rules, state transition | Unchanged |
| Transaction and block encoding, signing | Unchanged |
| P2P identifiers and message shapes | Unchanged. `p2p.rs` changes in behaviour only: failure events, redial, optional stable identity |
| Storage schema | Unchanged |
| RPC methods, params, results, codes | Unchanged. **Additions:** `error.data.kind`, the forwarding header, and refusal on an unrouted follower. Proposed as a short addendum to the RPC spec, see D2 |
| REST | The Lock does not name it. Additive field, one new route |
| `produce_block` | Tier 0 function, block selection only. No locked surface |
| Mempool policy, CLI flags, `main.rs` | Tier 1 or `NON_PROTOCOL` |

Every PR touching a Tier 0 file carries the "no locked surface" statement that
`CONTRIBUTION_TIERS.md` requires.

## 8. Acceptance matrix

Kinds: **U** unit, **I** integration on real nodes, **H** devnet harness. **S1** needs signed
blocks and is run at the end.

| ID | Case | Expected | Kind | PR | AC |
|---|---|---|---|---|---|
| F1 | Submit through a follower, producer up | Hash returned, included, same on all nodes | H | 3 | AC4 |
| F2 | Follower with no route | `no_route`, nothing retained | U | 3 | |
| F3 | Producer down | `not_delivered` within the deadline | I | 3 | |
| F4 | Producer accepts, response lost | `outcome_unknown`; after resubmission, one inclusion | I | 3 | |
| F5 | Producer refuses | `rejected`, its message passed through | I | 3 | |
| F6 | `n`, `n+1` in sequence through a lagging follower | Both admitted | I | 3 | AC4 |
| F7 | Bad signature at a follower | Refused, zero requests sent | U | 3 | |
| F8 | Transaction already in follower storage | Same answer as admission, zero requests sent | U | 3 | |
| F9 | Target is a follower | `misrouted`, one hop | I | 3 | |
| F10 | More than 32 concurrent forwards | Excess is `not_delivered` at once | U | 3 | |
| F11 | Wrong hash, non-JSON, oversized, redirect | `bad_response`, redirect not followed | U | 3 | |
| F12 | `--producer` with the URL, malformed URL | Refuses to start | U | 3 | |
| F13 | Producer on another chain | Refused before forwarding | I, S1 | 6 | AC7 |
| M1 | Global cap reached | New transaction refused, pool unchanged | U | 1 | |
| M2 | Retry of a pending hash, pool full | Hash returned | U | 1 | |
| M3 | Per-sender cap | Unchanged | U | 1 | |
| M4 | TTL passes | Entry and its suffix evicted, with reasons | U | 1 | |
| M5 | Evicted nonce resubmitted | Next block is valid | U | 1 | |
| M6 | Block imported | Included and stale entries removed | U | 1 | |
| M7 | Random operations | Indexes, order and counters stay consistent | U | 1 | |
| M8 | Largest transactions at the cap | Memory within the stated bound | U | 1 | |
| M9 | Restart | Pool empty, resubmission included | I | 1 | AC9 |
| M10 | Eviction | No change to storage or blocks | U | 1 | |
| M11 | One entry fails application | Evicted as `block_failed`, the rest included, production continues | U | 5 | |
| R1–R9 | Section 5 | As stated | I, H | 2, 6 | AC9 |
| R7s | Forged block from a peer | Refused, sync continues | I, S1 | 6 | AC7, AC9 |
| T1 | Each status in section 6 | Reachable and correct | U, I | 4 | |
| T2 | Unknown hash | 404 | U | 4 | |
| T3 | Restart | `included` stays, block fields may be null, nothing invented | I | 4 | |
| T4 | `GET /status` counters | Match the operations performed, with counts stated first | I | 4 | |
| G1 | Existing conformance, replay, convergence, compute harnesses | Pass | H | all | AC11 |

## 9. Implementation PRs

Each PR meets section 10 of the specification, in this order:

| # | PR | Files | Tier | Needs |
|---|---|---|---|---|
| 1 | **Mempool bounds and eviction** | `mempool.rs`, `submit_transaction`, `handle_incoming_block`, `main.rs` | 1 | Nothing from RFC 0006 |
| 2 | Sync recovery: #147 refreshed, redial, stable identity, R1–R6 | `p2p.rs`, `main.rs` | 0 file, no locked surface | Nothing from RFC 0006 |
| 3 | Forwarding and explicit failure, with the RPC addendum | `main.rs`, `backend.rs` admission, `server.rs`, `rpc.rs`, new `forwarder.rs` | 1, RPC addition | PR 1 first: bounds come before a reachable producer |
| 4 | Status and `/status` | `rest.rs`, `backend.rs` | 1 | PRs 1 and 3 |
| 5 | Block failure handling | `produce_block` | 0 function, no locked surface | PR 1 |
| 6 | Final acceptance on v0.5: F13, R7s, full R and AC runs | Harness, chain check | 1 | S0, S1 merged |

PR 1 comes first because it needs nothing else, and removes an unbounded resource before
forwarding exposes the producer.

**Overlap with S0 and S1:** S0 touches the transfer path only, so it does not meet these
PRs. S1 rewrites parts of `backend.rs` and `main.rs`, and every PR above that lands first
gives it a merge to resolve, or the reverse. The specification places S3 after S0, so PRs
1 to 5 starting earlier is decision D1.

## 10. Decisions requested

| ID | Decision | Proposal |
|---|---|---|
| D1 | May PRs 1 to 5 be implemented before S0 merges? They do not depend on it, while the specification orders S3 after it | Yes for PR 1 now, the rest at their own gate |
| D2 | RPC addendum for `error.data.kind`, the header and the unrouted refusal: is it non-conflicting under the Lock, or does it need more? | Addendum, reviewed before PR 3 |
| D3 | Producer RPC exposure, including `produce_block` | Firewall only for MVP v0.2, recorded as a limit. Rate limits stay on #207 |
| D4 | Refuse on a follower with no route, with no compatibility flag | Yes |
| D5 | Default values in section 4 | As stated, tunable by flag |

## 11. Limits

- Forwarding has one target. If the producer is down, transactions are refused, not queued.
- Nothing pending survives a restart.
- `outcome_unknown` stays unknown until the client reads the status.
- The JSON-RPC transport carries no authentication. Confidentiality and access control
  come from the private network, or from `https` when an operator sets it up.
- Section 1 is read from the code. R3 has not been run.
- No number in section 4 has been measured.
