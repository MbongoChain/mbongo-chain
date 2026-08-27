//! Mbongo node library surface.
//!
//! The node itself is a binary; this library exists so that operational
//! tooling shipped with the crate (the devnet harness and the external
//! convergence probe) can share one implementation instead of each
//! carrying its own copy.

pub mod convergence;
