# S3: Transaction Service and Recovery Design

**Status: PROPOSED, with decisions D1 to D5 recorded in section 10.** PR 1 is merged. PR 2a is
authorized for implementation. PR 2b is at preparation only, and has its own gate after 2a,
in section 12. Stage S3 of [`MVP_v0.2_NETWORK_READINESS.md`](../product/MVP_v0.2_NETWORK_READINESS.md),
tracked in #207. The roadmap stays in #209, and this document holds no schedule.

**Covers:** forwarding to the producer, explicit failure, mempool bounds and eviction,
sync and recovery acceptance, operator-visible status.

**Not covered, still on #207:** peer limits, rate limits, release binaries, the operator
guide.

**Protected surfaces:** no change to P2P protocols, consensus rules, transaction encoding
or state transitions. Section 7 lists every surface touched.

## 1. Current behaviour

Read from the code at `dev` `23832d43`. This is not a statement of test coverage.

| Fact | Where |
|---|---|
| `submit_transaction` has no producer check. A follower accepts a transaction, returns its hash and holds it forever | `backend.rs` `submit_transaction` |
| Only blocks are pushed to peers | `p2p.rs` |
| Admission reads committed state and the local mempool: exact expected nonce, balance less pending debits | `backend.rs` |
| Re-submitting a pending transaction, or an included plain one, returns its hash. An included `AnchorReceipt` or `ComputeTask` is refused | `backend.rs` |
| Per-sender cap 64. No global cap, no expiry, no eviction. `Mempool::remove` is unused | `backend.rs`, `mempool.rs` |
| A block takes the first 1000 entries in insertion order, and is all or nothing. A failed block leaves all of them pending, so the next attempt selects the same entries | `peek_for_block`, `produce_block` |
| An imported block does not clear the local mempool. Pushed blocks go through `handle_incoming_block`, while synced blocks call `apply_block` directly | `backend.rs`, `main.rs` |
| A pending receipt may depend on another sender's pending task | `backend.rs`, rules (q)–(s) |
| `apply_block` errors about a transaction carry its index. `Storage`, `BadParent`, `BadHeight` and `TransactionsRootMismatch` do not | `ApplyBlockError` |
| The orchestrator holds one peer, and asks it again straight after a refused block | `main.rs` |
| The mempool is not persisted | `NodeBackend::new` |
| `GET /transactions/:hash` reads storage only: a pending transaction is 404, and `block_hash` and `block_height` are always null | `backend.rs` |
| Storage has no index from transaction to block | `mbongo-storage` |
| Bootnodes are dialed once, at startup. A closed connection is only logged. The orchestrator is not told | `main.rs`, `p2p.rs` |
| Each start creates a fresh P2P identity, so a bootnode address with `/p2p/<id>` goes stale when that node restarts. The Docker devnet's addresses carry no `/p2p/` | `p2p.rs`, `docker-compose.yml` |
| mDNS is on by default and auto-dials what it finds | `main.rs`, `p2p.rs` |
| A failed outbound sync request leaves `in_flight` set (#146). PR #147, open and unmerged, proposes the fix | `p2p.rs`, `main.rs` |
| `reqwest` is already a normal dependency of `mbongo-node` | `Cargo.toml` |

Consequence for separate hosts, where mDNS does not help: from the code, a follower
should not rejoin a restarted producer without being restarted itself. This has not been
run. If it holds, AC9 cannot pass there until R3 is fixed.

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
| Runs | Type and form, field constraints, task envelope, signature, receipt rules (e) to (h) |
| Runs, positive answer only | Already in storage. Included transactions never leave, so a lagging node can trust a yes, and answer as admission does today, without forwarding. A no proves nothing |
| Left to the producer | Nonce, balance, per-sender and global limits, task and receipt state |

These checks form a new function used only on a follower. **The producer's admission path
keeps its code and its order,** so which error wins there does not change. A follower may
report a different first error than a producer would for a transaction with several
faults. After S1, the signature check also needs the configured `chain_id`.

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
any origin. **It stays on a private network, with access restricted to the followers and
the operator by firewall (D3).** Ordinary clients use a follower. A follower forwards
`submit_transaction` only, and its own `produce_block` is refused.

**After S1:** a follower compares `get_chain_info` with its own `chain_id` before its
first forward and refuses on a mismatch. The producer also refuses transactions signed
for another chain, because the signature no longer verifies.

## 3. Explicit failure

The error code stays `-32603`, and no failure is reported as success. RFC 0006 section 7
sets that rule for `rpc_v0.4`: a hash means the producer admitted the transaction or it is
committed. Messages are not contract, so the class below is carried in the standard
JSON-RPC `error.data`, as `{"kind": "..."}`. Clients that ignore `data` see today's error
shape. `BackendError` gets a variant carrying the class. The `kind` values and the
forwarding header are not in RFC 0006. They are additive, and PR 3 specifies them as a
minor revision of `rpc_v0.4` under `RFC_PROCESS.md`.

| `kind` | When | Reached the producer? | Client action |
|---|---|---|---|
| `no_route` | Follower with no `--producer-rpc-url` | No | Operator fixes the configuration |
| `invalid` | Refused by the follower's own checks | No | Fix the transaction |
| `not_delivered` | Connection refused or timed out, DNS failure, or 32 forwards in flight | No | Retry |
| `outcome_unknown` | Timeout or reset after the request was sent | Unknown | Read the status, then resubmit the same bytes if it is not included |
| `rejected` | The producer refused the transaction | Yes, refused | Read the message. For `invalid nonce`, it names the expected nonce |
| `already_included` | The producer holds this exact transaction in storage, and refuses it as a duplicate | Yes, included earlier | None: it succeeded |
| `producer_error` | The producer failed, for example on storage | Yes, not admitted | Retry |
| `misrouted` | The target is not a producer | No admission | Operator fixes the URL |
| `bad_response` | Not JSON-RPC, too large, a redirect, or a hash other than the one computed locally | Unknown | Same as `outcome_unknown` |

The producer sets `kind` on its own errors, and the follower passes it through. A response
with no `kind` is treated as `rejected`.

**No automatic retry.** Retrying is safe for a pending transaction, whose byte-identical
resubmission returns its hash. It is not uniform once included: an included plain
transaction returns its hash, while an included `AnchorReceipt` or `ComputeTask` is refused
as a duplicate. A lagging follower cannot tell the client that it succeeded, so that case
returns `already_included` rather than `rejected`.

**Behaviour change, stated plainly:** a follower with no route accepts today and will
refuse. Silent acceptance is the defect being removed, so no compatibility flag is
proposed (D4). This is a change to observable RPC behaviour, so it ships in `rpc_v0.4` (D2).

## 4. Mempool bounds and eviction

All of this is node-local policy (`NON_PROTOCOL` in the Lock). Two nodes may differ without
any effect on consensus.

| Policy | Value |
|---|---|
| Global cap | 8192 entries, `--mempool-max-pending`, 1 to 65536 |
| Memory | 5362 bytes of heap per entry for the largest admissible transaction, in every index, measured at caps of 1024, 8192 and 65536: about 42 MiB at the default and 335 MiB at the maximum. M8 asserts at most 6 KiB per entry |
| Per-sender cap | 64, unchanged |
| Expiry | 3600 s from admission, `--mempool-ttl-secs`, on a monotonic clock |
| Sweep | After each committed block, and every 30 s |
| Deduplication | The four existing indexes, unchanged. No replacement of a pending nonce |

**When full, the new transaction is refused.** Nothing is displaced. With no fees there is
no fair priority, and displacing the oldest would let a flood remove honest transactions.
The cap check sits with the per-sender check, after the idempotent return, so a retry of a
pending transaction still gets its hash.

**Eviction removes what depends on the evicted entry, to a fixed point.** Blocks follow
insertion order. If only nonce `n` expired and was then resubmitted, it would sit after
`n+1`, and every block containing both would fail. So:

| Reason | Sender's entries above it | Receipt bound to it, if a `ComputeTask` |
|---|---|---|
| `included` | Stay | Stays: the task is on chain |
| `stale_nonce` | Stay: still valid | Goes, unless the task is registered |
| `superseded`, `expired` | Go | Goes, unless the task is registered |

Whatever goes as `dependent` is treated the same way, so a receipt's sender loses its entries
above the receipt, and a `ComputeTask` among those takes its own receipt. Otherwise the
receipt fails rule (q) in every block, and a resubmitted task would sit behind it.

**The hook revalidates a block's senders.** What stays must start at the committed nonce,
run without a gap, and be affordable. A block not built from this mempool can leave an entry
unable to apply: its task registered, its receipt anchored, its balance spent, or a gap
below it. Only the same sender can cause this, since nonces and debits are the
sender's own, and a task or receipt id binds its sender. Those entries go as `superseded`.

**Reasons:**

| Reason | Trigger |
|---|---|
| `included` | The transaction is in a committed block, produced or imported |
| `stale_nonce` | Its nonce is below the committed nonce |
| `superseded` | A committed block made it unable to apply |
| `expired` | Older than the TTL |
| `dependent` | Something it depends on was evicted |

**One post-commit hook.** A single function updates the mempool after any commit. It is
called on all three paths: produced, pushed and synced. PR 4 adds the block map and counters
to it.

**Block failure.** Today one transaction that passes admission and fails application stops
block production for as long as it stays pending. PR 1 removes the causes it can see, and
the TTL clears the rest. RFC 0006 section 3 has the producer leave out a transaction that
fails in its view, and serialises production. **That logic belongs to S1, and S3 does not
repeat it.** PR 5 only applies mempool policy to what S1's block builder left out. Between a
commit and its hook, production can still select an entry the block just included. In PR 1
that fails one block and the next round succeeds. S1's single writer closes it.

**Restart:** pending transactions are lost, as they are today. Persisting them is not
proposed. The status is 404, and the client resubmits.

## 5. Sync and recovery

| ID | Case | Expected | Today |
|---|---|---|---|
| R1 | Outbound sync request fails | `in_flight` released, request retried | Stuck (#146); PR #147 proposes the fix |
| R2 | Follower restarts during sync | Resumes from committed height, same head | Expected to work, no test |
| R3 | Producer restarts | Followers reconnect with no operator action | Fails, measured in section 12.1 |
| R4 | Follower restarts while the producer keeps producing | Catches up, same head | Covered in part by the devnet harness |
| R5 | Peer lost in the middle of a range | Another known peer or redial, no duplicate apply | Not handled: one peer is held |
| R6 | Gap seen through a block announcement | Sync triggered, gap closed | Implemented, no test of failure paths |
| R7 | Peer serves a block that is refused | Block not applied, backoff, another peer tried | Same peer is asked again at once |
| R8 | Process crash during block commit | Atomic batch: restart at the old or the new height. A host crash is not covered, and no power-loss durability is claimed. RFC 0006 section 3 adds a synced commit on the authority, which S1 implements and must evidence | Storage batch is atomic; no crash test |
| R9 | All nodes restart | Same height, head hash and balances on every node | No test |
| R10 | Provider restarts during a job | Job recovers as the durable store defines, and its receipt still anchors through the network | Provider restart tests exist, with no network |
| R11 | Peer sends malformed or impossible sync data | Ignored, no crash, peer treated as failed. Sync state is held per peer, so one answer cannot replace another's | No test |

**Fix for R3, R5 and R7:** redial configured bootnodes on `ConnectionClosed` and on failed
dials, with capped backoff, and tell the orchestrator that the peer is gone. The
orchestrator keeps the set of connected peers, and moves to another with backoff after a
failure or a refused block. `--node-key-file` gives a stable identity, as RFC 0006 section 3
requires, and is needed only for bootnode addresses with `/p2p/`. Protocol
identifiers and message shapes do not change. R3 is tested with `--no-mdns`, with and
without `/p2p/`.

PR #147 has been open and unmerged since 2026-09-24, so it needs re-reviewing against
current `dev` before reuse.

## 6. Operator-visible status

**`GET /transactions/:hash`** keeps its fields and adds `status`. Lookup order: storage,
mempool, recent table.

| `status` | Meaning | Where |
|---|---|---|
| `included` | In a committed block on this node | All nodes |
| `pending` | In this node's mempool | Producer |
| `forwarded` | The producer accepted it at `forwarded_at`. This says nothing about now: the producer may since have evicted it or restarted | Follower |
| `forward_failed` | No attempt through this node was accepted. The last one failed, with its `kind`. Not final for `outcome_unknown` | Follower |
| `evicted` | Removed, with its reason | Producer |

A hash found nowhere stays 404, meaning *unknown to this node*, not *never existed*.

Each status reports what this node knows, and only `included` is final. An accepted entry
is never overwritten by a later failed attempt. `forwarded` and `forward_failed` expire
after the mempool TTL plus 10 minutes, and become 404, so a follower cannot go on claiming
acceptance. The answer to *is it still pending?* comes from the producer.

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
| RPC (B19, frozen) | **Touched from PR 3, not by PR 1.** Methods, params, result shapes and codes stay. `error.data.kind` and the header are additive. Refusal on an unrouted follower changes what a request returns, so it ships in `rpc_v0.4` under RFC 0006 (D2). A full-pool refusal in PR 1 is `-32603`, the existing admission path |
| REST | The Lock does not name it. Additive field, one new route |
| `produce_block` | Tier 0 function. PR 1 replaces its mempool removal with the hook, and nothing else. No locked surface |
| Mempool policy, CLI flags, `main.rs` | Tier 1 or `NON_PROTOCOL` |
| `backend.rs` outside validity rules, `server.rs`, `rpc.rs` | Tier 0 files or crates, no locked surface apart from the RPC row above |

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
| F4 | Producer accepts a plain transaction, response lost | `outcome_unknown`; after resubmission, one inclusion | I | 3 | |
| F4b | Same for a `ComputeTask` and an `AnchorReceipt`, follower lagging | Resubmission gives `already_included`, never `rejected` | I | 3 | |
| F5 | Producer refuses | `rejected`, its message passed through | I | 3 | |
| F6 | `n`, `n+1` in sequence through a lagging follower | Both admitted | I | 3 | AC4 |
| F7 | Bad signature at a follower | `invalid`, zero requests sent | U | 3 | |
| F8 | Transaction already in follower storage | Same answer as admission, zero requests sent | U | 3 | |
| F9 | Target is a follower | `misrouted`, one hop | I | 3 | |
| F10 | More than 32 concurrent forwards | Excess is `not_delivered` at once | U | 3 | |
| F11 | Wrong hash, non-JSON, oversized, redirect | `bad_response`, redirect not followed | U | 3 | |
| F12 | `--producer` with the URL, malformed URL | Refuses to start | U | 3 | |
| F13 | Producer on another chain | Refused before forwarding | I, S1 | 6 | |
| F14 | Transaction signed for another chain, through a follower | Refused | I, S1 | 6 | AC7 |
| F15 | Header sent to a producer | No effect | U | 3 | |
| F16 | Batch with mixed outcomes | One correct result per entry | U | 3 | |
| F17 | Producer storage error | `producer_error`, not `rejected` | U | 3 | |
| M1 | Global cap reached | New transaction refused, pool unchanged | U | 1 | |
| M2 | Retry of a pending hash, pool full | Hash returned | U | 1 | |
| M3 | Per-sender cap | Unchanged | U | 1 | |
| M4 | TTL passes | Entry and its suffix evicted, with reasons | U | 1 | |
| M4b | Pending task expires with a receipt bound to it | Receipt and its sender's suffix evicted, next block valid | U | 1 | |
| M4c | Block included up to the middle of a chain, or a stale entry removed | Valid entries above it stay | U | 1 | |
| M4d | Dependants two hops away, across senders | All evicted, next block valid | U | 1 | |
| M4e | Stale `ComputeTask` with a bound receipt | Receipt goes unless the task is registered | U | 1 | |
| M4f | Task registered, receipt anchored, or balance spent by another transaction in an imported block | `superseded`, with its dependants, next block valid | U | 1 | |
| M5 | Evicted nonce resubmitted | Next block is valid | U | 1 | |
| M6 | Block produced, pushed and synced | Included and stale entries removed on all three paths, through the real orchestrator | U | 1 | |
| M7 | Generated operations over transfers, tasks, receipts and foreign blocks | Indexes agree, chains stay applicable, every eviction reason occurs | U | 1 | |
| M8 | Largest transactions at the cap | Memory within the stated bound | U | 1 | |
| M9 | Store closed and reopened | Committed state kept, nothing pending, resubmission included once. A separate process is covered by R2 and R9 | U | 1 | |
| M10 | Eviction | No change to storage or blocks | U | 1 | |
| M11 | S1's builder leaves an entry out | Evicted as `left_out`, a reason PR 5 adds, with its dependants, the rest included | U | 5 | |
| M12 | A produced block fails with no transaction to blame | Nothing evicted | U | 5 | |
| R1–R9 | Section 5 | As stated | I, H | 2, 6 | AC9 |
| R11 | Section 5 | As stated | U | 2 | AC9 |
| R3b | Bootnode unreachable for a long time | Backoff stays capped, reconnects when it returns | I | 2 | AC9 |
| R10 | Section 5 | As stated | H | 6 | AC9 |
| R7s | Forged block from a peer | Refused, sync continues | I, S1 | 6 | AC7, AC9 |
| T1 | Each status in section 6 | Reachable and correct | U, I | 4 | |
| T2 | Unknown hash | 404 | U | 4 | |
| T3 | Restart | `included` stays, block fields may be null, nothing invented | I | 4 | |
| T3b | Before a restart | `block_hash` and `block_height` are correct on all three commit paths | I | 4 | |
| T3c | Failed attempt after an accepted one, then expiry | `forwarded` stays, then 404 | U | 4 | |
| T3d | Tables beyond their bounds | Oldest out first, size stays at the bound | U | 4 | |
| T4 | `GET /status` counters | Match the operations performed, with counts stated first | I | 4 | |
| G1 | Existing conformance, replay, convergence, compute harnesses | Pass | H | all | AC11 |

## 9. Implementation PRs

Each PR meets section 10 of the specification, in this order:

| # | PR | Files | Tier | Needs |
|---|---|---|---|---|
| 1 | **Mempool bounds, eviction, post-commit hook** | `mempool.rs`, `main.rs`, and in `backend.rs` admission, `handle_incoming_block` and the removal in `produce_block` | Tier 0 file, no locked surface | Nothing from RFC 0006 |
| 2 | Sync recovery: #147 refreshed, redial, per-peer state and rotation, stable identity, R1–R7, R11. Split into 2a and 2b, section 12.7 | `p2p.rs`, `main.rs`, `handle_incoming_block` in `backend.rs` | Tier 0 file, no locked surface | Nothing from RFC 0006 |
| 3 | Forwarding and explicit failure, with the RPC spec revision | `main.rs`, `backend.rs` admission, `server.rs`, `rpc.rs`, new `forwarder.rs` | Tier 0 files, RPC (D2) | PR 1 and S1. It ships with `rpc_v0.4` (D2) |
| 4 | Status and `/status` | `rest.rs`, `backend.rs` | Tier 0 file, no locked surface | PRs 1, 2 and 3: peer count needs the P2P events from PR 2 |
| 5 | Eviction of what S1's builder leaves out. No block-building code | `mempool.rs`, hook | Tier 1 | PR 1, S1 |
| 6 | Final acceptance on v0.5: F13, F14, R7s, full R and AC runs | Harness, chain check | 1 | S0, S1 merged |

PR 1 comes first because it needs nothing else, and removes an unbounded resource before
forwarding exposes the producer.

**Overlap with S0 and S1:** S0 touches the transfer path only, so it does not meet these
PRs. S1 rewrites parts of `backend.rs` and `main.rs`, so whichever lands second has a merge
to resolve. RFC 0006 requires reconnection, which PR 2 provides, on the same release SHA.

## 10. Decisions

Maintainer decisions of 2026-11-05.

| ID | Decision | Effect here |
|---|---|---|
| D1 | PR 1 is authorized before S0 merges, on condition that no locked protocol surface changes. Other S3 PRs need separate approval | Section 9 |
| D2 | Non-additive RPC behaviour goes into the `rpc_v0.4` contract planned with S1. Version labels do not bypass frozen-contract governance | PR 3 follows S1. The rule is in RFC 0006 section 7 |
| D3 | The producer RPC stays private and firewalled. `produce_block` is not exposed to ordinary clients | Section 2 |
| D4 | A follower with no valid producer route fails explicitly. No silent local admission, no compatibility fallback | Section 3 |
| D5 | The configurable mempool limits are accepted provisionally, pending resource and invariant tests | Section 4, M7, M8 |

## 11. Limits

- Forwarding has one target. If the producer is down, transactions are refused, not queued.
- Nothing pending survives a restart.
- `outcome_unknown` stays unknown until the producer's status, or the block, settles it.
- The JSON-RPC transport carries no authentication. Confidentiality and access control
  come from the private network, or from `https` when an operator sets it up.
- Section 1 is read from the code. R3 was run on one setup, see section 12.1.
- Memory is measured on heap allocation by a counting allocator, for the transaction shape
  stated, not on a running node. Deadline and concurrency values in section 2 are not measured.

## 12. PR 2 design: sync recovery

**Status: PR 2a authorized for implementation, with no merge without review. PR 2b is
prepared, and needs a separate gate after 2a.**

### 12.1 What was measured

R3, run with real binaries on Windows loopback, `--no-mdns`, on PR 1's code. A follower
connected and synced, then the producer was restarted:

| Bootnode address | Before restart | 25 s after restart |
|---|---|---|
| With `/p2p/<id>` | Producer 16, follower 16 | Producer 58, follower 33, one connection, closed, never redialed |
| Without `/p2p/` | Producer 16, follower 16 | Producer 41, follower 16, one connection, closed, never redialed |

The follower stops where the connection closed, and stays there. **R3 is confirmed on this
setup.** It was not run across hosts, or on Linux.

### 12.2 Reconnection

`P2PNode` keeps the configured bootnodes, each with its own state. A bare address has no
peer id, and its dial failures report none, so state is keyed on the dial's `ConnectionId`,
then on the `PeerId` learned at connection, which is replaced if a later dial learns another.

| Rule | Value |
|---|---|
| Trigger | A synchronous `dial` error, which emits no event and is counted directly, `OutgoingConnectionError`, or `ConnectionClosed` with `num_established == 0` on a bootnode |
| Duplicates | A bootnode already connected, by its learned `PeerId`, is not dialed. A bare address ignores dial conditions, so this check is explicit |
| Backoff | 1 s, doubling, capped at 60 s, with 20 percent jitter |
| Limit | None: it retries for as long as the node runs |
| Reset | After a connection has stayed up for 30 s, so a flapping peer does not retry at full speed |
| Identity mismatch | A dial refused because the peer id differs is logged as such, and keeps backing off |

The backoff is a pure type taking the time as an argument, so tests need no sleeping.

### 12.3 Stable identity

`--node-key-file <path>`: a 32-byte seed as 64 hex characters, as RFC 0006 section 3
requires. Without it, a node has a fresh identity at each start, as today. Peer identity
is not a locked surface. On Unix a file readable by group or others is refused. It is not
the authority key: once S1 exists, the node refuses a file holding the same seed as
`--producer-key-file`.

Launchers that pin `/p2p/<id>` keep working for a restart only with this flag. Those that
use a bare address do not need it. `start-devnet.ps1`, `mvp-launch.mjs`,
`mvp-d-acceptance.mjs`, `devnet_harness` and `compute_vertical` pin it today, and stay as
they are unless their own restart is tested.

### 12.4 Peer failure

The orchestrator's single peer slot becomes per-peer state:

```rust
struct PeerSync {
    connected: bool,
    reported_height: u64,
    failures: u32,
    retry_at: Option<Instant>,
}
```

| Case | Behaviour |
|---|---|
| Height answer | Updates that peer only. It never lowers or replaces another peer's value |
| Block announcement | `block_rx` carries the sending `PeerId`, internally, and `handle_incoming_block` reports the result. A valid next block is applied. A height above that only triggers a height request to that peer: an unverified height is not a reported height |
| Choosing a peer | Among connected peers, the highest reported height with no backoff in force. Ties go to the lower `PeerId`, so the choice is deterministic |
| One request at a time | `in_flight` names the peer it is waiting for. An answer from any other peer does not clear it |
| Disconnect | New event `PeerDisconnected`, sent at `num_established == 0`. The entry stays with `connected: false`, keeping its backoff. Only an abnormal close counts as a failure. `in_flight` clears if it named that peer |
| Failed request | New event `RequestFailed`, for height and block requests alike. The peer gets a failure, `in_flight` clears, another is chosen, and a failed height request is retried after the backoff |
| Blocks in a batch at or below the local height | Skipped: a pushed block may have arrived first. Not a failure |
| Refused block, an empty `Blocks` answer, or `SyncResponse::Error` | Failure and backoff for that peer, 1 s doubling to 60 s, instead of asking it again at once |
| Malformed or impossible data | Failure for that peer. Range arithmetic saturates |
| No peer available | Waits on a timer set to the earliest `retry_at`, as a third branch of the orchestrator's `select!`, so a retry does not depend on another event |

`SyncEvent` and `block_rx` are internal, so these changes reach nothing on the wire.

### 12.5 PR #147

Reassessed against `dev`. It has one commit, by an outside contributor, on `p2p.rs` and
`main.rs`, based on `89cadb67`.

| Part | Decision |
|---|---|
| Tracking outbound block request ids, `SyncEvent::RequestFailed` | Extend to height requests, where a negotiation failure first appears |
| `request_next_batch` returning whether the command was accepted | Reuse |
| Its regression tests | Rewrite the two that pin block-only tracking and a re-request with no backoff. Reuse the rest |
| Clearing `in_flight` with nothing more | Extend: a failed request also backs that peer off and rotates |
| Hoisting `request_next_batch` to module level | Reuse, it makes testing simpler |

It merges with PR 1 with one conflict, in `main.rs`. Its author's commit is kept with its
authorship, and follow-up commits sit on top. #146 stays open until that lands. Whether to
update #147 itself or supersede it, with credit, is for the contributor and the maintainer.

### 12.6 Compatibility with RFC 0006

| RFC 0006 | Here |
|---|---|
| Section 3, reconnection | Implements it |
| Section 3, `--node-key-file` | Implements it |
| Section 3, caught-up rule | Not here. S1 builds it, inside `produce_block`. Per-peer state is local to the orchestrator task, so S1 adds a shared handle for the time of the first answer and of the last progress, pushed blocks included. PR 2a adds none: it would have no user yet |
| Section 7, chain-bound identifiers | Independent. A negotiation failure is a peer failure with backoff |
| Rollout | Needs this on the release SHA, by the decision placing the lock at G4 |

Nothing here needs RFC 0006 to be accepted, and nothing here changes a protocol identifier
or message shape. If RFC 0006 changes before acceptance, sections 12.2 and 12.3 follow it.

### 12.7 Implementation split

| PR | Content | Files | Tier |
|---|---|---|---|
| 2a | #147 refreshed, per-peer state, rotation, retry timer, saturating ranges, R1, R2, R4, R5, R6, R7, R11 | `p2p.rs`, `main.rs`, `handle_incoming_block` in `backend.rs` | Tier 0 file, no locked surface |
| 2b | Reconnection, `--node-key-file`, R3, R3b, a restart stage in `devnet_harness` | `p2p.rs`, `main.rs`, harness | Tier 0 file, no locked surface |

**Added cases:** R3c, reconnection with a bare address and with `--node-key-file`, across a
producer restart. R3d, a pinned address after an identity change logs the mismatch. R5b, an
answer from a peer not asked does not clear `in_flight`. R5c, a low height from one peer does
not stop a sync served by another. R5d, one peer whose request failed is retried with no
other event. R7b, a batch overlapping a pushed block is no failure. R7c, an empty `Blocks`
answer backs off.

### 12.8 PR 2b preparation

Scope: bounded backoff, bootnode redial, peer failure recovery, stable node identity.

| Test | Setup | Expected |
|---|---|---|
| Backoff | Pure type, time as an argument | 1, 2, 4 and so on to 60 s, within jitter. Reset after 30 s up |
| Redial trigger | Each trigger in 12.2 | One dial scheduled, none when already connected |
| Producer restart, bare address | `devnet_harness` stage, `--no-mdns` | Follower rejoins and reaches the producer's head with no operator action |
| Producer restart, `--node-key-file` and `/p2p/` | Same | Same, and the same `PeerId` after restart |
| Pinned address, no stable identity | Same | Mismatch logged, backoff stays capped, no hot loop |
| Producer down for longer than the cap | Same | Reconnects when it returns |
| Follower restart | Same | Rejoins and catches up |
| Key file | Unit | Malformed or too open a file refuses to start |

A follower is judged by height and head hash, read through RPC, after the restart, with the
expected count of nodes stated first.

**Gate for 2b:** PR 2a merged, its review closed, the values in 12.2 approved, and a decision on
whether the restart stage runs on pull requests, which costs CI time, or on push only.

**RFC 0006:** 2b implements reconnection and `--node-key-file` as section 3 states them. If
that text changes before acceptance, 12.2 and 12.3 follow it. The release needs 2b at G4.
