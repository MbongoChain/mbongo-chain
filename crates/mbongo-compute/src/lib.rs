//! Reference compute worker, control plane and private data plane for
//! Mbongo Chain.
//!
//! This crate is the first executable proof of the Compute architecture:
//! a client commits a `ComputeTask` whose private input stays off-chain;
//! the executor the client named obtains that input through a scoped,
//! single-use off-chain authorization, verifies its commitment, runs a
//! deterministic reference profile, persists the private result, and
//! anchors an RFC 0005-bound receipt — and the chain never acts as
//! scheduler or as private-data transport.
//!
//! It is a **reference implementation**, not a product. Nothing here is
//! protocol authority: consensus is decided by RFC 0005 and implemented in
//! `mbongo-node`; the off-chain boundaries are those of
//! `docs/architecture/compute-control-plane-worker-interface.md` (E) and
//! `docs/architecture/compute-private-data-plane-interface.md` (F). Where
//! this crate chooses something those contracts left open — lease and
//! session durations, a confirmation depth, an execution profile, an input
//! representation — the choice is an implementation policy of this crate,
//! documented as such, and never a consensus rule.
//!
//! # The four planes, and what each one is authority for
//!
//! | Component | Authority for | Module |
//! |---|---|---|
//! | the chain | task existence, `task.executor`, `input_commitment`, and whether a receipt is anchored | [`chain`] |
//! | the control plane | leases, attempts, sessions — coordination only | [`control_plane`] |
//! | the private data plane | objects, capabilities, consumption, whether a result exists | [`data_plane`] |
//! | the worker | its own execution, and the executor key it holds | [`worker`], [`execution`] |
//!
//! # What this crate deliberately does not do
//!
//! No marketplace, scheduler, GPU runtime, TEE, attestation, ZK, fraud
//! proofs, payment, tokenomics, new consensus rule, new RPC method or
//! receipt field. The worker is **ordinary execution**: the process that
//! runs the reference profile sees the input plaintext, and no provider
//! confidentiality is claimed. The single point at which a confidential
//! profile would attach is named in [`execution`] and is not implemented.
//!
//! An anchored receipt produced here is a **bound claim** by the named
//! executor — bound to the task, to the committed input, and to the
//! executor's key. Neither this crate nor the chain checks that the work was
//! done correctly.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    // `lease.lease_id`, `session.session_id`: the vocabulary of E, kept verbatim.
    clippy::struct_field_names
)]

pub mod chain;
pub mod clock;
pub mod confidential;
pub mod conformance;
pub mod control_plane;
pub mod data_plane;
pub mod execution;
pub mod identity;
pub mod policy;
pub mod worker;
