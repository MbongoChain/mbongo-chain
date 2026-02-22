# RECEIPT_SPEC_v0.1

**Status:** EXPERIMENTAL  
**Stability:** Breaking changes allowed until v1.0-mainnet  
**Lock Target:** v1.0-mainnet  
**Scope:** Receipt Anchoring (v1 minimal primitive)

---

## 1. Overview

This specification defines the minimal v1 primitive for anchoring compute receipts on Mbongo Chain. Receipt anchoring records a cryptographic commitment to off-chain inference results. The chain does not execute workloads, interpret application data, or enforce economic logic. It validates structure, signature, and uniqueness only.

Aligned with [VISION_v1.md](../VISION_v1.md): verification layer only, no on-chain AI execution. Does not modify [PROTOCOL_LOCK_v0.2.md](./PROTOCOL_LOCK_v0.2.md) or existing v0.2 surfaces.

---

## 2. Receipt Structure (Canonical)

```text
Receipt {
    version:   u8,
    task_id:   [u8; 32],
    input_commitment:  [u8; 32],
    output_commitment: [u8; 32],
    executor:  Address,
    metadata:  Vec<u8>,
    signature: Signature
}
```

| Field | Type | Description |
|-------|------|-------------|
| `version` | u8 | Protocol version. Must be 1 for this spec. |
| `task_id` | [u8; 32] | Unique task identifier. Opaque to chain. |
| `input_commitment` | [u8; 32] | Commitment to input (e.g. BLAKE3 of input). Opaque to chain. |
| `output_commitment` | [u8; 32] | Commitment to output (e.g. BLAKE3 of output). Opaque to chain. |
| `executor` | Address | Ed25519 public key of the executor (32 bytes). |
| `metadata` | Vec\<u8\> | Opaque. Never interpreted by the protocol. |
| `signature` | Signature | Ed25519 signature over `receipt_hash`. |

`Address` and `Signature` use the same formats as in [PROTOCOL_DEFINITION_v0.1.md](./PROTOCOL_DEFINITION_v0.1.md).

---

## 3. Canonical Encoding

All on-wire and on-disk serialisation uses SCALE (`parity-scale-codec`).

- Field order is fixed. Adding, removing, or reordering fields is a breaking change.
- `Vec<u8>` is encoded as compact length prefix followed by bytes.
- `[u8; 32]` is encoded as 32 contiguous bytes.
- `Address` is 32 bytes.
- `Signature` is 64 bytes (Ed25519).

Encoding order: `version`, `task_id`, `input_commitment`, `output_commitment`, `executor`, `metadata`, `signature`.

---

## 4. Receipt Hash Computation

The receipt hash is computed over the signing payload (all fields except `signature`):

```text
receipt_hash = BLAKE3(
    SCALE_encode(
        version,
        task_id,
        input_commitment,
        output_commitment,
        executor,
        metadata
    )
)
```

- Hash output: 32 bytes.
- Display format: `0x` + 64 lowercase hex characters.
- The same encoded bytes must always produce the same hash (determinism).

---

## 5. Validation Rules (v1 Minimal)

A receipt is valid if and only if:

1. **Version:** `version == 1`.
2. **Signature:** `signature` is a valid Ed25519 signature over `receipt_hash` by `executor`.
3. **Duplicate:** `task_id` does not exist in storage (see Section 6).
4. **Encoding:** All fields decode correctly under SCALE.

The chain does **not** validate:

- Semantic meaning of `task_id`, `input_commitment`, or `output_commitment`
- Correctness of the underlying computation
- Application-level metadata
- Economic conditions (fees, rewards, slashing)

---

## 6. Duplicate Rule

`task_id` must be globally unique. A receipt is rejected if a receipt with the same `task_id` has already been anchored.

Storage must support:

- Insert: `(task_id, receipt_hash)` or equivalent.
- Lookup: existence check by `task_id` before accepting a new receipt.

Indexing requirement: O(1) or O(log n) lookup by `task_id` for duplicate detection.

---

## 7. Metadata Interpretation

`metadata` is opaque. The protocol never parses, validates, or interprets its contents. It is stored and retrieved as a byte blob. Application layers may define their own metadata schemas; the chain is agnostic.

---

## 8. Out of Scope (v0.1)

The following are **explicitly excluded** from this spec:

- Challenge mechanism
- Slashing
- Dispute resolution
- Proof of Useful Work (PoUW)
- Zero-knowledge proofs (ZK)
- Capital-weighted logic (staking, rewards)
- Fee or payment validation
- Task submission lifecycle
- Executor registration or reputation

---

## 9. Upgrade Path

This spec is experimental until v1.0-mainnet. Breaking changes are allowed without a new spec version until the lock target.

At v1.0-mainnet:

- Receipt structure, encoding, hash computation, and validation rules are locked.
- Changes require an RFC and protocol version bump.
- A new spec document (e.g. `RECEIPT_SPEC_v1.0.md`) will supersede this one and declare FROZEN status.

---

## 10. Design Principles

1. **Minimal surface.** Only what is necessary for receipt anchoring. No economic or semantic logic.
2. **Determinism.** Same receipt bytes → same hash → same validation outcome on every node.
3. **Opaque commitments.** The chain does not interpret `input_commitment` or `output_commitment`. Verification of correctness is an application-layer concern.
4. **Signature binding.** The executor attests to the receipt by signing. No delegation or proxy in v0.1.
5. **Unique anchoring.** One `task_id` per receipt. Prevents replay and duplicate claims.
