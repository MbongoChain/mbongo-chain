# P2P Resource Guard — Architecture Study

> **Status: study only. Non-normative. Nothing here is implemented.**
> Written 2026-11-14 against `dev` `232becb7`. Local, untracked file: not committed, not pushed, no PR.
> It changes no protocol surface, no RFC, no dependency and no MSRV.

> **Phase 1C note (2026-11-14, added when this file was committed).** The text below is the study as written, unchanged; the status lines above describe that moment.
> - **What happened since.** Phase 0 (feasibility) and Phase 1 (an observe-only guard on the real node) were carried out: see the [Phase 0](p2p-resource-guard-phase0-report.md), [Phase 1](p2p-resource-guard-phase1-report.md), [Phase 1B](p2p-resource-guard-phase1b-report.md) and [Phase 1C](p2p-resource-guard-phase1c-report.md) reports. No enforcement exists, and none is authorized by this study or by those reports.
> - **Evidence folders.** Paths under `C:\Dev\a2-work*` are on the maintainer's machine. They are not in this repository.
> - **Open items** are listed in the [P2P security backlog](p2p-security-backlog.md).

Every claim below is tagged where it matters:

- **[M]** measured in this repository's evidence (A2.0 – A2.3 trials, SHA-256 manifests under `C:\Dev\a2-work*`);
- **[S]** read in the pinned source, with file and line;
- **[U]** stated by an upstream page fetched for this study;
- **[E]** an engineering estimate or inference, not measured.

---

## 1. Executive summary

- **The problem has two layers and only one of them is covered.** Application work (GetHeight, GetBlocks, announcements) is now bounded by A2.3-b-1. Connection establishment is bounded before Noise by A2.3-a. **Stream establishment is not bounded at all**: a peer that opens streams fast still saturates 2 CPU cores [M].
- **The mitigations so far are sound but separate.** There are five independent admission points with five configurations and five sets of counters. Nothing ties a connection's streams to that connection, and nothing can act on a stream before protocol negotiation.
- **The reference model exists.** go-libp2p's Resource Manager defines nested scopes (system, transient, peer, connection, stream, protocol) with reservation and re-attribution [U]. It limits concurrent usage only: it has no rate limits [U], so it is a model for *accounting*, not a complete answer to this attack.
- **Recommended path, exactly one: Option A — a guarded `StreamMuxer` wrapper**, inserted with `SwarmBuilder::with_other_transport`, owned by a single `P2pResourceGuard` that also becomes the home of the existing limiters. Source reading supports its feasibility on every point asked [S]. **It has not been demonstrated by execution**, so the first step is a test-only feasibility gate, like the one that correctly stopped the separate-behaviour approach.
- **Not recommended: a dependency upgrade as the solution.** The changelogs of rust-libp2p after 0.53.2 mention no resource manager, stream admission or rate limiting [U], and releases from 0.57.0 need Rust 1.88 [U], above this repository's 1.85.

**Decision requested:** GO for Phase 0 only (test-only feasibility gate). No production change is proposed before it passes.

---

## 2. Baseline verified

| Item | Value | Source |
|---|---|---|
| `dev` | `232becb7e9bfb4270c9d2d82d87560794fdf0a87` | `git rev-parse origin/dev` |
| Working tree | clean (0 changes) before this file was written | `git status --porcelain` |
| MSRV | `rust-version = "1.85"` | `Cargo.toml:22` |
| libp2p | 0.53.2, features `tcp noise yamux kad mdns identify ping request-response tokio macros` | `Cargo.toml:41`, `Cargo.lock` |
| libp2p-swarm | 0.44.2 | `Cargo.lock` |
| libp2p-core | 0.41.3 | `Cargo.lock` |
| libp2p-yamux | 0.45.2 | `Cargo.lock` |
| yamux | 0.13.10 (used) and 0.12.1 (linked, unused by default) | `Cargo.lock`; `libp2p-yamux/src/lib.rs:226-230` [S] |
| libp2p-request-response | 0.26.3 | `Cargo.lock` |
| multistream-select | 0.13.0 | `Cargo.lock` |
| libp2p-connection-limits | 0.3.1 | `Cargo.lock` |

**Locked surfaces that this study must not touch** (`docs/specs/PROTOCOL_LOCK_v0.4.md` §9):

- negotiation strings `/mbongo-sync/2` and `/mbongo/block_notify/0.2.0`;
- message shapes;
- framing: `u32` LE length prefix, 16 MiB maximum frame;
- `MAX_RANGE = 256`.

Everything proposed here is local admission policy. A refusal is never a new validity rule.

---

## 3. Current network pipeline

