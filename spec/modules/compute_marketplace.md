Version: Canonical
Status: Final
Author: Mbongo Foundation

The Mbongo Compute Marketplace is a decentralized, verifiable, AI-native compute exchange layer integrated directly into the L1 runtime. It allows users and dApps to submit compute tasks that GPU/TPU/NPU/ASIC operators execute under PoUW (Proof-of-Useful-Work), producing verifiable proofs and earning rewards.

This document defines:

marketplace architecture

task lifecycle

incentive design

fee distribution

compute verification

economic integration

security model

1. Purpose

The Compute Marketplace provides:

A decentralized marketplace for AI and compute workloads

Guaranteed verifiable execution

Deterministic fee and reward rules

Transparent matching between users and compute nodes

Incentives aligned with network security

It transforms Mbongo Chain into a global AI computation layer.

2. Marketplace Architecture

The marketplace consists of 5 major components:

Task Layer — users submit compute requests

Scheduler — assigns tasks to eligible nodes

Compute Nodes (PoUW) — execute jobs and produce proofs

Verification Layer (PoS Validators) — validate compute proofs

Settlement Layer — reward distribution + fee processing

3. Task Types

Marketplace supports multiple categories:

AI Inference (ONNX, TensorRT, GGML, etc.)

ML Micro-Training (small updates, gradient checks)

ZK Proof Generation

3D Rendering

High-performance math (BLAS, FFT, etc.)

Scientific simulations

Generic GPU compute

Each type has:

deterministic mode (preferred)

redundancy mode (when determinism is not guaranteed)

4. Task Lifecycle

The lifecycle ensures fairness, verifiability, and efficient execution.

4.1 Submission

User submits:

input data (hashed)

model/config

max fee

deadline

category

The transaction enters the mempool.

4.2 Scheduling

Scheduler matches tasks to compute nodes using:

PoC score (hardware capability)

reputation

latency

task category specialization

availability

Selection formula:

NodeScore = w1*PoC + w2*Reputation + w3*Availability - w4*Penalty


Weighted scoring ensures fairness across hardware classes.

4.3 Execution

Compute node:

Executes compute container

Produces a Compute Proof

Stores output in temporary off-chain storage

Submits proof + output hash to L1

4.4 Verification

PoS validators verify:

proof validity

redundancy (if >1 nodes executed task)

timing constraints

reputation adjustments

slashing if fraud detected

Redundancy formula:

R >= 2    (minimum redundancy)


Future: ZK-proofs may reduce R → 1.

4.5 Settlement

Rewards are distributed as:

Validator Share:
20% of marketplace fees

Compute Provider Share:
70% of marketplace fees + 50% of block reward when participating in PoUW

Treasury Share:
10% of marketplace fees

AIDA Burn:

AIDA may burn 0–30% of fees only, not rewards.

Formula:

burn_amount = fee * burn_rate


Remaining fee distributed according to 70/20/10.

5. Fee Model

Fees are structured as follows:

5.1 Base Fee (Mandatory)

Paid by task submitter to cover:

block space

compute scheduling

validator verification

5.2 Compute Fee

Paid per compute cycle:

determined by market supply/demand

bounded by DAO parameters

5.3 AIDA Dynamic Fee Adjustment

AIDA may adjust:

gas multiplier (1.0 → 3.0)

compute fee multiplier (0.8 → 1.2)

fee burn rate (0% → 30%)

This prevents congestion, spam and speculation.

6. Hardware Scoring (PoC)

PoC ensures fairness across hardware:

Measured attributes:

TFLOPs performance

VRAM bandwidth

Driver stability

Error rate

Uptime

Reputation score

PoC determines:

eligibility

reward multiplier

scheduling priority

redundancy adjustment

7. Reputation System

Key reputation metrics:

successful tasks

failure rate

fraud attempts

latency

uptime

hardware reliability

Reputation evolves:

Reputation_new = Reputation_old + α*Success - β*Failures - γ*Penalties


Bad behavior → exponential decay
Good behavior → slow, stable growth

8. Slashing (PoUW Fraud)

If a compute node submits invalid proofs:

loss of current reward

reputation slash

increased penalty coefficient

potential permanent ban

stake slashing (if staked compute)

Validators receive proof-of-fraud bonuses.

9. Security Model

The Compute Marketplace is secured by:

✔ PoS validator verification

Ensures strong economic finality.

✔ PoC hardware diversity

Prevents single hardware class takeover.

✔ Redundant execution

Eliminates fraudulent output.

✔ Compute Proofs

Cryptographic assurance.

✔ Slashing

Punishes bad behavior.

✔ Fixed supply

Prevents inflation-based attacks.

✔ AIDA risk management

Predictive modeling against:

compute overload

fee congestion

spam attacks

sudden demand shocks

✔ Founder Council (10 years)

Prevents governance capture.

10. Interaction With Emission & Rewards

The compute marketplace reinforces the economic model:

10.1 Block Reward Contribution

50% of R(t) → compute nodes
Ensures stable long-term revenue beyond marketplace fees.

10.2 Marketplace Fees

Make PoUW profitable even after emissions decline.

10.3 Scarcity Effects

MBO scarcity increases economic value of running compute nodes.

10.4 DAO & Treasury

10% of fees ensure sustainable ecosystem funding without inflation.

11. Marketplace API Events (Simplified)
TaskSubmitted(task_id, user, category, fee)
TaskAssigned(task_id, node_id)
TaskCompleted(task_id, node_id, proof_hash)
TaskVerified(task_id, validator_id)
RewardsDistributed(task_id)
PenaltyApplied(node_id)


These events help explorers, wallets, and AI agent clients interact with the network.

12. Summary

The Mbongo Compute Marketplace:

transforms the blockchain into a global verifiable compute engine

securely integrates GPU/TPU/NPU/ASIC computing

provides stable incentives for compute providers

uses redundancy and cryptographic proofing for correctness

aligns with PoS rewards

integrates AIDA for dynamic fee management

ensures long-term sustainability

This design makes Mbongo Chain the first blockchain with:

AI-native economics

verifiable compute at L1

compute-secured consensus

multi-hardware participation

governance-protected long-term evolution
