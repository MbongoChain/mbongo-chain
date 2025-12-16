//! Mbongo Network - JSON-RPC server
//!
//! This crate exposes a minimal JSON-RPC 2.0 HTTP API using Axum.
//! It is backend-agnostic via the `RpcBackend` trait.

pub mod rpc;
pub mod server;

pub use crate::rpc::{JsonRpcRequest, JsonRpcResponse, RpcError, RpcErrorCode, RpcBackend, BackendError};
pub use crate::server::{router, serve_on_addr};
//! P2P networking layer for Mbongo Chain.
//!
//! This crate implements the networking infrastructure using libp2p:
//! - P2P connectivity and peer discovery
//! - Block propagation protocol
//! - Gossip protocol for transactions
//! - Validator discovery
//! - Network telemetry
//!
//! # Examples
//!
//! ```
//! // TODO: Add examples once networking is implemented
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

// Module structure (to be implemented)
// pub mod p2p;
// pub mod rpc;
// pub mod sync;
// pub mod gossip;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