```
 remote                         this node
   |
   |  TCP SYN ───────────────►  kernel accept
   |                            │
   |                    (G1)    ▼  handle_pending_inbound_connection
   |                            ├─ A2.3-a InboundRateLimit   source + global token buckets
   |                            ├─ A1 connection_limits      pending ≤ 32
   |                            └─ A1 IpLimits               ≤ 8 per IP
   |                            │   refusal here = socket dropped, no cryptography
   |  Noise XX ◄─────────────►  │  ~2.6 ms CPU per completed handshake [M]
   |  yamux upgrade             ▼
   |                    (G2)    connection established
   |                            └─ A1 connection_limits      ≤ 64 in, ≤ 16 out, ≤ 2 per peer
   |
   |  yamux SYN (new stream) ►  yamux: streams map, ≤ 512, else connection terminated
   |                            libp2p-yamux: inbound buffer ≤ 256, else stream dropped
   |                            │
   |          *** NO ADMISSION POINT HERE ***      ◄── the gap
   |                            ▼
   |  multistream-select ◄───►  swarm: ≤ 16 negotiating inbound streams, 10 s timeout
   |                            ▼
   |                            request-response handler: ≤ 4 sync, ≤ 2 notify per connection
   |                            │   over the cap: stream dropped AFTER negotiation (the counted WARN)
   |  frame bytes ───────────►  │
   |                    (G4)    ├─ A2.2b-lite FrameBudget     solicited 32 / unsolicited 48 / small 1 MiB
   |                            ▼  decode
   |                    (G5)    ├─ A2.3-b-1 RequestLimiter    peer, source, global work units
   |                            ├─ A1 Admission               per-peer quota, bounded queue
   |                            ▼
   |                            node: RocksDB reads, hashing, response encoding
```

**Where each existing control lives** (`crates/mbongo-network/src/`, `dev` `232becb7`):

| Gate | Control | Location |
|---|---|---|
| G1 | A2.3-a `InboundRateLimit` | `rate_limit.rs:330` (`handle_pending_inbound_connection`); defaults `rate_limit.rs:68-73` |
| G1/G2 | A1 connection limits | `limits.rs:48-62`; composed in `p2p.rs:176-190` |
| — | A1 negotiating streams | `limits.rs:70`, applied `p2p.rs:403` |
| — | A1 request/response stream caps | `limits.rs:73-75`, applied `p2p.rs:370`, `p2p.rs:378` |
| G4 | A2.2b-lite `FrameBudget` | `frame_budget.rs:33-45`; reserve-before-allocate at `p2p_protocol.rs:399` |
| G5 | A2.3-b-1 `RequestLimiter` | `request_rate.rs:357` (`try_admit`); defaults `request_rate.rs:140-144`; called from `p2p.rs:711` |
| G5 | A1 queues and quotas | `limits.rs:80-89`; `p2p.rs:729`, `p2p.rs:772` |
| — | Capacity-drop counter | `capacity_log.rs:32-42` |

**Upstream mechanics that define the gap** [S]:

- `libp2p-swarm/src/lib.rs:958`: the behaviours are asked about a pending inbound connection **before** `pool.add_incoming` (`lib.rs:986`) starts the Noise and yamux upgrade. This is why A2.3-a acts before cryptography.
- `libp2p-swarm/src/connection.rs:417-426`: an inbound stream is taken from the muxer (`poll_inbound_unpin`) only while fewer than `max_negotiating_inbound_streams` are negotiating; it goes straight into `StreamUpgrade::new_inbound` (`connection.rs:573`). There is no hook between the two.
- `libp2p-request-response/src/handler.rs:170`: a stream over the per-connection cap is dropped **after** negotiation.
- `libp2p-request-response/src/lib.rs:128-150`: events carry a `PeerId` but no connection id in 0.26.3.

---

## 4. Root-cause analysis from existing evidence

**Connection churn is solved at the right layer.** At an equal offered rate of 400 attempts/s from 4 IPv4 sources [M]:

| | Victim CPU | Handshakes completed |
|---|---|---|
| Before A2.3-a | 1.028 cores | 24,000 |
| With A2.3-a | 0.213 cores | 268 (23,732 refused before Noise) |

That gives about 2.6 ms per completed handshake and about 0.5 ms per refused attempt [E, derived from M]. In the profile, 58% of samples were Noise cryptography [M].

**Application work is solved at the right layer.** Under a flood of valid full-range `GetBlocks` [M]:

| | Before A2.3-b-1 | With A2.3-b-1 |
|---|---|---|
| Requests reaching the node in 60 s (4 IPs × 4 identities) | 77,277 | 1,938 |
| RocksDB share of CPU samples | 5.3% | 0.1% |
| BLAKE3 share | 0.9% | 0.0% |
| **Total victim CPU** | **2.00 cores** | **1.97 cores** |

**The remaining cost is stream establishment.** With application work removed, the profile of the same flood is [M]:

| Stage | Share of samples |
|---|---|
| Kernel (symbols unresolved) | 42.0% |
| Swarm and handler machinery | 8.6% |
| Runtime and timers | 6.6% |
| yamux | 6.5% |
| Allocator | 4.3% |
| Multistream negotiation and upgrade | 3.9% |
| Noise transport AEAD | 3.8% |
| Logging and tracing | 1.8% |
| Request processing (storage, hashing, decode, admission) | < 1% |
| Unclassified | 16.5% |

During these floods the node dropped 1.6 – 3.5 million streams per minute at the request-response cap, about 27,000 – 58,000 per second [M]. Dividing CPU by streams gives roughly **35 – 72 µs of victim CPU per attacker stream** [E]. What the kernel share consists of was not measured.

**Three approaches already tried and why they did not close the gap:**

