//! The provider side of generic compute: one service per independently
//! operated provider, and the client that talks to it.
//!
//! Design: `docs/architecture/G2_MBONGO_JOB_V1_PLAN.md` §9. A provider runs
//! [`service::ProviderService`] with its own executor key. Its runtimes sit
//! behind `mbongo-runner:v1` ([`runner`]), so no workload is compiled in.
//!
//! **Limits.** Runtime identity, capability declarations and metering are
//! untrusted. The process boundary gives interoperability, not isolation, and
//! no key isolation is claimed ([`service::KEY_ISOLATION_ENFORCED`]).
//!
//! **Durability.** Accepted requests, job specs, inputs, task status and
//! results survive a restart ([`durable`], [`journal`]). At most one receipt
//! per task; exactly-once execution of an external runtime is not claimed.

pub mod auth;
pub mod client;
pub mod durable;
pub mod journal;
pub mod lock;
pub mod runner;
pub mod service;
