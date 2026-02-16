# Compute Interface Specification v0.1

**Status:** SPEC ONLY (no implementation in v0.2)
**Related:** [PROTOCOL_LOCK_v0.2.md](./PROTOCOL_LOCK_v0.2.md)
**Target version:** Future v0.3+
**Last updated:** 2026-02-16

---

## 1. Design Goals

This specification defines the compute interface that Mbongo Chain will expose in a future protocol version. It is a **design document only**. No code implementing this spec exists in the v0.2 codebase, and none may be added without an RFC and protocol version bump.

**Goals:**

- **Deterministic verification.** Every compute result must be independently verifiable. Receipts carry a proof blob that allows any node to confirm correctness without re-executing the full workload.
- **Submission / execution separation.** Task submission (on-chain, SCALE-encoded) is decoupled from task execution (off-chain, hardware-specific). The chain records tasks and receipts; it does not run workloads.
- **Block format compatibility.** This spec does NOT modify the v0.2 block structure. Compute tasks and receipts will be introduced as new transaction types or as a dedicated block body section in v0.3+, gated by an RFC.
- **Backward compatibility.** v0.2 nodes must be able to operate on the network during the transition period. Compute-aware messages will use new P2P protocol strings that v0.2 nodes ignore.

**Non-goals (v0.1 of this spec):**

- Defining the verification algorithm (redundant execution vs TEE vs ZK-ML).
- Specifying fee auction or pricing mechanics.
- Defining cross-chain bridge interfaces.

---

## 2. Core Types (Future SCALE Encodable)

All types below are expressed as Rust-like pseudo-structs. When implemented, every field must derive `parity_scale_codec::{Encode, Decode}` in the canonical field order shown here. Adding, removing, or reordering fields is a breaking change subject to the [protocol lock](./PROTOCOL_LOCK_v0.2.md).

### ComputeTaskType

```rust
enum ComputeTaskType {
    AIInference,
    ZKProof,
    Rendering,
    Generic,
}
```

### ComputeTask

```rust
struct ComputeTask {
    /// Unique task identifier: BLAKE3(SCALE_encode(self without task_id)).
    task_id: Hash,
    /// Category of compute work requested.
    task_type: ComputeTaskType,
    /// BLAKE3 hash of the input payload (stored off-chain).
    input_hash: Hash,
    /// Optional model identifier for AI inference tasks.
    model_id: Option<Hash>,
    /// Maximum fee the submitter is willing to pay (in MBO base units).
    max_fee: u64,
    /// Deadline expressed as a block height. Task expires if not completed by this height.
    deadline: u64,
    /// Address of the account submitting the task.
    submitter: Address,
    /// Replay-protection nonce (same semantics as transfer nonce).
    nonce: u64,
}
```

### ComputeReceipt (Verifiable Work Proof)

```rust
struct ComputeReceipt {
    /// The task this receipt fulfills.
    task_id: Hash,
    /// Address of the node that executed the task.
    executor: Address,
    /// BLAKE3 hash of the output payload (stored off-chain).
    output_hash: Hash,
    /// Wall-clock execution time in milliseconds (self-reported, subject to verification).
    compute_time_ms: u64,
    /// Identifier of the hardware that performed the work (TEE attestation ID or self-declared).
    hardware_id: Hash,
    /// Opaque proof blob. Contents depend on verification strategy (redundant hash, TEE attestation, ZK proof).
    proof_blob: Vec<u8>,
    /// Ed25519 signature over SCALE_encode(all fields except signature).
    signature: [u8; 64],
}
```

Receipt verification logic is TBD. The verification strategy (redundant execution, TEE attestation, or ZK-ML proof) will be specified in a dedicated RFC before implementation.

### ComputeStatus

```rust
enum ComputeStatus {
    /// Task submitted, awaiting assignment.
    Pending,
    /// Task assigned to an executor node.
    Assigned,
    /// Execution in progress.
    Executing,
    /// Executor submitted a receipt; awaiting verification.
    Completed,
    /// Execution failed or timed out.
    Failed,
    /// Receipt verified; reward distributed.
    Verified,
    /// Fraud detected; executor slashed.
    Slashed,
}
```

---

## 3. RPC Surface Reservation

The following JSON-RPC method names are **reserved** for the compute interface. They are not implemented in v0.2 and MUST return standard JSON-RPC error `-32601` ("Method not found") if called against a v0.2 node.

