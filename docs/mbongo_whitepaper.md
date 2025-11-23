MBONGO CHAIN — WHITEPAPER v1.0

Canonical Release — 2025

0. Abstract

Mbongo Chain is a Rust-native, compute-first Layer 1 blockchain that unifies decentralized consensus with high-performance verifiable computation.

Instead of wasting electricity on useless hashing, Mbongo Chain transforms GPU/TPU/NPU compute into a consensus-strengthening primitive through a hybrid model:

Proof-of-Stake (PoS) for economic security

Proof-of-Useful-Work (PoUW) for verifiable compute

Proof-of-Compute (PoC) for hardware attestation

AIDA, a bounded, deterministic economic regulator

The architecture is designed for:

AI inference

rendering

scientific simulation

zero-knowledge prover acceleration

numerical computing

future accelerator hardware

Mbongo Chain introduces a fixed, time-based monetary model: 31,536,000 MBO, equal to the number of seconds in a year.

The network aims to become the global infrastructure for verifiable decentralized compute.

1. Vision

Modern blockchains suffer from structural constraints:

wasteful PoW mining

limited compute capabilities

centralization of AI workloads

non-verifiable AI outputs

poor hardware utilization

complex or fragmented execution layers

Mbongo Chain introduces a new paradigm:

Useful computation becomes the backbone of consensus, economics, and utility.

The network’s long-term vision is to provide a global open compute fabric, enabling:

decentralized AI

verifiable GPU/TPU/NPU workloads

scalable compute markets

tamper-proof, cryptographically verified compute pipelines

transparent, predictable economics

2. Architecture Overview

Mbongo Chain is built around five pillars:

Rust-native deterministic execution

Hybrid PoS + PoUW consensus

AIDA-regulated economic system

Compute marketplace integrated at protocol level

Monorepo Rust workspace for reliability

The architecture is modular:

node/        → networking, mempool, PoS, block production
runtime/     → deterministic execution engine
pouw/        → compute engine & verification
crypto/      → signatures, VRF, hashing, SMT
wallet/      → key management
sdk/         → TypeScript & Rust SDK
infra/       → deployment, monitoring
tests/       → integration & scenario tests

3. Consensus: PoX (Proof-of-Everything Useful)

Mbongo Chain introduces a hybrid model:

3.1 Proof-of-Stake (PoS)

Validators:

produce blocks

finalize states

verify PoUW receipts

vote on governance proposals

Penalties:

slashing for double-signing

slashing for invalid PoUW acceptance

slashing for censorship

3.2 Proof-of-Useful-Work (PoUW)

PoUW converts real computation into security:

AI inference

rendering

model evaluation

physics simulations

ZK prover acceleration

numerical computing

Compute nodes generate Validatable Work Proof (VWP) receipts that validators verify deterministically.

Mechanisms include:

redundant execution

fraud proofs

deterministic verification

compute reputation system

Compute providers receive:

50% of block rewards

compute fees (C-Gas)

marketplace incentives

3.3 Proof-of-Compute (PoC)

Hardware identity & capability attestation:

GPU/TPU/NPU classification

performance benchmark

hardware uniqueness

anti-fake hardware protection

4. Execution Engine

The execution engine is:

Rust-native

deterministic

gas-metered

modular

reproducible

Two types of gas:

E-Gas

Execution gas for:

state transitions

transfers

staking

governance

VM execution

C-Gas

Compute gas for:

AI jobs

rendering

numerical simulations

ZK proofs

Deterministic Execution Guarantees:

no floating point

BLAKE3 hashing

Sparse Merkle Tree state roots

fixed gas table

structured error model

reproducible outputs

5. State Machine

State transitions follow:

validate(tx)
execute(tx)
compute_state_root()


Modules include:

accounts

balances

staking

governance

compute marketplace

PoUW validator

fee/burn logic

AIDA read-only integration

Every block calculates:

state_root

receipts_root

compute_receipts_root

6. Networking

Built on libp2p with:

GossipSub

peer scoring

anti-Byzantine routing

sync protocol

multi-stream support

compute job gossip

receipt propagation

Networking is optimized for:

low latency

high throughput

compute workload distribution

7. Tokenomics
Total supply:

31,536,000 MBO
(number of seconds in a year)

Block reward:

0.1 MBO / second

Halving:

Every 157,680,000 blocks (~5 years)

Reward split:

50% PoS / 50% PoUW
(adjustable 40/60 → 60/40)

Deflation:

Burn rate controlled by AIDA (0–30%)

Categories:

45% network rewards

20% foundation

10% founders

5% core contributors

10% public sale / partners

10% community / airdrop

Founders Vesting:

4 years vesting
1-year cliff
Monthly unlock

8. AIDA: Autonomous Intelligent Dynamic Adjuster

AIDA is the network’s economic stabilizer.

Controls:

burn rate (0–30%)

base fee multiplier (0.5–3.0)

compute multiplier (0.8–1.2)

priority fee caps

AIDA cannot:

mint tokens

modify supply cap

change emission

alter consensus

modify vesting

override governance

AIDA is governed by:

DAO

Founder Council (10 years)

9. Governance
9.1 DAO

Handles:

parameters

upgrades

treasury

Voting:
Quadratic lock-weight model.

9.2 Founder Council (10-year mandate)

Can veto:

emission changes

supply alterations

AIDA parameter ranges

reward split out of bounds

slashing rule alterations

Cannot:

mint tokens

spend treasury

block normal upgrades

9.3 Upgrade Path

Every upgrade goes through:

Proposal

Safety review (90 days)

AIDA economic analysis

DAO vote

Founder Council veto window

Activation

10. PoUW Marketplace

Users submit tasks:

Submit compute job

Compute node executes

Generate VWP receipt

Validator verifies

Rewards distributed

Task types:

AI inference

ML batching

rendering

physics simulation

ZK proving

Receipts include:

deterministic hash of output

input commitment

hardware ID

compute time

VRF-linked randomness

Marketplace fees use:

C-Gas

AIDA multipliers

priority fees

11. Security

Security appendix summary:

PoS slashing

PoUW fraud proofs

hardware attestation

compute redundancy

SMT roots

AIDA bounded operation

libp2p anti-Byzantine protocols

deterministic execution

cryptographic signatures

Founder Council protection

12. Developer Ecosystem

SDKs:

TypeScript SDK

Rust SDK

Tools:

CLI wallet

RPC API

WebSocket & gRPC streams

local devnet

Docker-based testnet

Monorepo advantages:

synchronized modules

atomic upgrades

unified testing

simpler auditing

accelerated onboarding

13. Roadmap

Short-term (0–12 months):

PoS core

PoUW MVP

AIDA v1

SDKs

RPC API

Devnet

Mid-term (12–36 months):

GPU orchestration

compute subnets

WASM runtime

ZK verification

AIDA v2

Long-term (36+ months):

AI model hosting

GPU/TPU/NPU federation

compute sharding

global verifiable compute layer

14. Future Vision

Over the next decade, Mbongo Chain aims to become:

the backbone of decentralized AI

the global compute marketplace

the standard for verifiable AI inference

the zero-trust compute fabric for the internet

A network where:

Compute = Value
Value = Security
Security = Compute

Conclusion

Mbongo Chain introduces a new era of blockchain design:
compute-native, verifiable, secure, deterministic, and economically stable.

With Rust, PoX consensus, PoUW compute, AIDA economics, and a fixed supply tied to time, Mbongo Chain becomes a foundational infrastructure for decentralized AI and global compute markets.

Whitepaper Completed ✔
Version: Final (v1.0)