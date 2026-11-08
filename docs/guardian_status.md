# Mbongo Chain — Guardian Node Status

> **HISTORICAL, NON-NORMATIVE.** Banner added 2026-11-08; the text below it is unchanged.
>
> - **Nothing here is implemented.** The runtime has no consensus protocol. One node,
>   started with `--producer`, builds every block, and blocks carry no producer
>   signature. `crates/mbongo-consensus` is an empty stub. There is no PoX, PoS, PoUW,
>   PoC or AIDA, and no VRF leader election, validator set, staking, slashing, finality
>   mechanism or guardian node.
> - **Status lines and checkmarks below are historical.** "Canonical", "Final
>   Specification" or a feature marked as done does not describe the running system.
> - **No parameter here is adopted:** not the block or slot time, epoch length, quorum,
>   finality depth, validator count, stake minimum, slashing rate or unbonding period.
> - **PoUW is obsolete.** Validators executing workloads, and PoUW weights or rewards, are
>   withdrawn by [`VISION_v2.md`](VISION_v2.md) P1, and consensus outcomes must be
>   deterministic (P3).
> - **What is current:** the authority map, [`INDEX.md`](INDEX.md), and the protocol
>   lock, [`specs/PROTOCOL_LOCK_v0.4.md`](specs/PROTOCOL_LOCK_v0.4.md). Any change to
>   block production or validation needs an accepted RFC under [`RFC_PROCESS.md`](RFC_PROCESS.md).

This document describes the Guardian node role in Mbongo Chain, a planned lightweight node type designed for header validation, checkpoint verification, and serving light clients.

---

## 1. Guardian Node Role (Planned)

### Overview

A **Guardian node** is a lightweight node that validates block headers without executing transactions. Guardians bridge the gap between full nodes and light clients, providing a trust-minimized relay layer.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        GUARDIAN NODE                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  What Guardians DO:                                                         │
│  ├── Validate block headers (signatures, parent hash, timestamps)          │
│  ├── Verify finality checkpoints                                           │
│  ├── Maintain peer connections to full nodes                               │
│  ├── Serve header data to light clients                                    │
│  └── Optionally verify PoUW receipt inclusion proofs                       │
│                                                                             │
│  What Guardians DO NOT:                                                     │
│  ├── Execute transactions                                                  │
│  ├── Maintain full state                                                   │
│  ├── Produce blocks                                                        │
│  └── Participate in consensus voting                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Characteristics

| Property | Value |
|----------|-------|
| **Storage** | ~10 GB (headers only) |
| **Bandwidth** | Low-Medium |
| **CPU** | Minimal |
| **Trust Model** | Header chain + checkpoints |
| **Consensus Role** | None (observer only) |

### Header-Only Validation

Guardians verify headers without block bodies:

```rust
// Conceptual header validation
impl Guardian {
    fn validate_header(&self, header: &BlockHeader) -> Result<(), ValidationError> {
        // 1. Verify parent exists
        if !self.header_chain.contains(&header.parent_hash) {
            return Err(ValidationError::UnknownParent);
        }
        
        // 2. Verify height is sequential
        let parent = self.header_chain.get(&header.parent_hash)?;
        if header.height != parent.height + 1 {
            return Err(ValidationError::InvalidHeight);
        }
        
        // 3. Verify timestamp bounds
        if header.timestamp <= parent.timestamp {
            return Err(ValidationError::InvalidTimestamp);
        }
        
        // 4. Verify proposer signature
        let proposer = self.get_slot_proposer(header.slot)?;
        if !verify_signature(&proposer.pubkey, &header.hash(), &header.signature) {
            return Err(ValidationError::InvalidSignature);
        }
        
        Ok(())
    }
}
```

### Checkpoint Verification