| Approach | Result | Evidence |
|---|---|---|
| yamux concurrent-stream cap of 64 | Never triggers in the flood: the node releases refused streams faster than 64 accumulate. Against idle held streams it *raised* CPU tenfold (0.007 → 0.076 cores) and log volume fourteenfold by converting a cheap state into reconnect churn | [M], PR #261, closed |
| A separate `NetworkBehaviour` counting streams | Cannot count: `listen_protocol()` is also called once per poll pass (`connection.rs:434`), giving 7.3 – 7.7 calls per real stream; negotiated streams are routed only to the selected sub-handler (`handler/select.rs:85-94`, `:294-301`) | [S] + [M] probe |
| Post-decode request limits | Remove application work; cannot see a stream that never completes a request | [M], A2.3-b-1 |

**Root cause, stated once:** the first point where this node can refuse a stream today is *after* yamux accepted it, multistream-select negotiated it and the handler received it. Every earlier stage is paid for every attacker stream, and none of that cost is charged to anything.

**A lesson worth keeping** from the yamux-cap trial: closing a connection is only a mitigation when what it stops costs more than a reconnection. Idle held streams cost almost nothing (1,600 held streams: 0.007 cores, about 2 MiB [M]); closing for them is a net loss.

---

## 5. Reference architecture comparison

### 5.1 go-libp2p Resource Manager — documented upstream [U]

Source: `p2p/host/resource-manager/README.md`, read in full.

- **Scopes:** System, Transient, Service, Protocol, Peer, Connection, Stream, plus user transaction scopes. They form a DAG; usage is aggregated upward. System constrains everything.
- **Resources:** memory, file descriptors, inbound and outbound connections, inbound and outbound streams.
- **Transient scope:** resources "in the process of full establishment": a connection before its handshake belongs to no peer; a stream before protocol negotiation is constrained by it. The page calls it a DMZ.
- **Re-attribution:** `SetProtocol` adds the protocol constraint and removes the transient one; `SetService` adds a service constraint. Either can fail with "resource limit exceeded", a temporary error naming the rejecting scope.
- **Limits:** per-scope limit objects, scaled from base limits by memory and file descriptors; overrides per peer and per protocol; an allowlist with its own limits, to resist eclipse.
- **Observability:** Prometheus metrics including blocked resources by scope; nothing logged by default.
- **Stated limitations:** no prioritisation, "no notion of honest vs bad peers".
- **Rate limiting:** not mentioned. Only concurrent usage is limited.

### 5.2 libp2p DoS mitigation guide — documented upstream [U]

Source: `libp2p.io/docs/dos-mitigation/` (page commit dated 2026-01-29).

- Limit connections; limit *transient* (unidentified) connections in total.
- Limit **concurrent** streams per connection, not total streams. Brief sequential streams are described as fine.
- Reduce blast radius by splitting components into separate OS processes.
- Rate limiting inbound connections "can help"; mechanisms are named for go and js, not for Rust.
- Monitor, identify, block (firewall, fail2ban), allow and deny lists.
- For rust-libp2p it names connection keep-alive and "negotiating" connections. It does **not** mention a Rust resource manager, stream-opening rate limits or `max_negotiating_inbound_streams`.

### 5.3 What rust-libp2p 0.53.2 actually provides [S]

| Concept | Present in the pinned versions |
|---|---|
| Connection count limits | Yes: `libp2p-connection-limits` 0.3.1 (used by A1) |
| Pending-connection hook before handshake | Yes: `handle_pending_inbound_connection` (used by A2.3-a) |
| Negotiating-stream concurrency | Yes: `max_negotiating_inbound_streams` (used by A1) |
| Concurrent streams per connection | yamux `max_num_streams`, total in+out, terminates the connection |
| Per-protocol stream concurrency | Each protocol's own job (request-response `max_concurrent_streams`) |
| Scope hierarchy, reservations, transient scope | **No** |
| Stream admission before negotiation | **No** |
| Any rate limit | **No** |

### 5.4 Mapping

| go-libp2p scope [U] | MbongoChain today | Proposed (§6) |
|---|---|---|
| System | Scattered constants | `Global` scope |
| Transient (connections) | A1 pending ≤ 32 + A2.3-a | `Transient` scope, source-keyed |
| Transient (streams) | A1 negotiating ≤ 16 per connection, no global figure | `Transient` stream budget, global and per connection |
| Peer | A1 ≤ 2 connections; A2.3-b-1 peer buckets | `Peer` scope aggregating its connections |
| Connection | none as an object | `Connection` scope, owned by the guarded muxer |
| Stream | none as an object | `StreamPermit` (RAII) |
| Protocol | request-response caps; A2.3-b-1 kinds | `Protocol` scope, enforced at request admission |
| Allowlist | none | deferred to A2.3-c (pinned peers) |
| *(absent upstream)* rate limits | A2.3-a, A2.3-b-1 | rate buckets as first-class limits on scopes |

Everything in the right-hand column is **proposed MbongoChain design**, not upstream functionality.

---

## 6. Proposed Rust resource-management model

### 6.1 One guard, five gates

`P2pResourceGuard` is a single owner of scopes, limits and counters. The existing limiters are not rewritten: they become the policies evaluated at each gate, sharing one configuration object, one reservation vocabulary and one metrics surface.

