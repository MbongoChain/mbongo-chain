# Mbongo Chain — Architecture Overview for New Developers

**Version:** v0.2-devnet-stable
**Canonical Protocol Spec:** [PROTOCOL_DEFINITION_v0.1.md](./specs/PROTOCOL_DEFINITION_v0.1.md)

---

## Layer Separation

Mbongo Chain is structured as a Rust workspace with strict layer separation. Each crate has a single responsibility and well-defined boundaries.

```
┌──────────────────────────────────────────────────┐
│                  mbongo-node                      │
│          Orchestration and lifecycle              │
│    CLI parsing, wiring, startup, shutdown         │
├──────────────────────────────────────────────────┤
│                 mbongo-network                    │
│           Transport and protocol                  │
│   JSON-RPC, REST, libp2p P2P, sync service       │
├──────────────────────────────────────────────────┤
│                mbongo-storage                     │
│              Persistence layer                    │
│     RocksDB, InMemoryStorage, WriteBatch          │
├──────────────────────────────────────────────────┤
│                 mbongo-core                       │
│              Pure logic layer                     │
│  Block, Transaction, Account, Crypto, Encoding    │
└──────────────────────────────────────────────────┘
```

### mbongo-core — Pure Logic

The foundation. Contains all blockchain data types, cryptographic operations, and encoding logic.

- **Types:** `Block`, `BlockHeader`, `BlockBody`, `Transaction`, `Account`, `Hash`, `Address`
- **Crypto:** Ed25519 signing/verification, BLAKE3 hashing
- **Encoding:** SCALE (parity-scale-codec) derives on all types
- **Rules:** No I/O. No network. No filesystem. No dependencies on other Mbongo crates.

### mbongo-storage — Persistence

Handles reading and writing blockchain data. Defines the `Storage` trait with two implementations.

- **`Storage` trait:** `put_block`, `get_block_by_height`, `get_latest_height`, `get_account`, `put_account`, etc.
- **`RocksDbStorage`:** Production backend using RocksDB column families (blocks, transactions, accounts)
- **`InMemoryStorage`:** Test backend using `HashMap` behind `RwLock`
- **Atomicity:** All block writes go through `write_batch` — either every write in the batch succeeds, or none do
- **Rules:** No business logic. No network. No RPC.

### mbongo-network — Transport

Handles all communication: HTTP APIs and P2P networking.

- **JSON-RPC 2.0:** Axum-based server at `/rpc` with method dispatch
- **REST API:** Endpoints for blocks, transactions, accounts, validators
- **libp2p P2P:** Sync protocol (block request/response) and notification protocol (block announcements)
- **`RpcBackend` trait:** Abstracts the backend so the network layer never touches storage directly
- **Rules:** No state mutation. All mutations go through the backend trait.

### mbongo-node — Orchestration

The top-level crate that wires everything together and runs the node.

- **CLI:** `clap::Parser` for command-line arguments
- **`NodeBackend`:** Implements `RpcBackend`, owns storage and mempool, handles `produce_block` and `apply_block`
- **P2P lifecycle:** Creates libp2p swarm, starts sync service, manages block receiver channel
- **Timed production:** `tokio::time::interval` triggers `produce_block` on the configured cadence
- **Block broadcasting:** `BlockBroadcaster` trait for pushing new blocks to peers
- **Rules:** No business logic of its own. Delegates to core, storage, and network.

---

## Block Flow

### Producer: Creating a Block

```
     Timer fires (every --block-time seconds)
              │
              ▼
      produce_block()
              │
              ├─ 1. Drain mempool (pending transactions)
              │
              ├─ 2. Validate each transaction
              │     (signature, nonce, balance)
              │
              ├─ 3. Execute transactions
              │     (apply state transitions)
              │
              ├─ 4. Build BlockHeader
              │     (parent_hash, state_root,
              │      transactions_root, timestamp, height)
              │
              ├─ 5. write_batch()
              │     (atomically persist block + state)
              │
              └─ 6. Broadcast to peers
                    (libp2p block notification)
```

Key properties:
- The mempool is drained in insertion order
- The transactions root is a BLAKE3 hash commitment to the transaction list
- `write_batch` makes the block application atomic — no partial state on crash
- Broadcasting happens after successful persistence

### Follower: Receiving a Block

```
     Block arrives via P2P
     (notification or sync response)
              │
              ▼
       apply_block()
              │
              ├─ 1. Validate parent linkage
              │     (block.parent_hash == hash of previous block)
              │
              ├─ 2. Validate height
              │     (block.height == previous.height + 1)
              │
              ├─ 3. Validate transactions root
              │     (recompute and compare)
              │
              ├─ 4. Validate each transaction
              │     (signature, nonce, balance)
              │
              ├─ 5. Execute transactions
              │     (apply state transitions)
              │
              └─ 6. write_batch()
                    (atomically persist block + state)
```

Key properties:
- Followers never trust the producer blindly — every block is fully validated
- If any validation step fails, the block is rejected and not persisted
- `apply_block` is the single entry point for all externally received blocks
- The same `write_batch` atomicity guarantees apply

### Bootstrap Sync (New Node)

```
     New node starts
          │
          ▼
     Connect to bootnode(s) via libp2p
          │
          ▼
     Request blocks by height range
     (from local_height + 1 to peer_height)
          │
          ▼
     For each received block:
          apply_block()
          │
          ▼
     Caught up → switch to live notification sync
```

---

## Determinism

Every node processing the same sequence of blocks will arrive at the same state. This is guaranteed by:

1. **SCALE encoding** — deterministic byte serialization (same input always produces same bytes)
2. **BLAKE3 hashing** — deterministic hash function (same bytes always produce same hash)
3. **Ordered execution** — transactions within a block are applied in order
4. **Atomic commits** — `write_batch` ensures no partial state exists

The replay harness (M2.5) independently proves this by exporting blocks from a running producer, replaying them on a completely separate in-memory backend, and asserting that the final tip hash matches.

---

## What Exists vs. What Is Stubbed

| Crate | Status | Description |
|---|---|---|
| mbongo-core | **Implemented** | All types, crypto, encoding |
| mbongo-storage | **Implemented** | RocksDB + InMemory, full Storage trait |
| mbongo-network | **Implemented** | JSON-RPC, REST, libp2p sync + notification |
| mbongo-node | **Implemented** | Full node binary, backend, P2P, timed production |
| mbongo-api | **Implemented** | REST API handlers |
| mbongo-consensus | **Stub** | Placeholder — no consensus protocol yet |
| mbongo-compute | **Stub** | Placeholder — no compute marketplace yet |
| mbongo-verification | **Stub** | Placeholder — no TEE/ZK verification yet |
| mbongo-runtime | **Stub** | Placeholder — no smart contracts yet |
| mbongo-wallet | **Stub** | Placeholder — basic key management only |

---

## Further Reading

- [Developer Onboarding](./DEV_ONBOARDING.md) — Quick start, CLI flags, devnet commands
- [Devnet Stability Report](./DEVNET_STABILITY_REPORT.md) — Freeze documentation and test matrix
- [Protocol Definition v0.1](./specs/PROTOCOL_DEFINITION_v0.1.md) — Canonical protocol specification
- [Architecture Guardrails](./ARCHITECTURE_GUARDRAILS.md) — Layer rules and invariant protections
