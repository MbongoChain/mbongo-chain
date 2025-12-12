//! REST and WebSocket APIs for Mbongo Chain.
//!
//! This crate provides external APIs for interacting with the blockchain:
//! - REST API for compute job submission
//! - WebSocket API for real-time subscriptions
//! - JSON-RPC API for node queries
//! - Client SDKs (Rust, JavaScript, Python)
//!
//! # Examples
//!
//! ```
//! // TODO: Add examples once API server is implemented
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod rest;
// pub mod ws;
// pub mod rpc;
// pub mod sdk;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