```
                         P2pResourceGuard
   ┌───────────────────────────────────────────────────────────────┐
   │ Global        conns in/out · pending · streams · transient     │
   │               streams · stream-open rate · frame bytes · work  │
   │   │                                                            │
   │   ├── Transient   unauthenticated: keyed by SOURCE only        │
   │   │      └── Source(prefix)   conn-attempt rate · conns        │
   │   │                                                            │
   │   ├── Peer(id)    aggregate of its connections                 │
   │   │      └── Connection(id)  stream-open rate · concurrent     │
   │   │             │            inbound streams · abuse score     │
   │   │             └── StreamPermit (RAII)                        │
   │   │                                                            │
   │   └── Protocol(sync | notify | future)  concurrency · work     │
   └───────────────────────────────────────────────────────────────┘
        ▲ G1          ▲ G2            ▲ G3             ▲ G4   ▲ G5
     pending conn   established   stream opened    frame   request
     (A2.3-a, A1)   (A1)          (NEW)            bytes   (A2.3-b-1, A1)
```

### 6.2 Scopes and what each may hold

| Scope | Key | Limits | Notes |
|---|---|---|---|
| **Global** | — | active connections (in/out), pending connections, total active streams, transient streams, global stream-open rate, frame-byte budget, application work budget | Every other scope is also charged here |
| **Transient / Source** | IPv4 /32 or IPv6 /64 (configurable) | connection-attempt rate, connections per source | **Only** scope available before authentication. Never keyed by a claimed identity |
| **Peer** | `PeerId` (after Noise) | connections, aggregate stream-open rate, concurrent streams, sync work budget | Linked to the sources of its live connections; entries outlive disconnection so reconnecting refills nothing |
| **Connection** | `ConnectionId` | inbound stream-open rate, concurrent inbound streams, abuse score | Created when the muxer is wrapped; destroyed with it |
| **Protocol** | negotiation string | concurrency, work units | Enforced at G5, by peer (see 6.4) |

### 6.3 Identity rule

Before Noise completes, the peer identity does not exist. Therefore:

- G1 charges **Source and Global only**.
- At G2 the connection's reservation moves from Transient to `Peer(id)`; the Source link is kept, because one source may carry many identities.
- A peer with two connections is charged to both of its sources (the A2.3-b-1 rule), and its Peer scope sums both connections.

### 6.4 An explicit limitation: stream → protocol attribution

go-libp2p moves a stream from the transient scope to a protocol scope at `SetProtocol` [U]. In the pinned Rust versions this exact transfer is **not available to a muxer wrapper**: negotiation happens inside the swarm after the muxer hands the stream over, and request-response 0.26.3 events do not identify the connection [S].

The design therefore splits the idea in two, honestly:

- the **stream** stays charged to its Connection scope for its whole life (opened → dropped);
- the **protocol** budget is charged at G5, when a decoded request arrives with its peer — which is what A2.3-b-1 already does.

Per-protocol *concurrency per connection* stays where it is today, in request-response's own caps. A true transfer would need either a handler wrapper or the connection id that upstream added to request-response events in 0.28.0 [U].

### 6.5 Policy at the new gate G3

Per connection, when an inbound stream surfaces:

1. **Rate:** a token bucket of stream openings (experimental starting point: 32/s, burst 64). No token → the stream is refused.
2. **Concurrency:** a cap on admitted inbound streams alive at once. Over the cap → the stream is refused. **Never a closure**: idle streams are cheap [M].
3. **Global transient budget:** total streams admitted and not yet finished, across connections.
4. **Abuse score:** each rate refusal adds to a decaying score. Past a threshold — a grace allowance, so one burst does not close an honest connection — the guard makes **one** closure decision for that connection.
5. **After closure:** the reconnection goes through G1, where A2.3-a already allows about 1 attempt/s per source and refuses the rest before Noise.

"Refused" means the stream is dropped, which yamux turns into a reset (§7). No new error is put on the wire and no protocol rule changes.

---

## 7. StreamMuxer integration feasibility (Option A)

### 7.1 Where the wrapper goes [S]

`SwarmBuilder::with_tcp` hard-codes the composition: upgrade `V1Lazy`, authenticate, multiplex, then `.map(|(p, c), _| (p, StreamMuxerBox::new(c)))` (`libp2p/src/builder/phase/tcp.rs:83-88`). There is no injection point.

`SwarmBuilder::with_other_transport` (`builder/phase/other_transport.rs:25-51`, also reachable directly from the TCP phase, `tcp.rs:163`) accepts any `Transport<Output = (PeerId, impl StreamMuxer)>` and boxes it itself. The node can therefore build the same chain explicitly and add one step:

```
tcp::tokio::Transport
  .upgrade(Version::V1Lazy)
  .authenticate(noise)
  .multiplex(yamux)
  .map(|(peer, muxer), endpoint| (peer, GuardedMuxer::new(muxer, guard.connection(peer, endpoint))))
```

`Transport::map` passes the `ConnectedPoint` (`libp2p-core/src/transport.rs:190-193`), so the wrapper knows the authenticated peer, the direction and the remote address when it is created.

### 7.2 The questions asked, answered from source