Guardians track finality checkpoints:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   CHECKPOINT VERIFICATION                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Receive checkpoint from trusted source (full node, hard-coded)         │
│                                                                             │
│  2. Verify checkpoint signature(s)                                          │
│     • Must have >2/3 validator stake attestation                           │
│     • Signatures must be valid                                              │
│                                                                             │
│  3. Verify checkpoint is descendant of previous checkpoint                  │
│     • Follow parent chain from new to old                                   │
│     • Reject if ancestry broken                                             │
│                                                                             │
│  4. Update local finalized state                                            │
│     • Mark checkpoint as finalized                                          │
│     • Prune headers before checkpoint (optional)                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Optional PoUW Receipt Validation

Guardians can optionally verify compute receipt inclusion:

| Mode | Description | Use Case |
|------|-------------|----------|
| **Skip** | Trust full nodes for PoUW | Minimal resource usage |
| **Inclusion** | Verify Merkle proof only | Medium trust |
| **Full** | Verify proof correctness | Maximum security |

### Slashable Misbehavior

*Status: Placeholder for future implementation*

When guardian staking is implemented, misbehavior will be slashable:

| Violation | Description | Penalty |
|-----------|-------------|---------|
| Invalid Header Relay | Forwarding headers with bad signatures | Stake slash |
| Checkpoint Equivocation | Attesting to conflicting checkpoints | Full slash |
| Data Withholding | Selectively dropping valid data | Reputation penalty |

---

## 2. Responsibilities

### Verify Headers

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   HEADER VERIFICATION                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  For each received header:                                                  │
│                                                                             │
│  □ Parent hash matches known header                                        │
│  □ Height is parent.height + 1                                             │
│  □ Timestamp > parent.timestamp                                            │
│  □ Timestamp < now + MAX_FUTURE_TIME                                       │
│  □ Slot number is valid for timestamp                                      │
│  □ Proposer signature is valid                                             │
│  □ Proposer is assigned to this slot                                       │
│  □ Header hash is correctly computed                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Check Signatures

All cryptographic signatures are verified:

| Signature Type | Verification |
|----------------|--------------|
| Block proposer | Ed25519 over header hash |
| Attestations | Aggregated BLS (planned) |
| Checkpoint | Multi-sig from validators |

### Maintain Peer Score

Guardians track peer behavior:

```rust
// Conceptual peer scoring for guardians
struct GuardianPeerScore {
    /// Base score (starts at 100)
    score: i32,
    
    /// Headers received
    headers_received: u64,
    
    /// Invalid headers received
    invalid_headers: u64,
    
    /// Response latency (moving average)
    avg_latency_ms: u32,
    
    /// Last activity timestamp
    last_seen: Timestamp,
}

impl GuardianPeerScore {
    fn update_on_header(&mut self, valid: bool, latency_ms: u32) {
        self.headers_received += 1;
        
        if valid {
            self.score = (self.score + 1).min(100);
        } else {
            self.invalid_headers += 1;
            self.score -= 20;  // Significant penalty for invalid data
        }
        
        // Update latency average
        self.avg_latency_ms = (self.avg_latency_ms * 9 + latency_ms) / 10;
    }
    
    fn should_disconnect(&self) -> bool {
        self.score <= 0 || self.invalid_headers > 5
    }
}
```

### Forward Valid Data

Guardians relay validated headers to connected clients:

```
  Full Node A          Guardian            Light Client
       │                  │                     │
       │── NewHeader ────▶│                     │
       │                  │                     │
       │                  │ (validate)          │
       │                  │                     │
       │                  │── NewHeader ───────▶│
       │                  │                     │
```

### Drop Invalid Data

Invalid data is discarded and the source penalized:

| Data Type | Invalid Condition | Action |
|-----------|-------------------|--------|
| Header | Bad signature | Drop, penalize peer |
| Header | Unknown parent | Request parent first |
| Checkpoint | Insufficient attestations | Drop, log warning |
| Checkpoint | Conflicts with finalized | Drop, ban peer |

### Report Invalid Peers

*Status: Placeholder for future implementation*

Guardians will report misbehaving peers to a reputation system:

