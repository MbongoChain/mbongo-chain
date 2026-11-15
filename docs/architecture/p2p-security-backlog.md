# P2P Security Backlog

> **Status: a list of open items. Non-normative. It authorizes nothing.**
> Started 2026-11-14 (resource guard, Phase 1C). No P2P security backlog existed in this repository before; the work it follows is tracked under the S3 issue (#207).

Each entry says what was observed, what is established, what is not, and what would close it. An entry is closed by evidence, not by an explanation that fits.

**How to add an entry.** Give it the next `P2P-SEC-nnn` identifier and the same headings as the entries below. Keep measurements and hypotheses apart. Link the document that holds the evidence.

**Not migrated here yet.** Other open points of the resource guard work are still recorded where they were found: the residual-risk sections of the [Phase 1](p2p-resource-guard-phase1-report.md), [Phase 1B](p2p-resource-guard-phase1b-report.md) and [Phase 1C](p2p-resource-guard-phase1c-report.md) reports, and §14 of the [study](p2p-resource-guard-study.md). They are not duplicated in this file.

---

## P2P-SEC-001 — Under a stream flood, the guard counts a few more inbound streams than the application counters account for

| | |
|---|---|
| **Status** | **OPEN.** Bounded; cause not demonstrated |
| **Opened** | 2026-11-14, from the Phase 1 and Phase 1B measurements |
| **Component** | `mbongo-network`: `resource_guard` (observe-only) against the existing request counters |
| **Severity** | Accounting only. No stream is refused, lost or mishandled because of it |
| **Evidence** | [Phase 1 report](p2p-resource-guard-phase1-report.md), §5 (T3) and §6.2; [Phase 1B report](p2p-resource-guard-phase1b-report.md), §3 |

### What was observed

The guard's total of inbound streams was compared with what the node's existing counters account for: requests admitted or refused, streams dropped at the request-response capacity, plus the ping and identify streams of full nodes.

| Trial | Flooding connections | Streams observed by the guard | Streams the application counters do not account for |
|---|---|---|---|
| No hostile traffic, 3 trials | 0 | 507 each | **0, 0, 0** |
| Sync flood | 4 | 3,454,264 | 16 |
| Sync flood | 4 | 3,691,560 | 12 |
| Sync flood | 4 | 3,674,881 | 8 |
| Sync flood, every node guarded | 4 | 3,666,679 | 9 |
| Sync flood | 1 | 712,199 | 4 |
| Sync flood | 8 | 1,516,362 | 12 |

Seven flood trials are documented. In each, the guard saw **more** streams than the application counters, never fewer.

### Three events that must be told apart

1. **A stream handed by yamux to the wrapper.** Counted by the guard as `p2p_inbound_streams_total`, before any negotiation byte is read.
2. **A negotiation started.** Counted by the guard as `p2p_stream_negotiations_started_total`: the swarm did a first read or write on the stream. It says nothing of the outcome. In every trial above, (1) and (2) were equal.
3. **A stream actually handled by the application protocol.** Counted by the node's existing counters once a request has been decoded, then admitted or refused (`sync_requests_*`, `announcements_*`), or once request-response has dropped the stream at its per-connection capacity.

Between (2) and (3) a stream can exist without being in any application counter: while its negotiation runs, and while its request is being read. If it disappears in that interval, only the guard has seen it. Nothing counts that case today.

### What is established

- **No difference without hostile traffic**, in 3 trials of 3. A systematic offset between the two readings is excluded at that scale.
- **Under flood, a difference of 4 to 16 streams**, out of 0.7 to 3.7 million.
- **Never more than 4 per flooding connection**, in 7 trials of 7. Four is the request-response stream cap per connection.
- **The guard itself reported no error**: `p2p_stream_observation_errors_total` was 0 in every trial.

### What is not established

- **The cause.** The explanation that fits these bounds is: streams still open on the flooding connections when the traffic tool exits, observed by the guard and never decoded. It has not been demonstrated. No record attributes those streams one by one.
- **Proportionality.** The difference did not grow from four to eight flooding connections (12 in both cases).
- **Which interval.** Nothing distinguishes a stream lost during negotiation from one lost while its request was being read.

**This entry is not a statement that the counters are exact.** Exactness has been shown in controlled tests and in the no-flood scenario. Under flood there is a small, bounded, unexplained difference.

### What would close it: a reconciliation test at connection close

A future test, or set of tests, that meets all of these:

1. **Known states.** A client opens a known number of streams and brings each to one chosen state before closing the connection: (a) opened, no byte sent; (b) negotiation started, not finished; (c) negotiated, request partly sent; (d) request fully sent, not yet answered; (e) answered.
2. **One terminal category per stream.** After the connection has closed and no stream is active on it, every stream the guard observed falls in exactly one category: answered; refused, by which rule; dropped at capacity; negotiation failed; **closed before a request was decoded**; ping or identify.
3. **The sum closes to the unit.** The categories add up to `p2p_inbound_streams_total`, with no remainder, for each state mix in (1).
4. **Each state lands where predicted.** States (a), (b) and (c) fall in "closed before a request was decoded" and nowhere else; (d) and (e) in the application counters.
5. **Under flood too.** On the A2.0 harness, with the traffic tool stopping abruptly, the remainder is zero in every trial, for 1, 4 and 8 flooding connections. If it is not, the entry stays open.
6. **Readings taken at rest.** Both readings are taken after the closed connections have no active stream, and the test says how it knows.
7. **Observation only.** The test and whatever it needs change no admission decision.

**What it needs that does not exist.** An observable for "accepted by the request-response handler, lost before the request was decoded". `libp2p-request-response` 0.26.3 does not expose one. Where it would come from (a later libp2p, a handler wrapper, or something else) is not decided here.

### Out of scope for this entry

Any limit, refusal or closure policy. This entry is about counting.