| Question | Answer | Basis |
|---|---|---|
| Observe every inbound stream exactly once? | **Yes for every stream the swarm receives.** Each is returned once by `poll_inbound`. Streams that libp2p-yamux itself drops at its 256-stream buffer never surface, and never cost negotiation either | `libp2p-core/src/muxing.rs:68-113`; `libp2p-yamux/src/lib.rs:85-97`, `:126-145` |
| Before multistream negotiation? | **Yes.** The swarm calls `poll_inbound` and only then builds the negotiation | `libp2p-swarm/src/connection.rs:417-426` |
| Distinguish inbound from outbound? | **Yes, by construction**: separate trait methods | `muxing.rs:80`, `:86` |
| Refuse one stream without corrupting the muxer? | **Yes.** Dropping an open yamux stream removes it from the map and sends a reset. libp2p-yamux already does exactly this when its buffer is full | `yamux/src/connection.rs:511-560`; `libp2p-yamux/src/lib.rs:134-139` |
| Close only the offending connection? | **Yes.** An error from `poll_inbound` or `poll` propagates with `?` and closes that connection as `ConnectionError::IO` | `connection.rs:387`, `:418`; `connection/error.rs:28-35` |
| Preserve polling and wakers? | **Feasible, with care.** After refusing a stream the wrapper must poll the inner muxer again rather than return `Pending` with no waker registered. A per-call refusal budget followed by `wake_by_ref` keeps one connection from monopolising the executor | design requirement; libp2p-yamux uses the same wake-and-yield pattern at `lib.rs:142-144` |
| Preserve `AsyncRead` / `AsyncWrite`? | **Yes.** The admitted stream is wrapped in a struct that delegates both traits and holds the permit | `muxing.rs:70` (`Substream: AsyncRead + AsyncWrite`) |
| No unbounded queues or allocations? | **Yes.** The wrapper holds no queue: it admits or drops. State is one Connection scope plus one permit per live stream | design |
| Release on drop? | **Yes**, RAII: the permit is a field of the stream wrapper | design |
| Compatible with the existing yamux? | **Yes.** No yamux code or configuration changes; yamux defaults are kept | — |

### 7.3 What rejection can and cannot save

| Cost per attacker stream | Avoided by refusing at G3 | Basis |
|---|---|---|
| Kernel receive, TCP | No | — |
| Noise AEAD decryption of the frames | No | — |
| yamux frame parsing, stream object, reset frame | No | [S] |
| Multistream-select negotiation, timers | **Yes** | [S] |
| Swarm stream-upgrade and handler dispatch | **Yes** | [S] |
| request-response drop and its log record | **Yes** | [S] |

In the measured profile the avoidable stages are roughly negotiation 3.9% plus part of swarm 8.6%, runtime 6.6% and allocator 4.3% [M]; the kernel and yamux shares mostly remain. **Refusing streams alone will therefore not bring a determined flood far below saturation** [E]. It makes each stream cheaper; it does not stop them arriving.

**Closure is what removes the load**, and here it is justified because what it stops is expensive. For one source, after closure [E, from measured unit costs]:

- at most about 1 admitted reconnection per second (A2.3-a) × 2.6 ms;
- plus one burst of streams per connection × 35 – 72 µs each;
- plus 0.5 ms for each refused reconnection attempt.

With a burst of 64 that is on the order of 5 – 7 ms of CPU per second per source, against 2 saturated cores today. **This figure is an estimate and is the main thing Phase 2 must measure.**

### 7.4 Risks specific to Option A

- **Transport equivalence.** Replacing `with_tcp` by an explicit chain must reproduce it exactly, including `V1Lazy`. A mixed-build interoperability test is a gate, not an afterthought.
- **Observation follows the swarm's pace, not arrival.** The wrapper sees a stream when the swarm asks for one, which it does only while fewer than 16 are negotiating. Arrival itself stays inside yamux and its 256-stream buffer. This bounds what the wrapper can count, and is acceptable: unobserved streams cost no negotiation.
- **False closure of an honest peer.** Honest connections open few streams: a height poll every 15 s, one block range at a time, one announcement per block. The bucket leaves an order of magnitude of headroom, and the grace allowance covers a burst. It still must be shown on the devnet harness and under catch-up.
- **Reconnect amplification.** Closure must not be reachable by cheap states (§4 lesson). Only rate abuse feeds the score.
- **Pinned-version coupling.** The wrapper uses only the public `StreamMuxer` trait. Whether that trait is unchanged in later releases was not stated in the changelogs read, so it is unverified.

---

## 8. Upstream upgrade feasibility (Option B)

All statements in this section are from the rust-libp2p changelogs and workspace `Cargo.toml` fetched for this study [U]. **A silent changelog does not prove a feature is absent from the code**; no later source was read.

| `libp2p` release | MSRV stated | Notable entries found |
|---|---|---|
| 0.54.0 | 1.73 | — |
| 0.55.0 | 1.83.0 | moves to `libp2p-core` 0.43.0 |
| 0.56.0 | not stated | — |
| 0.57.0 | **1.88.0** | — |
| 0.58.0 (master) | workspace `rust-version = "1.88.0"`, edition 2024 | swarm 0.49.0, core 0.44.0, libp2p-yamux 0.48.0, request-response 0.31.0 |

Relevant API entries:

- `libp2p-core` 0.42.0: `Transport::dial` takes a `DialOpts`; `dial_as_listener` removed.
- `libp2p-swarm` 0.46.0 deprecates `InboundOpenInfo` / `OutboundOpenInfo`; 0.47.0 undoes it.
- `libp2p-request-response` 0.28.0: "Add connection id to the events".
- `libp2p-yamux` 0.48.0: yamux 0.12 support removed; yamux 0.14; MSRV 1.88.0.

Findings:

- **Is there a native solution?** None found. No changelog in range mentions a resource manager, stream admission, backpressure changes or rate limiting. The issue search returned no open issue about one.
- **MSRV:** 0.55.x fits under 1.85; 0.56.0 is unstated; 0.57.0 and later do not fit.
- **What an in-range upgrade would buy:** the connection id on request-response events, which would allow the stream → protocol link of §6.4. Which `libp2p` release carries request-response 0.28.0 was not stated.
- **What it would cost:** a transport API change, handler API churn, and re-validation of every behaviour in use (kad, mdns, identify, ping, request-response) plus the A1 – A2.3 evidence, which is tied to the pinned versions.

**Conclusion:** an upgrade does not address the measured attack. It should not be recommended on the strength of a version number, and it is not recommended here. It remains a separate hygiene decision under the existing MSRV rule.

---

## 9. Resource reservation lifecycle

```
 (1) TCP accepted        charge  Source + Global.pending          RAII: PendingPermit
        │  refused → socket dropped, counters only
 (2) Noise + yamux       no new charge (already reserved)
        │  handshake fails or times out → PendingPermit dropped
 (3) authenticated       move    Pending → Peer(id).connections   ConnectionScope created
        │  over a peer or global connection limit → denied (A1), permit dropped
 (4) stream opened       charge  Connection + Peer + Global.transient   StreamPermit
        │  no token / over cap → stream dropped (reset), score += 1
        │  score over threshold → ONE closure of this connection
 (5) negotiation         permit unchanged
        │  fails or times out (10 s) → stream dropped → permit released
 (6) request decoded     charge  Protocol + Peer + Source + Global.work   (tokens, consumed)
        │  refused → dropped unanswered, as today
 (7) frame bytes         charge  Global.frames by pool            RAII: Reservation (exists today)
 (8) stream finished     release StreamPermit
 (9) connection closed   release ConnectionScope and every permit under it
```

| Event | Behaviour |
|---|---|
| Failed authentication | The pending permit is dropped. The source's attempt token stays spent: failing a handshake is not free |
| Failed protocol negotiation | The stream is dropped by the swarm; its permit is released; the opening token stays spent |
| Stream rejection | Dropped at G3, reset on the wire, no permit issued, score incremented |
| Connection termination | Everything under the Connection scope is released by ownership, not by bookkeeping |
| Peer reconnection | The Peer and Source entries persist until fully refilled; a new connection starts with the peer's existing balance |
| Several connections for one peer | Each has its own Connection scope; the Peer scope sums them; A1 keeps the count at 2 |
| NAT and shared addresses | All peers behind one address share its Source buckets. This is a known false-positive source; the source burst must stay generous and the prefix configurable |
| Counter overflow | Saturating integer arithmetic throughout, as in the existing buckets |
| Clock | `Instant` only; a reading earlier than the last adds nothing |
| Bounded state | Peer and Source tables capped (4096 today) with the existing rule: drop only fully refilled entries, otherwise share an overflow entry. Connection scopes are bounded by the connection limits. Stream permits are bounded by the concurrency caps |
| Concurrency | The swarm task owns admission decisions; scope counters shared with stream permits are atomics, released from whichever task drops the stream |

**Tokens versus reservations.** Rates are *tokens*: consumed, refilled by time, never returned. Concurrency and bytes are *reservations*: held by an RAII guard and returned on drop. Mixing the two is the commonest source of leaks, so the guard exposes them as two distinct types.

---

## 10. Security and fairness analysis

| Risk | Status with the full design | Why |
|---|---|---|
| High-frequency short-lived streams | **Mitigated (expected, unmeasured)** | G3 rate bucket, then closure, then A2.3-a on reconnect |
| Connection churn | **Mitigated (measured)** | A2.3-a before Noise |
| Expensive valid requests | **Mitigated (measured)** | A2.3-b-1 before storage |
| Reconnection amplification | **Mitigated by construction** | Closure reachable only through rate abuse; reconnects throttled before cryptography |
| Slow or held streams | **Partially** | Concurrency caps refuse new streams; the 10 s negotiation timeout frees slots. Up to 16 negotiating slots per connection can still be occupied for 10 s |
| Sybil identities from one address | **Partially** | They share Source buckets. Identities themselves are free |
| Many source addresses | **Partially** | Global budgets bound the total; they do not protect an honest peer's share |
| IPv6 privacy addresses | **Mitigated within a /64**; not against an attacker holding a wider prefix | Prefix grouping |
| NAT concentration | **Not mitigated: accepted false positives** | Shared Source buckets |
| Global budget exhaustion | **Not mitigated** | Needs reserved shares or pinned peers — A2.3-c |
| Honest-peer starvation | **Partially** | This node's own sync uses outbound streams and the solicited frame pool, both outside the inbound limits. Serving others can still be crowded out |
| Eclipse and inbound-slot occupation | **Not mitigated** | Needs slot reservation and fair replacement — A2.3-c |
| yamux receive-window memory | **Not measured** | Default 256 KiB credit per stream, 512 streams, 1 GiB connection window (`yamux/src/lib.rs:45`, `:89-90`). G3 concurrency caps would bound the stream count the node keeps |
| Decoded messages, outbound responses | **Not mitigated** | Outside A2.2b-lite; separate work |
| Allocator retention | **Not mitigated** | Measured glibc behaviour; a memory-policy question, not a guard question |

