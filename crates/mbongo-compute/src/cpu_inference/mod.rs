//! The MVP CPU inference worker (MVP-B): the `mbongo-infer:v1` execution
//! profile on a local, pinned `llama-server`.
//!
//! **Non-consensus, application-level.** Implements
//! `docs/product/MVP_CPU_INFERENCE_WORKER.md` on top of the frozen contract
//! of `docs/product/MVP_INFERENCE_CONTRACT_v1.md` (module
//! [`crate::inference`]), which it never reinterprets: every task goes
//! through [`crate::inference::preflight`] and [`crate::inference::finish`].
//!
//! | Module | Concern |
//! |---|---|
//! | [`config`] | operator configuration: pinned runtime builds, paths, port, threads, timeouts |
//! | [`verify`] | start-up verification: frozen profile digests, executable SHA-256 and self-reported revision, model SHA-256 |
//! | [`http`] | a bounded loopback HTTP/1.1 client |
//! | [`llama`] | the long-lived `llama-server` child: spawn, readiness, `/props` identity, restart |
//! | [`backend`] | the completion request built only from the job spec, and the backend trait (with a test double) |
//! | [`profile`] | [`InferenceProfile`] and the deterministic [`ProfileDispatch`] |
//!
//! The worker lifecycle (`crate::worker`) is unchanged: session, lease,
//! capability, fetch, input-commitment check, execute, persist, receipt.
//! The profile only replaces the "execute" step. Ordinary execution: the
//! Mbongo-operated worker sees the CV plaintext (J PUBLIC profile); no
//! provider confidentiality is claimed, no attestation is required.

pub mod backend;
pub mod config;
pub mod http;
pub mod llama;
pub mod profile;
pub mod verify;

pub use backend::{BackendIdentity, ChatRequest, Completion, InferenceBackend};
pub use llama::{LlamaServer, StartupReport};
pub use profile::{InferenceObservation, InferenceProfile, ProfileDispatch, DISPATCH_TAG};