```rust
// Conceptual peer reporting
struct PeerReport {
    /// Reporting guardian
    reporter: GuardianId,
    
    /// Accused peer
    accused: PeerId,
    
    /// Type of misbehavior
    violation: ViolationType,
    
    /// Evidence (e.g., conflicting signed headers)
    evidence: Vec<u8>,
    
    /// Timestamp
    reported_at: Timestamp,
}

enum ViolationType {
    InvalidSignature,
    CheckpointEquivocation,
    DataWithholding,
    ProtocolViolation,
}
```

---

## 3. Node Architecture

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     GUARDIAN NODE ARCHITECTURE                              │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌───────────────────────────────────────────────────────────────────────┐
  │                        GUARDIAN NODE                                  │
  ├───────────────────────────────────────────────────────────────────────┤
  │                                                                       │
  │  ┌─────────────────────────────────────────────────────────────────┐ │
  │  │                    NETWORKING LAYER                             │ │
  │  │                                                                 │ │
  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │ │
  │  │  │ Full Node   │  │ Full Node   │  │ Full Node   │             │ │
  │  │  │ Connection  │  │ Connection  │  │ Connection  │  ...        │ │
  │  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │ │
  │  │         │                │                │                     │ │
  │  │         └────────────────┼────────────────┘                     │ │
  │  │                          │                                      │ │
  │  │  ┌─────────────┐  ┌──────▼──────┐  ┌─────────────┐             │ │
  │  │  │ Light       │  │  Message    │  │ Light       │             │ │
  │  │  │ Client Srv  │◀─│  Router     │─▶│ Client Srv  │  ...        │ │
  │  │  └─────────────┘  └─────────────┘  └─────────────┘             │ │
  │  └─────────────────────────────────────────────────────────────────┘ │
  │                              │                                       │
  │  ┌───────────────────────────▼─────────────────────────────────────┐ │
  │  │                    VALIDATION LAYER                             │ │
  │  │                                                                 │ │
  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │ │
  │  │  │  Header     │  │ Checkpoint  │  │   Peer      │             │ │
  │  │  │  Validator  │  │  Verifier   │  │   Scorer    │             │ │
  │  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │ │
  │  │         │                │                │                     │ │
  │  │         └────────────────┼────────────────┘                     │ │
  │  │                          │                                      │ │
  │  └──────────────────────────┼──────────────────────────────────────┘ │
  │                             │                                        │
  │  ┌──────────────────────────▼──────────────────────────────────────┐ │
  │  │                    STORAGE LAYER                                │ │
  │  │                                                                 │ │
  │  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │ │
  │  │  │  Header Chain   │  │  Checkpoint     │  │  Peer State     │ │ │
  │  │  │  (SQLite/LevelDB)│  │  Store         │  │  (In-Memory)    │ │ │
  │  │  └─────────────────┘  └─────────────────┘  └─────────────────┘ │ │
  │  │                                                                 │ │
  │  └─────────────────────────────────────────────────────────────────┘ │
  │                                                                       │
  └───────────────────────────────────────────────────────────────────────┘
```

### Networking Layer

Guardians maintain two types of connections:

| Direction | Connection Type | Purpose |
|-----------|-----------------|---------|
| Upstream | Full nodes | Receive headers, checkpoints |
| Downstream | Light clients | Serve headers, proofs |

### Minimal Storage

Storage requirements are significantly reduced:

| Data | Size Estimate | Retention |
|------|---------------|-----------|
| Headers | ~500 bytes each | All (prunable) |
| Checkpoints | ~2 KB each | All |
| Peer state | ~1 KB per peer | Session only |
| **Total (1M blocks)** | **~500 MB** | — |

### Header Chain

```rust
// Conceptual header chain storage
struct HeaderChain {
    /// Header by hash
    by_hash: HashMap<Hash, BlockHeader>,
    
    /// Hash by height (canonical chain)
    by_height: BTreeMap<u64, Hash>,
    
    /// Current tip
    tip: Hash,
    
