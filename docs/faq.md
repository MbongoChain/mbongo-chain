# Mbongo Chain — Technical FAQ  
Status: Canonical  
Version: v1.0

This FAQ provides high-level and technical answers to the most common questions about Mbongo Chain’s architecture, economics, governance, development model, and compute ecosystem.

---

# 1. General Questions

---

### **Q1. What is Mbongo Chain?**  
Mbongo Chain is a Rust-native, compute-first Layer 1 blockchain combining:

- Proof-of-Stake (PoS)  
- Proof-of-Useful-Work (PoUW)  
- Proof-of-Compute (PoC)  
- AIDA economic regulator  

The network transforms real computation (AI, GPU, TPU, simulation, rendering) into a security primitive.

---

### **Q2. What makes Mbongo Chain different from other blockchains?**  
Key innovations:

- verifiable compute (PoUW)  
- deterministic Rust runtime  
- GPU-first execution model  
- AIDA dynamic economic regulator  
- hybrid PoS + PoUW consensus  
- fixed supply tied to time (31,536,000 MBO)  
- compute marketplace integrated at protocol level  

No wasteful mining.  
All work is useful.

---

### **Q3. Is Mbongo Chain based on Cosmos, Substrate, or another framework?**  
No.  
It is a **fully custom Rust L1**:

- custom consensus  
- custom runtime  
- custom PoUW engine  
- custom networking layer  
- custom tokenomics  

Built from scratch for compute-first workloads.

---

# 2. Architecture & Execution

---

### **Q4. Why did you choose Rust?**  
Rust provides:

- memory safety  
- deterministic execution  
- high performance  
- no garbage collector  
- safe concurrency  
- predictable behavior  

Rust is ideal for consensus, state machines, and GPU compute integration.

---

### **Q5. How is the execution engine deterministic?**  

- no floating-point use  
- fixed gas metering  
- fixed hashing (BLAKE3)  
- strict module isolation  
- reproducible state transitions  
- same input → same output → same state root  

---

### **Q6. Do you support smart contracts?**  
Yes — through a **future WASM module**, with:

- deterministic `no_std` Rust contracts  
- bounded memory  
- fixed gas tables  
- deterministic host functions  

---

### **Q7. How does PoUW computation work?**  

1. Users submit jobs  
2. Compute nodes execute workloads (GPU/TPU/NPU/CPU)  
3. Nodes generate VWP receipts  
4. Validators verify receipts  
5. Network rewards honest compute  
6. Fraud proofs catch incorrect computation  

---

### **Q8. What workloads can PoUW handle?**  

- AI inference  
- ML batch workloads  
- rendering  
- physics simulations  
- ZK proving  
- numerical computing  
- scientific simulation  

---

### **Q9. How does the network prevent fake compute?**  
Using:

- redundancy  
- fraud proofs  
- hardware attestation (PoC)  
- compute reputation  
- deterministic VWP receipts  

---

### **Q10. Can compute nodes cheat by submitting invalid results?**  
No.  
Invalid compute will:

- fail verification  
- get challenged  
- trigger fraud proofs  
- lead to slashing  

---

# 3. Tokenomics & AIDA Economics

---

### **Q11. What is the total supply of MBO?**  
**31,536,000 MBO**  
(equal to the number of seconds in a year)

Fixed, immutable, enforced by protocol.

---

### **Q12. Why use a time-based supply model?**  
Because:

- mathematically clean  
- non-inflationary  
- predictable  
- symbolic (compute + time)  
- trusted by engineers & economists  

---

### **Q13. What is the block reward?**  
**0.1 MBO per second**  
with halving every **5 years**.

---

### **Q14. How are rewards split between PoS and PoUW?**  
**50% PoS / 50% PoUW**

Adjustable between 40–60% with DAO approval and Founder Council oversight.

---

### **Q15. What is AIDA?**  
AIDA = Autonomous Intelligent Dynamic Adjuster.  
It regulates:

- burn rate
- base fee multiplier  
- compute pricing multipliers  

AIDA **cannot** modify supply, emission, or consensus rules.

---

### **Q16. How does the burn work?**  
AIDA adjusts burn rate between:



0% – 30%


based on demand, congestion, and compute load.

---

### **Q17. Does AIDA use AI models?**  
AIDA uses:

- deterministic on-chain logic  
- optional off-chain advisory ML  

Only deterministic results are applied on-chain.

---

### **Q18. Can AIDA be hacked to print money?**  
No:

- supply is immutable  
- emission schedule is immutable  
- AIDA has bounded parameters  
- governance + Founder Council ensure safety  

---

# 4. Governance

---

### **Q19. What is the Founder Council?**  
A 10-year temporary oversight body that can veto:

- supply changes  
- emission schedule modifications  
- PoS/PoUW reward split outside the 40–60% range  
- AIDA parameter expansions  

It ensures early-stage protection.

---

### **Q20. Does the Founder Council centralize the chain?**  
No.

It:

- cannot create or delete tokens  
- cannot alter runtime logic  
- cannot bypass DAO  
- only vetoes *critical* protocol-risk decisions  

Expires after 10 years unless renewed by the DAO.

---

### **Q21. Who controls the treasury?**  
The DAO.  
AIDA may provide advisory forecasts but cannot execute treasury actions.

---

# 5. Networking & Security

---

### **Q22. What networking protocol does Mbongo Chain use?**  
libp2p (Rust implementation):

- GossipSub  
- peer scoring  
- multi-stream multiplexing  
- anti-Byzantine routing  

---

### **Q23. How does the chain prevent spam?**  
Through:

- base fee  
- priority fee  
- AIDA-adjusted fee multipliers  
- compute gas limits  
- block gas limits  

---

### **Q24. What ensures long-term security after rewards decline?**  
Fees + compute revenue become dominant, similar to Bitcoin's late-stage economics.

---

### **Q25. What protects the chain from governance capture?**  

- DAO supermajority  
- Founder Council  
- quadratic lock-weight voting  
- safety review windows  
- AIDA risk forecasts  

---

# 6. Development & Contribution

---

### **Q26. How do developers start building?**  
Using:

- TypeScript SDK  
- Rust SDK  
- RPC API  
- local devnet  
- CLI wallet  

---

### **Q27. How is the code organized?**  
As a **Rust monorepo workspace**, with crates for:

- node  
- runtime  
- pouw  
- crypto  
- wallet  
- sdk  
- infra  
- tests  

---

### **Q28. How to create a runtime module?**  
Implement the `Module` trait:

```rust
fn validate(&self, tx: &Transaction) -> Result<()>;
fn execute(&self, tx: &Transaction, state: &mut State) -> Result<()>;

Q29. Do you support off-chain workers?

Not in v1.0, planned for later releases.

Q30. How do I run the devnet?
./mbongo-node start --dev

7. Future Directions
Q31. Will Mbongo support on-chain AI models?

Yes — part of the long-term roadmap.

Q32. Will Mbongo support ZK-verified compute?

Yes — PoUW + ZK hybrid verification is planned.

Q33. Are compute subnets planned?

Yes — specialized AI/compute subnets are part of the scaling roadmap.

Summary

This FAQ covers:

architecture

economics

governance

compute

security

development

future vision

It provides quick answers for developers, researchers, and contributors building on Mbongo Chain.