//! P2P networking layer for Mbongo Chain.
//!
//! This crate implements the networking infrastructure:
//! - JSON-RPC 2.0 HTTP API via Axum
//! - P2P connectivity and peer discovery via libp2p
//! - Block propagation protocol (planned)
//! - Gossip protocol for transactions (planned)
//! - Validator discovery (planned)
//! - Network telemetry (planned)

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

/// Keeping configured bootnodes connected.
pub mod bootnodes;
/// Counts libp2p's per-stream capacity warning instead of writing it (S3 A2.5).
pub mod capacity_log;
/// A global budget for inbound frame bytes (S3 A2.2b-lite).
pub mod frame_budget;
/// Admission limits, quotas and counters for inbound P2P work (S3 A1).
pub mod limits;
/// A stable P2P identity read from a file.
pub mod node_key;
/// Minimal libp2p networking node (ping, identify, mDNS, block-sync).
pub mod p2p;
/// Block sync protocol messages and SCALE codec.
pub mod p2p_protocol;
/// This process's memory as the kernel accounts it, for diagnostics (S3 A2.2a).
pub mod proc_memory;
/// JSON-RPC 2.0 request/response types and backend trait.
pub mod rpc;
/// HTTP server wiring (Axum router + serve).
pub mod server;

pub use crate::limits::{P2PCounters, P2PCountersSnapshot};
pub use crate::node_key::load_node_key;
pub use crate::p2p::{
    BlockBroadcaster, ChannelBroadcaster, InboundBlock, InboundSyncRequest, P2PNode, SyncCommand,
    SyncEvent, SyncReply, SyncRequestKind,
};
pub use crate::p2p_protocol::{
    BlockNotifyAck, BlockNotifyCodec, SyncCodec, SyncNotification, SyncRequest, SyncResponse,
    BLOCK_NOTIFY_PROTOCOL, MAX_RANGE, SYNC_PROTOCOL,
};
pub use crate::rpc::{
    BackendError, JsonRpcRequest, JsonRpcResponse, RpcBackend, RpcError, RpcErrorCode,
};
pub use crate::server::{router, serve_on_addr};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
