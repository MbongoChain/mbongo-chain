use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[allow(async_fn_in_trait)]
pub trait RpcBackend: Clone + Send + Sync + 'static {
    async fn get_block_height(&self) -> Result<u64, BackendError>;
    async fn ping(&self) -> Result<&'static str, BackendError> {
        Ok("pong")
    }
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("internal backend error: {0}")]
    Internal(String),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    #[serde(default)]
    pub id: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Copy, Clone)]
pub enum RpcErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
}

impl RpcErrorCode {
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
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0",
            result: Some(result),
            error: None,
            id,
        }
    }

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