| Method | Params | Returns | Description |
|--------|--------|---------|-------------|
| `submit_compute_task` | `{ task: string }` (hex-encoded SCALE) | `{ task_id: string }` | Submit a compute task for execution |
| `get_compute_task` | `{ task_id: string }` | `ComputeTask` JSON | Retrieve a submitted task by ID |
| `get_compute_receipt` | `{ task_id: string }` | `ComputeReceipt` JSON | Retrieve the receipt for a completed task |
| `list_compute_tasks` | `{ status: string, limit: u32 }` | `Vec<ComputeTask>` JSON | List tasks filtered by status |
| `get_compute_node_status` | `{ address: string }` | `{ tasks_completed: u64, reputation: u64 }` | Query a compute provider's status |

These reservations prevent name collisions. The exact parameter and return shapes may change before implementation; any such change will be documented in the implementing RFC.

---

## 4. Event Model (Future Block Events)

The following event types are reserved for the compute lifecycle. They are not emitted in v0.2.

| Event | Fields | Trigger |
|-------|--------|---------|
| `TaskSubmitted` | `task_id`, `submitter`, `task_type`, `max_fee` | `submit_compute_task` accepted into mempool |
| `TaskAssigned` | `task_id`, `executor` | Scheduler assigns task to a compute provider |
| `TaskCompleted` | `task_id`, `executor`, `output_hash` | Executor submits a receipt |
| `TaskVerified` | `task_id`, `executor`, `reward` | Verification passes; reward distributed |
| `FraudDetected` | `task_id`, `executor`, `evidence_hash` | Verification fails; slashing triggered |

The event encoding format and delivery mechanism (block body extension, log entries, or separate receipt trie) will be defined in the implementing RFC.

---

## 5. Economic Placeholders

The following parameters are placeholders. All values are proposals only and require RFC approval before activation.

| Parameter | Proposed value | Status |
|-----------|---------------|--------|
| `compute_fee_multiplier` | TBD | Requires economic modeling |
| `reward_split` | 70% validators / 20% compute providers / 10% treasury | Proposed, not final |
| `slashing_rate` | TBD | Requires security analysis |
| `task_timeout_penalty` | TBD | Requires simulation data |
| `minimum_stake_for_compute` | TBD | Requires economic modeling |

No economic parameters will be hard-coded until the governing RFC is accepted. Placeholder constants may appear in future spec revisions but will have no on-chain effect until activated by a protocol upgrade.

---

## 6. Compatibility Guarantees

This specification introduces **zero changes** to the v0.2 protocol. Explicitly:

| v0.2 surface | Changed by this spec? |
|--------------|-----------------------|
| Block header/body SCALE encoding | No |
| Transaction SCALE encoding | No |
| BLAKE3 hashing rules | No |
| `apply_block` validity rules | No |
| Atomic `write_batch` requirement | No |
| P2P wire formats (`SyncRequest`, `SyncResponse`, `SyncNotification`) | No |
| Protocol negotiation strings (`/mbongo-sync/1`, `/mbongo/block_notify/0.1.0`) | No |
| RPC method names/params/return types in rpc_v0.1 | No |
| Storage trait semantics | No |

Future compute integration will require:

1. An RFC filed under [RFC_PROCESS.md](../RFC_PROCESS.md).
2. A protocol version bump (v0.2 to v0.3 at minimum).
3. A new or updated protocol lock document.
4. A migration plan for v0.2 nodes that do not upgrade immediately.

---

## 7. Versioning Plan

| Version | Compute status | Scope |
|---------|---------------|-------|
| **v0.2** (current) | No compute | Transfer-only chain, single producer, block sync, devnet stable |
| **v0.3** (planned) | Additive extension | Compute task/receipt types added to block body. Reserved RPC methods activated. New P2P protocol string for compute messages. v0.2 nodes can still sync blocks but ignore compute fields. |
| **v1.0** (target) | Full integration | Verification strategy finalized (TEE and/or ZK-ML). Economic parameters activated. Compute provider registration on-chain. Fraud proof / slashing live. |

The v0.2 to v0.3 transition is the critical boundary. It must be gated by an RFC, reviewed by Core Maintainers, and accompanied by a new protocol lock document as defined in [PROTOCOL_LOCK_v0.2.md](./PROTOCOL_LOCK_v0.2.md).
