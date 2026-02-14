# Phase 1 Architecture

---

## Layer Separation

```
+------------------------------------------------------------------+
|                     mbongo-node (orchestration)                   |
|  - CLI parsing                                                    |
|  - Component lifecycle                                            |
|  - RPC/REST server wiring                                         |
+------------------------------------------------------------------+
|                     mbongo-network (transport)                     |
|  - HTTP server                                                    |
|  - JSON-RPC dispatch                                              |
|  - REST routing                                                   |
+------------------------------------------------------------------+
|                     mbongo-storage (persistence)                   |
|  - RocksDB wrapper                                                |
|  - Column families: blocks, transactions, accounts               |
|  - write_batch atomicity                                          |
+------------------------------------------------------------------+
|                     mbongo-core (pure logic)                       |
|  - Block, Transaction, Account types                              |
|  - SCALE encoding                                                 |
|  - BLAKE3 hashing                                                 |
|  - Signature verification                                         |
|  - No I/O, no network                                             |
+------------------------------------------------------------------+
```

---

## Flow: submit_transaction

```
Client -> HTTP POST /rpc
    -> mbongo-network: parse JSON-RPC request
    -> validate method = "submit_transaction"
    -> decode params[0] as hex -> signed_tx bytes
    -> mbongo-core: decode SCALE Transaction
    -> mbongo-core: verify_signature()
    -> mbongo-storage: get account(sender) for nonce/balance check
    -> mbongo-core: validate nonce, balance
    -> mbongo-storage: insert into mempool (or dedupe)
    -> return { tx_hash }
```

---

## Flow: produce_block

```
Client -> HTTP POST /rpc
    -> mbongo-network: parse JSON-RPC request
    -> validate method = "produce_block"
    -> mbongo-storage: read mempool transactions
    -> mbongo-storage: read latest block (parent_hash, height)
    -> for each tx (in order):
        -> mbongo-core: execute (transfer logic)
        -> mbongo-storage: update account state
    -> mbongo-core: compute state_root, transactions_root
    -> mbongo-core: build BlockHeader, BlockBody
    -> mbongo-storage: write_batch(block, updated accounts, tx indexes)
    -> mbongo-storage: clear included txs from mempool
    -> return { block_hash, height }
```

---

## Explicit Separation

| Layer | Crate | Responsibility |
|-------|-------|----------------|
| Orchestration | mbongo-node | Startup, config, wiring. No business logic. |
| Transport | mbongo-network | HTTP, JSON-RPC, REST. No state. |
| Persistence | mbongo-storage | RocksDB, column families, atomic batches. |
| Logic | mbongo-core | Types, validation, hashing. No I/O. |

mbongo-core has no dependencies on storage or network. It can be tested in isolation.
