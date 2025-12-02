# Mbongo Chain — Contributor Compensation Framework

> **Document Version:** 1.0.0
> **Last Updated:** December 2025
> **Status:** Active

---

## Table of Contents

1. [Overview](#1-overview)
2. [Token Allocation](#2-token-allocation)
3. [Vesting Schedule](#3-vesting-schedule)
4. [Contributor Categories](#4-contributor-categories)
5. [Bounty System](#5-bounty-system)
6. [Evaluation Process](#6-evaluation-process)
7. [Compensation Tiers](#7-compensation-tiers)
8. [Treasury Governance](#8-treasury-governance)
9. [Anti-Abuse Rules](#9-anti-abuse-rules)
10. [Payment Process](#10-payment-process)
11. [Long-Term Incentives](#11-long-term-incentives)
12. [Examples](#12-examples)

---

## 1. Overview

### Purpose

The Mbongo Chain Contributor Compensation Framework establishes a fair, transparent, and sustainable system for rewarding contributors who help build and maintain the protocol. This framework ensures that contributors are aligned with long-term project success through token-based incentives.

### Principles

- **Meritocracy**: Compensation based on contribution value and impact
- **Transparency**: Clear criteria and public evaluation processes
- **Long-term Alignment**: Vesting schedules ensure sustained commitment
- **Decentralization**: Multi-signature governance prevents single points of control
- **Flexibility**: Adaptive system that evolves with project needs

---

## 2. Token Allocation

### Total Contributor Allocation

**10% of total supply = 3,153,600 MBO**

From the maximum supply of 31,536,000 MBO tokens, 10% is allocated to the Contributor Treasury for compensating ecosystem contributors.

### Allocation Breakdown

| Category | Allocation | MBO Amount | Purpose |
|----------|-----------|------------|---------|
| **Core Contributors** | 40% | 1,261,440 MBO | Protocol development, architecture, research |
| **Community Contributors** | 25% | 788,400 MBO | Documentation, testing, community management |
| **Bounty Program** | 20% | 630,720 MBO | One-time tasks, bug fixes, feature implementations |
| **Strategic Contributors** | 10% | 315,360 MBO | Advisors, partnerships, business development |
| **Reserve** | 5% | 157,680 MBO | Future needs, emergency allocations |

### Eligibility

Contributors become eligible for compensation when:
1. They have made substantive contributions to the project
2. Their work has been reviewed and approved by the Contributor Committee
3. They have signed the Contributor Agreement
4. They have provided KYC/identity verification (for allocations > 10,000 MBO)

---

## 3. Vesting Schedule

### Standard Vesting Terms

All contributor token allocations follow this vesting schedule:

```
┌─────────────────────────────────────────────────────────────────┐
│                     VESTING TIMELINE                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Month 0        Month 6              Month 36                  │
│    │               │                     │                      │
│    │◄────────────►│◄──────────────────►│                      │
│    │   6 Month    │   30 Month Linear  │                      │
│    │    Cliff     │     Vesting        │                      │
│    │              │                     │                      │
│    0%         0%                     100%                      │
│                   │                     │                      │
│                   ▼                     ▼                      │
│            First Release         Final Release                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

Vesting Formula:
  If t < 6 months:     vested_amount = 0
  If t ≥ 6 months:     vested_amount = total_allocation × (t - 6) / 30
  If t ≥ 36 months:    vested_amount = total_allocation
```

### Vesting Parameters

- **Cliff Period**: 6 months
- **Linear Vesting**: 30 months (2.5 years)
- **Total Duration**: 36 months (3 years)
- **Release Frequency**: Monthly (1/30th per month after cliff)

### Cliff Mechanics

During the 6-month cliff period:
- No tokens are released
- Contributions continue to be tracked
- If contributor leaves before cliff: **0 tokens vested**
- After 6 months: All accrued tokens for 6 months vest immediately

### Accelerated Vesting Conditions

Vesting may be accelerated in the following cases:

1. **Full Acquisition**: If Mbongo Chain is acquired, all unvested tokens vest immediately
2. **Change of Control**: If governance control changes hands (>50% voting power transfer)
3. **Protocol Shutdown**: If the protocol ceases operations (voted by DAO)
4. **Extraordinary Contribution**: Committee may approve accelerated vesting for exceptional work

---

## 4. Contributor Categories

### 4.1 Core Contributors

**Definition**: Full-time or near-full-time contributors working on critical protocol infrastructure.

**Responsibilities**:
- Protocol development and architecture
- Security audits and testing
- Consensus mechanism implementation
- Smart contract development
- Core infrastructure maintenance

**Compensation Range**: 20,000 - 150,000 MBO per contributor

**Evaluation Criteria**:
- Lines of code committed (weighted by complexity)
- Code review participation
- Bug fixes and security improvements
- Design proposals and RFCs
- Time commitment (hours/week)

### 4.2 Community Contributors

**Definition**: Part-time contributors focused on documentation, community building, and ecosystem growth.

**Responsibilities**:
- Technical documentation
- Tutorial creation and maintenance
- Community support and moderation
- Translation and localization
- Educational content creation
- Social media management

**Compensation Range**: 5,000 - 30,000 MBO per contributor

**Evaluation Criteria**:
- Documentation quality and completeness
- Community engagement metrics
- Support ticket resolution
- Content views and engagement
- Translation accuracy

### 4.3 Bounty Hunters

**Definition**: One-time or task-based contributors completing specific bounties.

**Responsibilities**:
- Bug fixes
- Feature implementations
- Security audits
- Performance optimizations
- Integration work

**Compensation Range**: 500 - 25,000 MBO per bounty

**Evaluation Criteria**:
- Bounty completion quality
- Timeliness of delivery
- Code quality and testing
- Documentation included

### 4.4 Strategic Contributors

**Definition**: Advisors, partners, and strategic contributors providing non-technical value.

**Responsibilities**:
- Strategic partnerships
- Business development
- Regulatory guidance
- Marketing strategy
- Investor relations

**Compensation Range**: 10,000 - 50,000 MBO per contributor

**Evaluation Criteria**:
- Partnership value (measured in adoption, integrations)
- Network effects generated
- Regulatory outcomes achieved
- Brand visibility and reach

---

## 5. Bounty System

### Bounty Tiers

| Tier | Complexity | MBO Range | Example Tasks |
|------|------------|-----------|---------------|
| **Tier 1: Trivial** | Low | 500 - 1,500 | Typo fixes, minor documentation updates |
| **Tier 2: Easy** | Low-Medium | 1,500 - 5,000 | Simple bug fixes, basic feature additions |
| **Tier 3: Medium** | Medium | 5,000 - 10,000 | Module implementation, test coverage improvements |
| **Tier 4: Hard** | Medium-High | 10,000 - 18,000 | Complex features, optimization work |
| **Tier 5: Critical** | High | 18,000 - 25,000 | Security fixes, consensus changes, major refactors |

### Bounty Process

```
1. BOUNTY CREATION
   │
   ├─ Contributor Committee creates bounty
   ├─ Defines scope, requirements, acceptance criteria
   ├─ Assigns tier and compensation
   └─ Posts to GitHub Issues with `bounty` label
   │
   ▼
2. CLAIMING
   │
   ├─ Contributor comments "Claiming this bounty"
   ├─ Committee assigns bounty to contributor
   ├─ Deadline set (typically 2-4 weeks)
   └─ Contributor begins work
   │
   ▼
3. SUBMISSION
   │
   ├─ Contributor submits PR linked to bounty issue
   ├─ PR must pass CI/CD checks
   ├─ Code review by 2+ committee members
   └─ Testing and QA verification
   │
   ▼
4. APPROVAL & PAYMENT
   │
   ├─ Committee approves or requests changes
   ├─ If approved: Bounty marked as "Completed"
   ├─ Compensation added to contributor's vesting schedule
   └─ Tokens vest according to standard schedule
```

### Active Bounties

All active bounties are listed at:
- **GitHub Issues**: https://github.com/mbongo-chain/mbongo-chain/issues?q=is%3Aissue+is%3Aopen+label%3Abounty

### Bounty Guidelines

**Do's**:
- Claim only one bounty at a time until proven track record
- Communicate progress regularly
- Ask questions if requirements are unclear
- Submit clean, tested, documented code

**Don'ts**:
- Don't claim multiple bounties simultaneously (initially)
- Don't submit incomplete work
- Don't copy/paste code without attribution
- Don't miss deadlines without communication

---

## 6. Evaluation Process

### Monthly Evaluation Cycle

```
┌─────────────────────────────────────────────────────────────────┐
│                  MONTHLY EVALUATION PROCESS                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Week 1: Data Collection                                        │
│  ─────────────────────────                                      │
│  • GitHub contributions tracked                                 │
│  • Community metrics aggregated                                 │
│  • Bounties completed recorded                                  │
│  • Self-assessment forms submitted                              │
│                                                                 │
│  Week 2: Committee Review                                       │
│  ─────────────────────────                                      │
│  • Committee reviews all contributions                          │
│  • Scoring according to rubric (0-100 points)                   │
│  • Peer review feedback                                         │
│  • Draft compensation recommendations                           │
│                                                                 │
│  Week 3: Approval & Transparency                                │
│  ──────────────────────────────                                 │
│  • Recommendations published to contributors                    │
│  • 7-day appeal period                                          │
│  • Committee addresses appeals                                  │
│  • Final approval via 4/6 multisig                              │
│                                                                 │
│  Week 4: Allocation & Vesting                                   │
│  ───────────────────────────                                    │
│  • Allocations added to vesting contracts                       │
│  • Contributors notified of total allocations                   │
│  • Public transparency report published                         │
│  • Vesting schedule updated on-chain                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Scoring Rubric

Contributors are scored on a 0-100 point scale across multiple dimensions:

#### Core Contributor Scoring (100 points total)

| Dimension | Weight | Criteria |
|-----------|--------|----------|
| **Code Quality** | 25% | Cleanliness, testing, documentation, architecture |
| **Impact** | 30% | Criticality of contributions, user/protocol benefit |
| **Quantity** | 15% | Lines of code, PRs merged, commits |
| **Collaboration** | 15% | Code reviews, mentorship, team communication |
| **Innovation** | 15% | Novel solutions, research, design proposals |

#### Community Contributor Scoring (100 points total)

| Dimension | Weight | Criteria |
|-----------|--------|----------|
| **Content Quality** | 30% | Accuracy, clarity, completeness, usefulness |
| **Engagement** | 25% | Views, comments, community feedback |
| **Responsiveness** | 20% | Support tickets, Discord responses, issue triage |
| **Consistency** | 15% | Regular contributions, reliability |
| **Growth Impact** | 10% | New users onboarded, adoption metrics |

### Appeal Process

If a contributor disagrees with their evaluation:

1. **Submit Appeal**: Within 7 days of evaluation publication
2. **Provide Evidence**: GitHub links, metrics, testimonials
3. **Committee Re-review**: Within 3 days of appeal
4. **Final Decision**: Committee vote (majority rules)
5. **Binding**: No further appeals after committee decision

---

## 7. Compensation Tiers

### Monthly Compensation Guidelines

Based on contribution level and commitment:

| Tier | Commitment | Monthly MBO | Annual MBO | Example Role |
|------|-----------|-------------|------------|--------------|
| **Tier 1** | Part-time (10h/week) | 500 | 6,000 | Community moderator |
| **Tier 2** | Part-time (20h/week) | 1,200 | 14,400 | Technical writer |
| **Tier 3** | Near full-time (30h/week) | 2,500 | 30,000 | Protocol engineer |
| **Tier 4** | Full-time (40h/week) | 4,000 | 48,000 | Senior engineer |
| **Tier 5** | Full-time + leadership | 6,000 | 72,000 | Tech lead / architect |
| **Tier 6** | Executive leadership | 10,000 | 120,000 | CTO / Chief Architect |

**Note**: These are guidelines, not fixed amounts. Actual compensation depends on:
- Performance scores (rubric above)
- Treasury availability
- Market conditions
- Individual negotiation

### Performance Multipliers

Base compensation can be adjusted by performance:

- **Outstanding (90-100 points)**: 1.2× multiplier
- **Excellent (80-89 points)**: 1.1× multiplier
- **Good (70-79 points)**: 1.0× multiplier (baseline)
- **Adequate (60-69 points)**: 0.9× multiplier
- **Below Expectations (<60 points)**: 0.7× multiplier or removal

---

## 8. Treasury Governance

### Multi-Signature Control

The Contributor Treasury is controlled by a **4-of-6 multisig wallet**:

```
┌─────────────────────────────────────────────────────────────────┐
│                     TREASURY MULTISIG                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Signers (6 total):                                            │
│   ├─ 2 Core Protocol Engineers                                  │
│   ├─ 1 Community Representative (elected)                       │
│   ├─ 1 Strategic Advisor                                        │
│   ├─ 1 Independent Security Auditor                             │
│   └─ 1 DAO Representative (appointed)                           │
│                                                                 │
│   Threshold: 4 of 6 signatures required                         │
│                                                                 │
│   Timelock: 48 hours between proposal and execution             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Governance Process

**1. Proposal Submission**
- Committee member proposes compensation batch
- Includes: recipient addresses, amounts, justifications
- Posted publicly to governance forum

**2. Review Period (48 hours)**
- Community can review and comment
- DAO can veto with 60% majority vote
- Contributors can appeal (see above)

**3. Multisig Execution**
- After 48h timelock, 4 signers execute transaction
- Tokens transferred to vesting contracts
- Transaction published on-chain

**4. Transparency Report**
- Monthly report published with all allocations
- Individual amounts anonymized (unless contributor opts in)
- Aggregate statistics shared publicly

### Emergency Procedures

In case of emergency (security breach, critical bug bounty):

- **Fast-track approval**: 24-hour timelock (instead of 48h)
- **Higher threshold**: Requires 5-of-6 signatures
- **DAO notification**: Emergency alert sent to all DAO members
- **Post-mortem**: Full transparency report within 7 days

---

## 9. Anti-Abuse Rules

### Sybil Resistance

To prevent abuse through fake identities:

1. **KYC Required**: For allocations > 10,000 MBO
2. **GitHub History**: Minimum 6-month account age for bounties
3. **Unique Identity**: One allocation per person (verified via KYC)
4. **Plagiarism Detection**: All code submissions checked for copying
5. **Behavioral Analysis**: Pattern detection for abuse (same IP, similar code style)

### Quality Standards

Contributions must meet minimum quality standards:

- **Code**: Must pass linter, tests, and 2+ code reviews
- **Documentation**: Proper formatting, no plagiarism, technically accurate
- **Community Work**: Genuine engagement, no spam or bot activity

### Claw-back Provisions

Tokens may be clawed back in the following cases:

1. **Fraud**: Contributor provided false information or plagiarized work
2. **Misconduct**: Violation of Code of Conduct or contributor agreement
3. **Security Breach**: Contributor intentionally introduced vulnerabilities
4. **Legal Issues**: Contributor involved in illegal activity related to project

**Claw-back Process**:
- Committee presents evidence
- Contributor given 14 days to respond
- DAO votes on claw-back (requires 75% majority)
- If approved: Unvested tokens cancelled, vested tokens subject to legal recovery

### Conflict of Interest

Contributors must disclose:
- Competing projects they work on
- Financial interests in competitors
- Personal relationships with other contributors affecting evaluation

**Consequences of Undisclosed Conflicts**:
- First offense: Warning
- Second offense: 3-month suspension
- Third offense: Permanent removal from program

---

## 10. Payment Process

### Vesting Contract Deployment

When a contributor is approved for compensation:

```solidity
// Simplified vesting contract logic
contract ContributorVesting {
    address public beneficiary;
    uint256 public totalAllocation;
    uint256 public startTime;
    uint256 public cliffDuration = 6 months;
    uint256 public vestingDuration = 30 months;
    uint256 public released;

    function releasableAmount() public view returns (uint256) {
        if (block.timestamp < startTime + cliffDuration) {
            return 0; // Still in cliff period
        }

        uint256 timeVested = block.timestamp - startTime - cliffDuration;
        uint256 vestedAmount = (totalAllocation * timeVested) / vestingDuration;

        if (vestedAmount > totalAllocation) {
            vestedAmount = totalAllocation;
        }

        return vestedAmount - released;
    }

    function release() external {
        uint256 amount = releasableAmount();
        require(amount > 0, "No tokens to release");
        released += amount;
        token.transfer(beneficiary, amount);
    }
}
```

### Claiming Vested Tokens

Contributors can claim vested tokens:

**Method 1: Self-Service Portal**
- Navigate to https://contributors.mbongochain.io
- Connect wallet
- View vesting schedule
- Click "Claim Available Tokens"

**Method 2: Direct Contract Interaction**
- Call `release()` function on vesting contract
- Tokens automatically transferred to registered address

**Method 3: CLI**
```bash
mbongo-cli contributor claim --address <YOUR_ADDRESS>
```

### Tax Considerations

**Important**: Contributors are responsible for their own tax obligations.

- Tokens vest on a schedule, but taxable events depend on jurisdiction
- Consult with a tax professional
- Some jurisdictions tax on vesting, others on sale
- Project provides vesting records for tax reporting

---

## 11. Long-Term Incentives

### Retention Bonuses

To encourage long-term commitment:

| Milestone | Bonus | Eligibility |
|-----------|-------|-------------|
| **1 Year** | 10% of total allocation | Still active contributor |
| **2 Years** | 20% of total allocation | Still active contributor |
| **3 Years** | 30% of total allocation | Completed full vesting |

**Bonus Structure**: Paid as additional tokens with immediate vesting (no cliff)

### Performance Bonuses

Exceptional contributions may earn additional bonuses:

- **Protocol Milestone**: 5,000 - 20,000 MBO when major milestone achieved
- **Security Contribution**: 10,000 - 50,000 MBO for critical security improvements
- **Ecosystem Growth**: 5,000 - 30,000 MBO for driving significant adoption

### Referral Program

Contributors who bring in other quality contributors earn:

- **10% of referred contributor's first year allocation**
- Max 3 referrals per contributor per year
- Referred contributor must complete at least 6 months
- Referral bonus vests immediately (no cliff)

---

## 12. Examples

### Example 1: Core Engineer (Full-time)

**Profile**:
- Full-time core protocol engineer
- 40 hours/week commitment
- High-quality code contributions

**Month 1-6 (Cliff Period)**:
- Monthly work: Protocol development, code reviews
- No tokens released
- Contributions tracked: 6 months × 4,000 MBO = 24,000 MBO accrued

**Month 7**:
- Cliff ends
- **Release**: 24,000 MBO (all accrued tokens from months 1-6)
- New monthly accrual: 4,000 MBO

**Month 8-36**:
- Monthly release: (4,000 MBO × 29 months) / 30 = ~3,867 MBO/month
- Total over 36 months: 24,000 + (4,000 × 30) = **144,000 MBO**

**With Performance Bonus** (Outstanding 95 points):
- Multiplier: 1.2×
- Total: 144,000 × 1.2 = **172,800 MBO**

**With Retention Bonus** (3 years):
- 3-year bonus: 172,800 × 30% = 51,840 MBO
- **Grand Total: 224,640 MBO**

### Example 2: Community Contributor (Part-time)

**Profile**:
- Part-time documentation writer
- 15 hours/week
- Consistent quality

**Monthly Allocation**: 800 MBO

**36-Month Timeline**:
- Months 1-6: 0 MBO released (cliff)
- Month 7: 4,800 MBO released (6 months accrued)
- Months 7-36: 800 MBO/month
- **Total: 28,800 MBO**

### Example 3: Bounty Hunter

**Profile**:
- Occasional contributor
- Completes bounties only

**Bounties Completed**:
- Tier 2 Bug Fix: 2,500 MBO
- Tier 3 Feature: 7,000 MBO
- Tier 4 Optimization: 12,000 MBO

**Total**: 21,500 MBO

**Vesting**:
- Same vesting schedule applies (6-month cliff, 30-month linear)
- Month 7 release: 21,500 MBO × 6/36 = 3,583 MBO
- Months 8-36: 603 MBO/month

**No cliff exemption for bounties** - ensures long-term alignment

---

## Summary

The Mbongo Chain Contributor Compensation Framework provides:

✅ **Fair Compensation**: 10% of total supply (3,153,600 MBO) dedicated to contributors
✅ **Long-term Alignment**: 6-month cliff + 30-month vesting ensures commitment
✅ **Transparent Governance**: 4-of-6 multisig with 48h timelock and public reporting
✅ **Flexible System**: Bounties, tiers, and performance-based adjustments
✅ **Anti-Abuse Protections**: KYC, quality standards, claw-back provisions
✅ **Growth Incentives**: Retention bonuses, referrals, performance multipliers

**For more information**:
- Contributing Guide: [CONTRIBUTING.md](../CONTRIBUTING.md)
- Governance Forum: https://forum.mbongochain.io
- Contributor Portal: https://contributors.mbongochain.io
- Questions: contributors@mbongochain.io

---

**Last Updated**: December 2025
**Next Review**: June 2026
**Document Owner**: Contributor Committee
**Approval**: 4/6 Multisig (Pending)
