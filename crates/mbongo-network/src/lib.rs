//! P2P networking layer for Mbongo Chain.
//!
//! This crate implements the networking infrastructure:
//! - JSON-RPC 2.0 HTTP API via Axum
//! - P2P connectivity and peer discovery (planned)
//! - Block propagation protocol (planned)
//! - Gossip protocol for transactions (planned)
//! - Validator discovery (planned)
//! - Network telemetry (planned)

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

/// JSON-RPC 2.0 request/response types and backend trait.
pub mod rpc;
/// HTTP server wiring (Axum router + serve).
pub mod server;

pub use crate::rpc::{BackendError, JsonRpcRequest, JsonRpcResponse, RpcBackend, RpcError, RpcErrorCode};
pub use crate::server::{router, serve_on_addr};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