go-libp2p's own README makes the same admission for its design: no notion of honest versus bad peers [U]. A resource guard bounds cost; it does not establish trust.

---

## 11. Metrics and operational design

Counters are aggregated and written with the existing once-a-minute summary. **No per-stream or per-refusal line above DEBUG.**

| Group | Metrics |
|---|---|
| Streams | inbound stream attempts observed; admitted; refused by rate / by concurrency / by global budget; active and peak, global and per-connection maximum |
| Negotiation | negotiation failures and timeouts (where observable); streams dropped at the request-response cap (existing counter) |
| Budgets | per scope and per limit: used, peak, capacity, refusals — the shape `FrameBudget` already uses |
| Peers and protocols | admitted and refused by protocol; top-N peers by refusals (bounded N) |
| Abuse | connections closed for stream abuse; reconnections refused at G1 after such a closure |
| Process | CPU and resident memory (existing `P2P resources` line) |
| Liveness | time since this node's last successful height answer; catch-up lag in blocks |

**Operational alerts — five, each actionable:**

1. **Sustained refusals at one gate** for more than a few minutes → under attack, or a limit is too low.
2. **Closures for abuse concentrated on few sources** → candidates for a firewall rule.
3. **Global budget at capacity** while honest lag grows → raise the budget or shed load; the signal that A2.3-c is needed.
4. **Honest sync lag above a threshold** → liveness regression, regardless of cause.
5. **Refusals with no matching rise in offered load** → suspect a false positive (NAT, misconfiguration).

---

## 12. Future CPU/GPU compute compatibility

Nothing here is implemented; this fixes boundaries only.

- **Separate guards, no shared pool.** The P2P guard budgets network resources. A future compute guard budgets execution. Neither may borrow from the other.
- **Resource classes reserved for later:** CPU execution, GPU execution, memory, storage, network bandwidth. They would be scopes of the compute guard, not of this one.
- **Isolation by process, not by convention.** The DoS guide's "reduce blast radius" advice [U] applies directly: a compute worker should run in its own OS process and cgroup, so that exhausting its budget cannot starve consensus or P2P. This matches the existing separation between the node and the provider/worker binaries.
- **One shared vocabulary.** The token / reservation distinction and the scope pattern are reusable; the *instances* and their limits are not.

---

## 13. Comparison of alternatives

| Criterion | **A. Guarded `StreamMuxer`** | B. Dependency upgrade | C. request-response handler wrapper | D. Keep isolated mitigations |
|---|---|---|---|---|
| Security | Closes the stream gap; one model | Does not address the attack | Closes it later in the pipeline | Gap stays open |
| CPU effectiveness | Refusal before negotiation; closure removes the load | None by itself | Refusal after negotiation; closure removes the load | None |
| Memory protection | Bounds streams the node keeps | None found | None before negotiation | None |
| Honest-peer fairness | Per-connection; needs measured grace | n/a | Per-connection, exact counts | n/a |
| MSRV compatibility | 1.85, no change | 0.55.x only; 0.57+ needs 1.88 | 1.85, no change | — |
| Maintenance cost | Medium: one small wrapper on a public trait, plus an explicit transport chain | High: re-validate everything | Medium to high: delegate a whole behaviour and handler | Rising with each new patch |
| Dependency risk | None added | High | None added | None |
| Future extensibility | Scope tree extends to new protocols | Unknown | Per-protocol only | Poor |

---

## 14. Implementation phases

Each phase is a separate decision. None is authorised by this document.

| Phase | Content | Exit condition |
|---|---|---|
| **0. Feasibility gate (test-only)** | A counting muxer wrapper used only in tests, on the explicit transport chain. No node behaviour change | All four hold: (a) real sync streams counted exactly, N for N; (b) each is seen before negotiation; (c) a refused stream leaves the connection and later streams working; (d) a build with the explicit chain interoperates with a `with_tcp` build |
| 1. Observe | Guard skeleton with scopes and metrics; the wrapper counts but never refuses | Counters agree with the request-response capacity counter under the existing flood; no regression in the devnet harness or Vertical |
| 2. Enforce at G3 | Rate and concurrency refusal, then closure with grace | Acceptance criteria of §15 |
| 3. Consolidate | Move A1, A2.2b-lite, A2.3-a and A2.3-b-1 configuration and counters under the guard, without changing their decisions | Byte-identical decisions on recorded inputs |
| 4. Later work | A2.3-c (reserved slots, fair replacement), decoded-message and outbound budgets, yamux window measurement | Separate designs |

If Phase 0 fails, the fallback is Option C, already analysed; the upgrade remains not recommended as a fix.

