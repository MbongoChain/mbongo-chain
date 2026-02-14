# Phase 2 Plan

Phase 2 builds on the frozen Phase 1 foundation. Focus: mempool, timed production, P2P, and multi-node devnet.

---

## Milestone 1 — Mempool

- Mempool structure for pending transactions
- Eviction policy (e.g. by fee or age)
- Size limits
- Integration with submit_transaction and produce_block

---

## Milestone 2 — Timed Block Production

- Configurable block interval
- Background timer triggers produce_block
- No manual RPC call required for block production

---

## Milestone 3 — P2P Propagation

- libp2p integration for block and transaction gossip
- Peer discovery (mDNS for local, DHT for global)
- Receive blocks from peers and persist
- Broadcast produced blocks to peers

---

## Milestone 4 — Fork Handling Minimal

- Track multiple block tips
- Simple longest-chain rule (or highest height)
- Reorg handling: revert state when switching chain

---

## Milestone 5 — Devnet Multi-Node

- Multi-node testnet setup
- Bootstrap from genesis
- Sync from peers
- Validation that multiple nodes converge to same chain

---

## Non-Goals (Phase 2)

- Full consensus protocol
- Finality gadget
- Economic penalties
- Gas model