    /// Tip height
    height: u64,
}

impl HeaderChain {
    fn insert(&mut self, header: BlockHeader) -> Result<(), ChainError> {
        // Verify parent exists
        if !self.by_hash.contains_key(&header.parent_hash) {
            return Err(ChainError::MissingParent);
        }
        
        let hash = header.hash();
        self.by_hash.insert(hash, header.clone());
        
        // Update canonical chain if this extends tip
        if header.parent_hash == self.tip {
            self.by_height.insert(header.height, hash);
            self.tip = hash;
            self.height = header.height;
        }
        
        Ok(())
    }
}
```

### Checkpoint Chain

Finalized checkpoints form a separate chain:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     CHECKPOINT CHAIN                                        │
└─────────────────────────────────────────────────────────────────────────────┘

  Genesis         Checkpoint 1      Checkpoint 2      Checkpoint 3
     │                 │                 │                 │
     ▼                 ▼                 ▼                 ▼
  ┌─────┐          ┌─────┐          ┌─────┐          ┌─────┐
  │  0  │─────────▶│1000 │─────────▶│2000 │─────────▶│3000 │
  └─────┘          └─────┘          └─────┘          └─────┘
     │                 │                 │                 │
     └─── Finalized ───┴─── Finalized ───┴─── Finalized ───┘
```

### Peer Sampling

Guardians sample peers for data availability:

```rust
// Conceptual peer sampling
impl Guardian {
    fn sample_peers(&self) -> Vec<PeerId> {
        let mut candidates: Vec<_> = self.peers
            .iter()
            .filter(|(_, score)| score.score > 50)
            .collect();
        
        // Sort by score (descending)
        candidates.sort_by_key(|(_, score)| -score.score);
        
        // Return top N peers
        candidates
            .into_iter()
            .take(self.config.sample_size)
            .map(|(id, _)| *id)
            .collect()
    }
}
```

---

## 4. Supported Message Types

### Message Catalog

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     GUARDIAN MESSAGE TYPES                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Header Messages                                                            │
│  ───────────────                                                            │
│  • NewHeaderAnnounce    - Announce new header (hash only)                  │
│  • GetHeaders           - Request header range                              │
│  • Headers              - Response with header data                         │
│  • GetHeaderByHash      - Request specific header                           │
│  • HeaderByHash         - Response with single header                       │
│                                                                             │
│  Peer Discovery                                                             │
│  ──────────────                                                             │
│  • GetPeers             - Request peer list                                 │
│  • Peers                - Response with peer addresses                      │
│  • PeerAnnounce         - Announce self to network                          │
│                                                                             │
│  Checkpoint Sync                                                            │
│  ───────────────                                                            │
│  • GetCheckpoints       - Request checkpoint range                          │
│  • Checkpoints          - Response with checkpoint data                     │
│  • NewCheckpoint        - Announce new finalized checkpoint                 │
│                                                                             │
│  Liveness                                                                   │
│  ────────                                                                   │
│  • Ping                 - Liveness probe                                    │
│  • Pong                 - Liveness response                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Message Formats

```rust
// Conceptual message definitions
enum GuardianMessage {
    // Header messages
    NewHeaderAnnounce { hash: Hash, height: u64 },
    GetHeaders { start_height: u64, count: u32 },
    Headers { headers: Vec<BlockHeader> },
    GetHeaderByHash { hash: Hash },
    HeaderByHash { header: Option<BlockHeader> },
    
    // Peer discovery
    GetPeers { max_count: u32 },
    Peers { peers: Vec<PeerInfo> },
    PeerAnnounce { info: PeerInfo },
    
    // Checkpoint sync
    GetCheckpoints { start_epoch: u64, count: u32 },
    Checkpoints { checkpoints: Vec<Checkpoint> },
    NewCheckpoint { checkpoint: Checkpoint },
    
    // Liveness
    Ping { nonce: u64 },
    Pong { nonce: u64 },
}
```

### Message Flow Examples