---

## 15. Acceptance criteria

For Phase 2, on the existing A2.0 harness (512 MiB, 2 CPUs, internal network). These are **experimental targets on that host**, not security guarantees.

**Method.**
- Stream floods must be compared at **equal offered stream rate**: the harness needs an open-loop stream generator, as `churn-rate` provides for connections. Closed-loop tools slow themselves when refused and overstate the gain.
- At least 3 trials per scenario, with raw evidence and manifests.

**Targets.**

| Area | Criterion |
|---|---|
| CPU, rapid stream churn | Victim CPU at or below 0.5 cores at an offered rate that saturates 2 cores today |
| CPU, held streams | No higher than without the guard (0.007 cores measured) — closure must not be triggered |
| Counting | Observed stream openings equal the generator's count for admitted connections |
| Closures | One closure per offending connection; reconnections refused at G1 are counted |
| Honest liveness | An established honest peer receives at least 95% of the producer's blocks; a late follower converges within 30 s; no honest connection is closed for abuse in the devnet harness or under catch-up |
| Memory | Guard state bounded by the configured table sizes; peak no worse than today |
| Logs | At most 200 lines per 10 s under attack |
| Compatibility | Mixed-build interoperability passes; no change to negotiation strings, framing or message shapes |
| Regression | Devnet Convergence and Compute v0.4 Vertical pass; A1, A2.2b-lite, A2.3-a and A2.3-b-1 tests pass unchanged |

Success is **not** to be claimed from a rate counter or a closure count alone: the CPU and liveness rows decide.

---

## 16. Recommendation

**Exactly one path is recommended: Option A, the guarded `StreamMuxer` under a single `P2pResourceGuard`, starting with the Phase 0 test-only feasibility gate.**

- **GO** — Phase 0 only.
- **NO-GO** — any production implementation before Phase 0 passes.
- **NO-GO** — a libp2p or yamux upgrade as a mitigation for this attack.
- **NO-GO** — public network exposure. The stream layer, inbound-slot fairness (A2.3-c) and several memory items remain open.

**What is and is not established.** Source reading supports Option A on every feasibility question. No wrapper has been run. Until Phase 0 passes, the accurate statement is: *a safe path is identified and argued from source, but not yet demonstrated.*

---

## Appendix — source references

Pinned crates, as in `Cargo.lock` at `232becb7`:

| Fact | File and lines |
|---|---|
| Pending-connection hook before the upgrade | `libp2p-swarm-0.44.2/src/lib.rs:958`, `:986` |
| Muxer polled; errors close the connection | `libp2p-swarm-0.44.2/src/connection.rs:387`, `:398`, `:418` |
| Inbound stream → negotiation, no hook between | `connection.rs:417-426`, `:573` |
| `listen_protocol()` also called per poll pass | `connection.rs:189`, `:421`, `:434`, `:457-463` |
| Negotiated streams and errors routed to one sub-handler | `libp2p-swarm-0.44.2/src/handler/select.rs:85-94`, `:148-175`, `:294-301` |
| Negotiation timeout default 10 s | `libp2p-swarm-0.44.2/src/handler.rs:482` |
| `StreamMuxer` trait | `libp2p-core-0.41.3/src/muxing.rs:68-113` |
| `StreamMuxerBox::new` | `libp2p-core-0.41.3/src/muxing/boxed.rs:90` |
| `Transport::map` receives the `ConnectedPoint` | `libp2p-core-0.41.3/src/transport.rs:190-193` |
| `with_tcp` composition | `libp2p-0.53.2/src/builder/phase/tcp.rs:83-88` |
| `with_other_transport` | `libp2p-0.53.2/src/builder/phase/other_transport.rs:25-51`; `tcp.rs:163` |
| libp2p-yamux defaults to yamux 0.13 | `libp2p-yamux-0.45.2/src/lib.rs:226-230` |
| Inbound buffer of 256, dropped beyond | `libp2p-yamux-0.45.2/src/lib.rs:61`, `:126-145` |
| `set_max_num_streams` | `libp2p-yamux-0.45.2/src/lib.rs:326` |
| yamux defaults: 256 KiB credit, 512 streams, 1 GiB connection window | `yamux-0.13.10/src/lib.rs:45`, `:89-90` |
| Stream cap exceeded → connection terminated | `yamux-0.13.10/src/connection.rs:622-624`, `:725-727` |
| Dropped open stream → removed and reset | `yamux-0.13.10/src/connection.rs:511-560` |
| request-response drops at capacity after negotiation | `libp2p-request-response-0.26.3/src/handler.rs:170` |
| request-response events carry no connection id | `libp2p-request-response-0.26.3/src/lib.rs:128-150` |

Upstream pages read for this study:

- go-libp2p Resource Manager README (`p2p/host/resource-manager/README.md`, master);
- libp2p DoS mitigation guide (`libp2p.io/docs/dos-mitigation/`);
- rust-libp2p master: `libp2p`, `swarm`, `core`, `muxers/yamux` and `protocols/request-response` changelogs, and the workspace `Cargo.toml`.

Evidence sets (each with a SHA-256 manifest): `C:\Dev\a2-work` through `C:\Dev\a2-work10`.
