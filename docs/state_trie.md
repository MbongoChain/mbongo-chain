# State Trie (Merkle Patricia Trie)

This document describes the Merkle Patricia Trie (MPT) used for state storage in Mbongo Chain.

- Hexary Patricia structure with node types: Branch (16-way), Extension, Leaf
- Node serialization: SCALE-encoded for compactness and speed
- Hash function: BLAKE3 over node encoding (32 bytes), yielding `Hash`
- Operations: insert, get, delete in O(length(key)) time, where key length (in nibbles) determines the trie path depth
- Proofs: path proofs are sequences of `(node_hash, node_bytes)` from root to target

## Interface

In `mbongo-core` crate, module `storage::trie` exposes:
- `MerklePatriciaTrie::with_memory()` – in-memory store
- `MerklePatriciaTrie::with_rocksdb(path)` – persistent store (behind optional `rocksdb` feature)
- `insert(key: &[u8], value: Vec<u8>)`
- `get(key: &[u8]) -> Option<Vec<u8>>`
- `delete(key: &[u8]) -> bool`
- `root() -> Hash`
- `get_proof(key: &[u8]) -> Option<Vec<ProofNode>>`

`ProofNode` contains the node hash and raw SCALE-encoded node bytes, allowing independent verification.

## Benchmarks

Criterion benchmarks are provided in `crates/mbongo-core/benches/state_trie_bench.rs`.
Run:

```pwsh
cargo bench -p mbongo-core --bench state_trie_bench
```

## Notes

- The design is compatible in spirit with Ethereum's MPT but uses SCALE and BLAKE3.
- RocksDB support is optional via feature `rocksdb` of `mbongo-core`.
- Future work: parallelized hashing, batched updates, and compact proofs.