**Header Sync:**
```
  Full Node                Guardian               Light Client
      │                       │                        │
      │── NewHeaderAnnounce ─▶│                        │
      │   (hash: 0x123)       │                        │
      │                       │                        │
      │◀── GetHeaderByHash ───│                        │
      │    (hash: 0x123)      │                        │
      │                       │                        │
      │── HeaderByHash ──────▶│                        │
      │   (header data)       │                        │
      │                       │                        │
      │                       │ (validate)             │
      │                       │                        │
      │                       │── NewHeaderAnnounce ──▶│
      │                       │   (hash: 0x123)        │
      │                       │                        │
```

**Checkpoint Sync:**
```
  Full Node                Guardian
      │                       │
      │── NewCheckpoint ─────▶│
      │   (epoch: 100)        │
      │                       │
      │                       │ (verify attestations)
      │                       │ (verify ancestry)
      │                       │
      │                       │ (update finalized state)
      │                       │
```

---

## 5. Security & Limitations

### Non-Consensus Role

Guardians are explicitly **non-consensus** nodes:

| Capability | Full Node | Guardian | Light Client |
|------------|-----------|----------|--------------|
| Validate headers | ✓ | ✓ | ✗ |
| Validate blocks | ✓ | ✗ | ✗ |
| Execute transactions | ✓ | ✗ | ✗ |
| Produce blocks | ✓ | ✗ | ✗ |
| Vote in consensus | ✓ | ✗ | ✗ |
| Serve headers | ✓ | ✓ | ✗ |
| Serve state proofs | ✓ | ✗ | ✗ |

### Cannot Validate Blocks Fully

Guardians cannot verify:

- Transaction execution correctness
- State root validity
- PoUW proof correctness (without optional mode)
- Balance changes
- Contract execution

### Cannot Produce Blocks

Guardians lack the state needed for block production:

- No mempool (no transaction ordering)
- No state (no execution capability)
- No validator keys (no signing authority)

### Useful Applications

| Application | Benefit |
|-------------|---------|
| **Monitoring** | Track chain progress without full resources |
| **Light Client Server** | Serve headers to mobile/browser clients |
| **Bridge Relay** | Relay headers to other chains |
| **Archive Access** | Provide header history without full state |

### Must Reject Invalid Checkpoints

Critical security rule:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   CHECKPOINT REJECTION RULES                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  MUST REJECT checkpoint if:                                                 │
│                                                                             │
│  □ Signature verification fails                                            │
│  □ Insufficient validator stake (<2/3)                                     │
│  □ Conflicts with already-finalized checkpoint                             │
│  □ Not a descendant of previous checkpoint                                  │
│  □ References unknown block hash                                            │
│  □ Epoch number is not sequential                                           │
│                                                                             │
│  On rejection:                                                              │
│  • Log detailed rejection reason                                            │
│  • Penalize sending peer                                                    │
│  • Do NOT update local finalized state                                      │
│  • Alert operator if repeated                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Network Topology

