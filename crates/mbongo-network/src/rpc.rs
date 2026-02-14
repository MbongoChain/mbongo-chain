use std::future::Future;

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Trait for the RPC server backend. Implementors supply chain data.
pub trait RpcBackend: Clone + Send + Sync + 'static {
    /// Returns the current block height.
    fn get_block_height(&self) -> impl Future<Output = Result<u64, BackendError>> + Send;
    /// Health-check ping.
    fn ping(&self) -> impl Future<Output = Result<&'static str, BackendError>> + Send {
        std::future::ready(Ok("pong"))
    }
}

/// Errors returned by [`RpcBackend`] implementations.
#[derive(Debug, Error)]
pub enum BackendError {
    /// An opaque internal error with a human-readable message.
    #[error("internal backend error: {0}")]
    Internal(String),
}

/// A single JSON-RPC 2.0 request object.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcRequest {
    /// Protocol version string; must be `"2.0"`.
    pub jsonrpc: String,
    /// Name of the RPC method to invoke.
    pub method: String,
    /// Optional structured parameters for the method.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// Caller-supplied request identifier, echoed back in the response.
    #[serde(default)]
    pub id: Option<serde_json::Value>,
}

/// A single JSON-RPC 2.0 response object.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JsonRpcResponse {
    /// Protocol version string; always `"2.0"`.
    pub jsonrpc: &'static str,
    /// Result payload on success; absent on error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error payload on failure; absent on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// Request identifier echoed from the corresponding request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
}

/// Structured error object inside a JSON-RPC response.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RpcError {
    /// Numeric error code per the JSON-RPC 2.0 specification.
    pub code: i32,
    /// Short human-readable error description.
    pub message: String,
    /// Optional additional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Standard JSON-RPC 2.0 error codes.
#[derive(Debug, Copy, Clone)]
pub enum RpcErrorCode {
    /// Invalid JSON was received (-32700).
    ParseError,
    /// The request object is not a valid JSON-RPC request (-32600).
    InvalidRequest,
    /// The requested method does not exist (-32601).
    MethodNotFound,
    /// Invalid method parameters (-32602).
    InvalidParams,
    /// Internal server error (-32603).
    InternalError,
}

impl RpcErrorCode {
    /// Returns the numeric code for this error category.
    pub fn code(self) -> i32 {
        match self {
            RpcErrorCode::ParseError => -32700,
            RpcErrorCode::InvalidRequest => -32600,
            RpcErrorCode::MethodNotFound => -32601,
            RpcErrorCode::InvalidParams => -32602,
            RpcErrorCode::InternalError => -32603,
        }
    }
}

impl JsonRpcResponse {
    /// Builds a success response with the given result payload.
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0",
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Builds an error response with the given code, message, and optional data.
    pub fn error(
        id: Option<serde_json::Value>,
        code: RpcErrorCode,
        message: impl Into<String>,
        data: Option<serde_json::Value>,
    ) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0",
            result: None,
            error: Some(RpcError {
                code: code.code(),
                message: message.into(),
                data,
            }),
            id,
        }
    }
}

/// Maps a JSON-RPC error code to the appropriate HTTP status code.
pub fn http_status_for_error(code: i32) -> StatusCode {
    match code {
        -32700 => StatusCode::BAD_REQUEST,
        -32600 => StatusCode::BAD_REQUEST,
        -32601 => StatusCode::NOT_FOUND,
        -32602 => StatusCode::BAD_REQUEST,
        -32603 => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
