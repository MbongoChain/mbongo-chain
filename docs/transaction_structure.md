# Transaction Structure [L2] [PRIMARY]

This document specifies the fundamental Transaction data structure used in Mbongo Chain.

## Overview

Transactions represent user-initiated state changes. Each transaction includes sender and receiver addresses, amount, a nonce to prevent replay, and an ed25519 signature validating authenticity. Transactions support multiple types for different intents.

## Schema

- `tx_type`: One of `Transfer`, `ComputeTask`, or `Stake`.
- `sender`: 32-byte address (ed25519 public key).
- `receiver`: 32-byte address (destination varies by type).
- `amount`: 128-bit unsigned integer.
- `nonce`: 64-bit unsigned integer.
- `signature`: 64-byte ed25519 signature over the signing payload.

### Signing Payload

The signing payload is the SCALE-encoded tuple of `(tx_type, sender, receiver, amount, nonce)`.
The `signature` field itself is not part of the payload.

## Serialization

- **SCALE**: All transaction fields derive SCALE `Encode`/`Decode` for compact, deterministic binary encoding.
- **JSON**: Human-readable serialization via `serde`, with `sender`/`receiver` rendered as hex strings (0x-prefixed).

## Signature Verification

Verification uses ed25519 and the sender's public key:

1. Build the signing payload from the transaction without the signature.
2. Verify the 64-byte ed25519 signature against this payload using the sender's key.

## Merkle-like Root

`transactions_root` in the block header is the Blake3 hash over the concatenation of length-prefixed, SCALE-encoded transactions.

## Transaction Types

- `Transfer`: Standard value transfer from `sender` to `receiver` of `amount`.
- `ComputeTask`: Payment or assignment for compute work. `receiver` identifies the compute provider.
- `Stake`: Staking of `amount` to a staking contract or validator.

## References

- MVP Tasks: `docs/mvp_tasks.md` — Task 2.1.2
- Execution Engine: `docs/execution_engine_overview.md`
- Block Structure: `docs/block_structure.md`