### Full Nodes → Guardian → Light Apps

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     NETWORK TOPOLOGY                                        │
└─────────────────────────────────────────────────────────────────────────────┘


                          CONSENSUS LAYER
  ┌─────────────────────────────────────────────────────────────────────────┐
  │                                                                         │
  │    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐       │
  │    │   Full   │◀──▶│   Full   │◀──▶│   Full   │◀──▶│   Full   │       │
  │    │  Node 1  │    │  Node 2  │    │  Node 3  │    │  Node 4  │       │
  │    │(Validator│    │(Validator│    │          │    │          │       │
  │    └────┬─────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘       │
  │         │               │               │               │             │
  └─────────┼───────────────┼───────────────┼───────────────┼─────────────┘
            │               │               │               │
            │               │               │               │
            ▼               ▼               ▼               ▼
  ┌─────────────────────────────────────────────────────────────────────────┐
  │                                                                         │
  │                         GUARDIAN LAYER                                  │
  │                                                                         │
  │         ┌──────────────┐              ┌──────────────┐                 │
  │         │   Guardian   │◀────────────▶│   Guardian   │                 │
  │         │    Node A    │              │    Node B    │                 │
  │         └──────┬───────┘              └───────┬──────┘                 │
  │                │                              │                         │
  └────────────────┼──────────────────────────────┼─────────────────────────┘
                   │                              │
                   │                              │
     ┌─────────────┼─────────────┐    ┌──────────┼──────────┐
     │             │             │    │          │          │
     ▼             ▼             ▼    ▼          ▼          ▼
  ┌─────────────────────────────────────────────────────────────────────────┐
  │                                                                         │
  │                        LIGHT CLIENT LAYER                               │
  │                                                                         │
  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐      │
  │  │ Mobile  │  │ Browser │  │  DApp   │  │ Wallet  │  │ Bridge  │      │
  │  │   App   │  │   App   │  │ Backend │  │   App   │  │  Relay  │      │
  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘      │
  │                                                                         │
  └─────────────────────────────────────────────────────────────────────────┘


  Data Flow:
  ══════════
  
  Blocks/State    ─────▶  Full Nodes (produce, validate, store)
                              │
  Headers/Checkpoints ───────▶  Guardians (validate, relay)
                                    │
  Headers/Proofs  ──────────────────▶  Light Apps (consume, verify)
```

### Trust Relationships

| Relationship | Trust Level | Verification |
|--------------|-------------|--------------|
| Full Node → Guardian | Medium | Guardians verify all headers |
| Guardian → Light Client | High | Light clients trust guardian validation |
| Guardian → Guardian | Low | Cross-verify via multiple guardians |

---

## 7. Future Extensions

### Guardian Staking

*Status: Placeholder*

Guardians may stake tokens to provide economic guarantees:

```rust
// Conceptual staking structure
struct GuardianStake {
    /// Guardian identifier
    guardian_id: GuardianId,
    
    /// Staked amount
    stake: u128,
    
    /// Slashing conditions
    slashable_until: Epoch,
    
    /// Reward accumulator
    pending_rewards: u128,
}
```

**Benefits:**
- Economic incentive for honest behavior
- Slashing for misbehavior
- Reward distribution for service

### Delegated Verification

*Status: Placeholder*

Light clients may delegate verification to specific guardians:

| Feature | Description |
|---------|-------------|
| Subscription | Light clients subscribe to guardian |
| Push Updates | Guardian pushes new headers |
| Attestation | Guardian attests to header validity |
| Payment | Light client pays guardian for service |

### On-Chain Slashing

*Status: Placeholder*

Misbehaving guardians may be slashed on-chain:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   GUARDIAN SLASHING (FUTURE)                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Slashable Offenses:                                                        │
│  • Relaying headers with invalid signatures                                 │
│  • Attesting to conflicting checkpoints                                     │
│  • Systematic data withholding                                              │
│                                                                             │
│  Slashing Process:                                                          │
│  1. Reporter submits evidence transaction                                   │
│  2. On-chain verification of evidence                                       │
│  3. If valid: slash guardian stake                                          │
│  4. Reporter receives portion of slashed stake                              │
│                                                                             │
│  Slash Amounts:                                                             │
│  • Invalid header relay: 10% of stake                                       │
│  • Checkpoint equivocation: 100% of stake                                   │
│  • Data withholding: 5% of stake (requires proof)                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Additional Roadmap

- [ ] Guardian discovery protocol
- [ ] Header subscription service
- [ ] Proof-of-relay mechanism
- [ ] Cross-guardian consensus
- [ ] Light client SDK

---

## Summary

Guardian nodes provide a lightweight alternative to full nodes for applications that need header validation without full block execution. They bridge the trust gap between full nodes and light clients, enabling efficient data distribution while maintaining security through checkpoint verification.

For full node architecture, see [Node Architecture](node_architecture.md).

For consensus details, see [Consensus Overview](consensus_overview.md).

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

