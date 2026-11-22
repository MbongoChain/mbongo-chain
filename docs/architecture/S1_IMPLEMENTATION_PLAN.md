# S1: RFC 0006 Implementation Plan

**Status: ACTIVE. Documentation only.** The gates this plan waited for are passed:

- **G1:** RFC 0006 was accepted on 2026-11-21 and merged in
  [PR #211](https://github.com/MbongoChain/mbongo-chain/pull/211) (`027086e5`).
- **G2:** S0 was merged in [PR #263](https://github.com/MbongoChain/mbongo-chain/pull/263)
  (`c36c0437`).

Each pull request below still needs the maintainer's explicit approval, and it goes through
the merge gate.

**Revision of 2026-11-21.** It records the dependencies confirmed after G1 and G2, in section
3a. Pull request S1-R is added, and S1-1, S1-3 to S1-7 and S1-10 to S1-13 are amended. The
order in section 4 changes accordingly. RFC 0006 is unchanged.

Stage S1 of [`MVP_v0.2_NETWORK_READINESS.md`](../product/MVP_v0.2_NETWORK_READINESS.md),
tracked in #205. The roadmap stays in #209, and this document holds no schedule.

**The RFC is the authority.** This plan only cuts it into reviewable pull requests. Where the
two differ, the RFC wins, and this plan is corrected. Section numbers such as "RFC §3" refer
to RFC 0006 at `6a88782`. "Lock §N" refers to [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md),
by the section labels the RFC uses.

## 1. Not in S1

Each of these is done, or belongs to another stage. S1 reuses it and does not rebuild it.

| Item | Where |
|---|---|
| Native balance conservation on the transfer path | S0 (#204), merged in #263 (`c36c0437`) |
| Reconnection: redial bootnodes with capped backoff, addresses without `/p2p/` (RFC §3, Reconnection) | Merged, S3 PR 2b (#230), `mbongo-network/src/bootnodes.rs` |
| `--node-key-file`, a stable peer identity (RFC §3) | Merged, S3 PR 2b (#230), `mbongo-network/src/node_key.rs` |
| Mempool bounds | Merged, S3 PR 1 (#213) |
| Sync recovery after a failed request | Merged, S3 PR 2a (#218), `mbongo-node/src/sync_tracker.rs` |
| Forwarding to the producer, and `submit_transaction` failing with `-32603` on a node with no route to it (RFC §7) | S3 (#207), not merged |
| Peer and rate limits, a prerequisite for untrusted peers (RFC Security) | S3 (#207) |
| `PROTOCOL_LOCK_v0.5.md`, `rpc_v0.4.md`, `storage_invariants.md`, the `v0.5-devnet-stable` tag | G4, after S1 and the S3 items above (RFC Rollout step 4) |
| Wallet CLI, faucet, test MBO allocations of a real network | S2 (#206) |

## 2. Working assumptions

- **Clean genesis, no migration.** A v0.5 network starts from a new genesis in an empty
  directory (RFC Compatibility). Nothing converts v0.4 data. `v0.4` and its tags stay the
  stable reference, and `mvp-v0.1.0` is untouched.
- **`dev` carries v0.5 in progress from the first merge.** Between S1-1 and G4, `dev` is
  neither v0.4 nor a finished v0.5. No network runs from `dev` in that window, and no data
  directory written by a build in that window is supported by a later one. A v0.4 fix, if
  ever needed, goes on a branch from the v0.4 tag.
- **Every pull request keeps the required checks green.** A PR that changes an encoding, a
  domain or a flag updates, in the same PR, every consumer it breaks: harnesses, tests,
  scripts, `docker-compose.yml`, the `Dockerfile`. A consumer is never left broken for a
  later PR.
- **Schema 4 arrives once, with the final block format** (S1-8). An earlier schema-4 stamp
  would label directories whose blocks a later PR can no longer decode.
- **Rollback is a revert.** Each PR is one squash commit on `dev`. Reverting it restores the
  previous behaviour, and there is no data to migrate back, by the assumption above.
- **New vector files, never rewritten ones.** The v0.4 fixtures stay as records (RFC
  Testing). Each PR adds versioned files beside them.

## 3a. Dependencies confirmed after G1 and G2

These do not change RFC 0006. Each one says where an obligation lands, or in what order two
pieces of work must go.

| # | Dependency | Where it lands |
|---|---|---|
| D-1 | **The SDK moves with the node change that breaks it.** A PR that changes what the SDK signs, decodes or calls updates the SDK in the same PR, so the required SDK checks never test against a stale node | S1-5 (signing domain), S1-7 (block decoding), S1-11 (`get_chain_info`, `chain_id` comparison). S1-12 keeps only the release shape |
| D-2 | **Transaction relay needs authenticated RPC before it crosses machines.** Forwarding from a follower makes the producer's RPC reachable by another host | S3 forwarding (#207) may merge for loopback and single-host tests. No multi-host run, and no S1-13 multi-host item, before the RPC authentication profile (a TLS proxy with mutual TLS or a token, and a method allow-list) exists |
| D-3 | **Compute clients and workers need accounts in genesis, with zero balances.** `ComputeTask` and `AnchorReceipt` require an existing sender account. Under v0.5 accounts come only from genesis or as the receiver of a `Transfer` | S1-3 accepts `"0"` allocations (RFC §1 allows them). S1-4 adds the compute client and worker keys of the dev and test genesis files as zero-balance allocations, with a test that a zero-balance genesis account can commit a task and anchor a receipt |
| D-4 | **The replay harness must call the real block-application path.** Today it stores exported blocks and compares the tip, without calling `apply_block` | New S1-R, early and independent. It moves out of S1-13, because S1-6 needs it to prove state agreement |
| D-5 | **An invalid transaction must not stop the production of otherwise valid blocks.** Today `produce_block` applies everything it selects as one block, and a single failing transaction makes every later attempt fail the same way | S1-6 (view-based production): a failing transaction is left out with no effect, and is removed from the pool or bounded, so it cannot be selected forever. Poison-transaction test required. S1-1 already keeps one such case out of the pool: a nonce beyond the bound |
| D-6 | **Writes from production, RPC and synchronization are serialized.** Block production, blocks applied from peers, and any RPC path that writes share one writer | S1-10, with a test that drives the three concurrently and checks one block per height and a consistent mempool |
| D-7 | **The P2P peer allow-list follows chain binding.** The allow-list is checked after the protocol negotiation that S1-9 binds to `chain_id` | Peer allow-list after S1-9 (outside S1; tracked with the network access work) |

## 3. Pull requests

Every PR states the items required by section 10 of the MVP specification, uses `Refs #205`,
and is Tier 0 unless marked otherwise. Each one merges only with the maintainer's explicit
approval, through the merge gate.

### S1-1. Transfer path conformance (RFC §6), residual after S0

- **Depends on:** G1 and G2. S0 merged.
- **Scope (set after S0 merged):** what RFC §6 still requires once the merged code is
  compared with it:
  - **The nonce bound.** Step 2 adds one to the sender's nonce, and `u64::MAX` has no
    successor. A transaction whose nonce is `u64::MAX` is refused with `InvalidNonce`
    (implementation note C-3 on #211), in every build profile and for every transaction
    type. Admission refuses it too, so the pool never holds a transaction no block could
    include. `MAX_TRANSACTION_NONCE = u64::MAX - 1`.
  - **Shared vectors:**
    - Transfer and Stake encodings at the nonce and amount bounds;
    - AnchorReceipt at the bound, through the SDK builder;
    - malformed encodings;
    - signatures over the wrong message;
    - the state-transition cases of RFC Testing at the bound;
    - `SCALE(Account)`, which RFC §4 makes a consensus input.
- **Modules:**
  - `mbongo-core/src/account.rs` (`validate_and_increment_nonce`);
  - `mbongo-node/src/backend.rs` (admission);
  - new fixtures `test-vectors/transaction/nonce-boundary-v1.json` and
    `test-vectors/account/account-v1.json`;
  - Rust and SDK consumers.
- **Protected:** Lock §5, validation rules. A change in which transactions are accepted.
- **Compatibility:** v0.4 chains could be read differently. Acceptable only because v0.5
  starts from a new genesis.
- **Tests:**
  - invariants I1 to I4;
  - for each transaction type: a stored copy, a second copy in one block, and two copies of
    a stored `Transfer` or `Stake`, with a vector;
  - a failure writes nothing;
  - the nonce bound for Transfer, Stake, ComputeTask and AnchorReceipt, and at admission.

### S1-R. Replay through `apply_block` (RFC Testing), early and independent

- **Depends on:** nothing in S1. It can merge before or after S1-1 and S1-2.
- **Scope:** `replay_harness` replays the exported chain on a fresh
  `NodeBackend<InMemoryStorage>`, starting with `ensure_genesis` rather than a hand-made dev
  account, and calls `apply_block` for every block. It compares the tip, and the state the
  blocks touched, with the producer. It stops storing blocks and derived indexes itself.
- **Smallest change:** `backend` and `mempool` become modules of the `mbongo-node` library
  (`lib.rs`), and `main.rs` and its modules import them from there. The harness can then
  construct the backend. No behaviour changes.
- **Modules:** `mbongo-node/src/lib.rs`, `main.rs` and the modules that import `backend`;
  `mbongo-node/src/bin/replay_harness.rs`.
- **Protected:** none. It is a test harness, plus a visibility change.
- **Tests:**
  - the harness replays at least 10 blocks holding a Transfer, a ComputeTask and an
    AnchorReceipt, through `apply_block`;
  - tip hash, balances and nonces of every touched account, tasks and receipts equal the
    producer's;
  - **a negative self-test:** an exported block altered by one byte is refused by the
    replay, and the harness fails with the block's height;
  - once S1-6 lands, `state_root` agrees at every height.

### S1-2. Strict Ed25519 verification (RFC §2, §3)

- **Depends on:** G1. Independent of S0 in content, merged after it to keep one order.
- **Scope:** one strict definition for transaction, receipt and block signatures: `s`
  below the group order, `R` and the key canonical and not of small order, the equation
  without the cofactor. `verify_strict` plus an explicit canonical-encoding check.
- **Modules:** `mbongo-core/src/crypto.rs`, `primitives.rs` (`Transaction::verify_signature`),
  `receipt.rs`; `mbongo-verification/src/receipt.rs` (`verify_receipt_signature`, which
  uses the library's default `verify` today); the SDK's receipt verification
  (`sdk/typescript/src/receipt.ts`), so that client and node apply one definition (D-1).
- **Protected:** Lock §5 rule (h), receipt signature verification
  (`RECEIPT_SPEC_v0.1.md`, verification only).
- **Compatibility:** signatures accepted today may be refused. No encoding changes.
- **Tests:** negative vectors for non-canonical `s`, `R` and key, small-order `R` and key,
  and a signature that passes only the cofactored equation, for transactions and receipts.
  Block cases join in S1-7.

### S1-3. Genesis file and `chain_id`, as a library (RFC §1)

- **Depends on:** G1.
- **Scope:** parse and validate `mbongo-genesis:v1`, refuse any non-canonical file, derive
  `chain_id`, and add the dev genesis file and the dev authority seed. No node behaviour
  changes yet.
- **Modules:** a new module in `mbongo-core` (for example `genesis.rs`); the dev genesis
  file in the repository.
- **Protected:** Lock §10, genesis; Lock §11, vectors.
- **Compatibility:** none at runtime: nothing calls it yet.
- **Tests:**
  - vectors for `chain_id` and the genesis file, independently derived;
  - every malformed form refused, including more than one authority, unsorted or duplicate
    keys and allocations, escapes, whitespace and overflow;
  - a `"0"` allocation accepted, creating the account (D-3).

### S1-4. Node genesis wiring and flags (RFC §1)

- **Depends on:** S1-3.
- **Scope:** `--genesis <file>` or `--dev`, exactly one; `--chain` and `MBONGO_CHAIN`
  removed; genesis block, allocations written at start. `ensure_genesis` stops creating the
  hard-coded dev account and reads the file instead. Chain identity is computed at start,
  not yet stored (S1-8).
- **Modules:** `mbongo-node/src/main.rs` (Tier 1 for flags), `backend.rs`
  (`ensure_genesis`, Tier 0).
- **Consumers updated in the same PR:** `docker-compose.yml`, `Dockerfile`, `scripts/devnet/*`,
  `scripts/devnet_test.ps1`, `scripts/replay_test.ps1`, `scripts/check-harness-ports.mjs`,
  `scripts/mvp-launch.mjs`, `scripts/mvp-d-acceptance.mjs`, `replay_harness`,
  `devnet_harness`, `compute_vertical`, `compute_harness`,
  `mvp_inference_harness`, `provider_harness`, and the tests that start a node, among them
  `deterministic_replay`, `devnet_convergence`, `node_key_flag`, `p2p_mdns_toggle`,
  `reference_flow`. The RFC lists most of these, and the scripts in the second half of the
  list are added here.
- **Protected:** Lock §10, genesis.
- **Compatibility:** every operator command line changes. `--dev` must reproduce today's
  dev account and amount, so that the harnesses keep their balances.
- **Tests:**
  - genesis written atomically;
  - `--dev` and `--genesis` exclusive;
  - the dev genesis gives the existing dev account;
  - the zero-balance compute client and worker accounts of the dev genesis exist, and can
    commit a task and anchor a receipt (D-3).

### S1-5. Transaction signing domain (RFC §2)

- **Depends on:** S1-2, S1-4.
- **Scope:** signed bytes become `"mbongo-tx:v2" || chain_id || SCALE(...)`. The
  `Transaction` encoding and hash do not change.
- **Modules:** `mbongo-core/src/primitives.rs` (`signing_payload`, `verify_signature`);
  every Rust signer: `mbongo-wallet` and its examples, `mbongo-compute` `chain.rs`, client,
  worker, service and harnesses; `backend.rs` admission and `apply_block` verification;
  **the SDK signers** (`sdk/typescript/src/anchor.ts`, `compute-task.ts`) and their tests,
  in the same PR (D-1).
- **Protected:** Lock §2, signing input; Lock §5.
- **Compatibility:** every signature made for v0.4 is refused, including by SDK 0.2.0.
  Fixtures `compute-task-v1`, `anchor-receipt-v1`, `anchor-binding-v1` and
  `compute-task-rpc-v1` gain new versions wherever they hold a signature.
- **Tests:** signing-domain vector; a transaction signed for another `chain_id` is
  refused; the compute vertical passes on the new domain.

### S1-6. State commitment (RFC §4, §5 rule (w)) and view-based production (RFC §3)

- **Depends on:** S1-1, S1-4.
- **Scope:** `state_root(0)` from the allocations, `state_root(h)` from each block's
  sorted changes, rule (w) after all transactions; the producer executes on an uncommitted
  view, leaves out a failing transaction with no effect, computes `state_root`, sets
  `timestamp = max(now, parent.timestamp)`, then commits through `apply_block`.
- **Modules:** `backend.rs` (`apply_block`, `produce_block`); `mbongo-core/src/account.rs`
  (`Account` encoding becomes a consensus input); `mbongo-storage` only if a view needs a
  read path it lacks.
- **Protected:** Lock §5 (new rule (w)); the `Account` encoding, locked from v0.5.
- **Compatibility:** `state_root` is non-zero from here. Rule (v), `InvalidTimestamp`,
  lands in S1-7 with the header change.
- **Depends also on:** S1-R, so that replay proves state agreement through `apply_block`.
- **Tests:**
  - `state_root` vectors;
  - two nodes forced to diverge stop at the first differing block;
  - a transaction failing in the view leaves no partial effect, and the block stays valid;
  - generated blocks over every sender and receiver combination with boundary amounts,
    checked against RFC §6;
  - **poison transaction (D-5):** a pending transaction that fails in the view does not
    stop the next blocks. It is left out of each one and cannot be selected forever.

### S1-7. Authenticated blocks (RFC §3, §5 rules (t), (u), (v))

- **Depends on:** S1-2, S1-6.
- **Scope:** `BlockHeader.producer` and `Block.producer_signature` appended; block signing
  domain `"mbongo-block:v1" || chain_id || SCALE(header)`; rules (t), (u), (v) in that
  order; `--producer-key-file`, the dev authority key under `--dev`, and refusal of
  `--dev` with `--producer-key-file`. `compute_block_hash` keeps hashing the whole block.
- **Modules:** `mbongo-core/src/primitives.rs`; `backend.rs`; `main.rs`;
  `mbongo-network/src/p2p_protocol.rs` and `sync_service.rs` carry the new block bytes;
  `replay_harness` (its own `compute_block_hash`); `mbongo-compute` `chain.rs`; **every
  block decoder in the SDK** (`client.ts`, `types.ts`, `compute-task.ts`
  `computeTasksInBlock`) and its tests, in the same PR (D-1).
- **Protected:** block SCALE encoding, block hash input, Lock §5.
- **Compatibility:** every stored or transmitted block changes format. Peers on v0.4 can
  no longer decode blocks, and S1-9 makes them fail earlier, at negotiation.
- **Tests:** negative tests for (t), (u) and (v), each asserting its own error; strict
  negative vectors for blocks; a forged block refused.

### S1-8. Storage schema 4 and chain identity (RFC Compatibility)

- **Depends on:** S1-7, so that the first schema-4 directory holds final-format blocks.
- **Scope:** schema 3 → 4; `chain_id` and the genesis hash stored with the genesis block,
  in one `write_batch`; refusal of any stamp below 4, and of another chain's directory; a
  crash during genesis creation restarts as new. The authority's block commit becomes a
  synced write (RFC Scope).
- **Modules:** `mbongo-storage` (`rocksdb.rs`, `memory.rs`, `storage.rs`, `lib.rs`);
  `backend.rs` (`ensure_genesis`, commit path).
- **Protected:** Lock §6, storage schema; `write_batch` durability for the authority.
- **Compatibility:** a v0.4 directory is refused rather than stamped, unlike earlier
  upgrades. Operators must start from an empty directory.
- **Tests:** schema-3 directory refused; another chain's directory refused; a directory
  left by a crash during creation restarts as new.

### S1-9. P2P chain binding (RFC §7)

- **Depends on:** S1-4. May merge any time after it, and should precede S1-10.
- **Scope:** `/mbongo-sync/3/<c>` and `/mbongo/block_notify/0.3.0/<c>`, with `<c>` the first
  16 hex characters of `chain_id`; identify metadata `/mbongo/0.5.0`. Framing unchanged.
- **Modules:** `mbongo-network/src/p2p_protocol.rs`, `p2p.rs`;
  `mbongo-network/tests/protocol_negotiation.rs`.
- **Protected:** Lock §9, P2P identifiers.
- **Compatibility:** v0.4 and v0.5 nodes stop negotiating, by design.
- **Tests:** two nodes with different genesis files refuse to sync, at negotiation.

### S1-10. Producer obligations (RFC §3)

- **Depends on:** S1-7, S1-8, S1-9. Reuses the reconnection of #230.
- **Scope:** production, blocks applied from peers, and every RPC path that writes, under
  one writer (D-6); synced commit before broadcast;
  the 30-second catch-up after a start; `--producer-start-alone <height>`; stop on a valid
  signed block above the tip, resume once applied, alert if it cannot be; `produce_block`
  fails while waiting or stopped. None of the wait or stop applies under `--dev`.
- **Modules:** `backend.rs` (`produce_block`); `main.rs` (producer task, flags);
  `sync_service.rs` where answers report heights.
- **Protected:** producer obligations (RFC Scope); `produce_block`.
- **Compatibility:** an authority can now halt rather than fork, until an operator acts.
- **Tests:** concurrent production, peer blocks and RPC writes give one block per height and
  a consistent pool; a crash before commit, a
  restart behind a peer, a restart with no peer answering, each give no second block at a
  height; false heights do not stop production nor end a catch-up served by another peer;
  `--producer-start-alone` refusals; `produce_block` fails through RPC and the timer alike;
  after an authority restart, followers redial and it resumes, with an address without
  `/p2p/` and with `--node-key-file`.

### S1-11. RPC surface of `rpc_v0.4` that belongs to S1 (RFC §7)

- **Depends on:** S1-7. Coordinated with S3 forwarding, which owns the `submit_transaction`
  rule.
- **Scope:** block results gain `producer` and `producer_signature`; new method
  `get_chain_info`. The `rpc_v0.4.md` document waits for G4.
- **Modules:** `mbongo-network/src/rpc.rs`, `server.rs`; `backend.rs` accessors;
  `mbongo-network/tests/jsonrpc_tests.rs`; `test-vectors/rpc`; **the SDK** `get_chain_info`
  call and its `chain_id` comparison against the caller's own genesis, in the same PR (D-1).
- **Protected:** Lock §8, RPC.
- **Compatibility:** a result shape changes. `mbongo-explorer`, which reads blocks over RPC,
  is optional and untested (MVP specification §3).
- **Tests:** shapes and `get_chain_info` against vectors; a client never takes `chain_id`
  from an endpoint.

### S1-12. TypeScript SDK release shape for v0.5 (Tier 2)

- **Depends on:** S1-5, S1-6, S1-7, S1-11.
- **Scope:** the protocol work has already landed with S1-5, S1-7 and S1-11 (D-1), and
  strict verification with S1-2. What remains: a new version number, README and package
  metadata, and the package gate. **Not published:** publication is a separate approval,
  through `npm-package-gate`.
- **Modules:** `sdk/typescript/src` (`client.ts`, `receipt.ts`, `types.ts`, and a new
  signing module); `sdk/typescript/test`.
- **Compatibility:** 0.2.0 stays valid for v0.4 only.
- **Tests:** every new vector matches its Rust derivation; strict negative vectors.

### S1-13. Acceptance completion (RFC Testing)

- **Depends on:** all of the above.
- **Scope:** devnet convergence and the compute vertical on v0.5; any RFC Testing item not
  yet covered by an earlier PR. Replay through `apply_block` moved to S1-R. Multi-host items
  wait for the RPC authentication profile (D-2).
- **Modules:** `devnet_harness.rs`, `compute_vertical.rs`; CI.
- **Tests:** the RFC acceptance criterion: every item passes in CI on one SHA, and the
  vectors match in Rust and TypeScript.

## 4. Order

| PR | Needs merged first | Can proceed independently now |
|---|---|---|
| S1-1 | G1, G2 (S0) | yes (in review) |
| S1-R | — | yes |
| S1-2 | G1 | yes |
| S1-3 | G1 | yes |
| S1-4 | S1-3 | after S1-3 |
| S1-5 | S1-2, S1-4 | no |
| S1-6 | S1-1, S1-4, S1-R | no |
| S1-7 | S1-2, S1-6 | no |
| S1-8 | S1-7 | no |
| S1-9 | S1-4 | after S1-4 |
| S1-10 | S1-7, S1-8, S1-9 | no |
| S1-11 | S1-7 | no |
| S1-12 | S1-5, S1-6, S1-7, S1-11 | no |
| S1-13 | all of the above, the S3 items of section 5, and for multi-host items the RPC authentication profile (D-2) | no |
| Peer allow-list (outside S1) | S1-9 (D-7) | after S1-9 |

S1-1, S1-R, S1-2 and S1-3 touch disjoint code. They can be reviewed in parallel, each on its
own branch from `dev`.

Proposed merge order: S1-1, S1-R, S1-2, S1-3, S1-4, S1-5, S1-6, S1-7, S1-9, S1-8, S1-11,
S1-10, S1-12, S1-13.

G3 is the merge of S1. The v0.5 lock, `rpc_v0.4` and the tag stay at G4.

## 5. Coordination with S3

S3 forwarding changes `submit_transaction` in `backend.rs` and `rpc.rs`, the same files as
S1-5, S1-7 and S1-11. Whichever merges second merges `dev` in and re-runs CI at its new
head. The RFC acceptance criterion needs forwarding and the failure rule on the same SHA
as S1, so S1-13 runs after them.

## 6. Decisions for the maintainer

1. **Merge into `dev` one PR at a time, or through an integration branch.** This plan
   assumes `dev`, with the assumptions of section 2. A long-lived branch would keep `dev` on
   v0.4, at the cost of one very large final review.
2. **S1-1 scope.** Resolved on 2026-11-21, after S0 merged: the nonce bound and the shared
   vectors (section 3, S1-1).
3. **The consumer list of RFC §1** names `scripts/devnet/*` and the harnesses, but not
   `scripts/devnet_test.ps1`, `scripts/replay_test.ps1` or `scripts/check-harness-ports.mjs`.
   S1-4 updates them. This reads as an informative list, not a normative change, so it
   needs no RFC revision. If the maintainer disagrees, it is a text change after the
   `READY_FOR_REVIEW` declaration, and needs its own review.
4. **SDK publication** after S1-12, as its own approval.
