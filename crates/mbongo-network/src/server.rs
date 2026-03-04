use axum::{
    extract::State,
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};

use crate::rpc::{
    http_status_for_error, JsonRpcRequest, JsonRpcResponse, RpcBackend, RpcErrorCode,
};

/// Shared application state holding the backend implementation.
#[derive(Clone)]
pub struct AppState<B: RpcBackend> {
    backend: B,
}

/// Builds an Axum [`Router`] with the JSON-RPC endpoint and CORS middleware.
pub fn router<B: RpcBackend>(backend: B) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([Method::POST])
        .allow_headers(Any)
        .allow_origin(Any);

    Router::new()
        .route("/rpc", post(handle_rpc::<B>))
        .with_state(AppState { backend })
        .layer(cors)
}

/// Binds a TCP listener on `addr` and serves the JSON-RPC router until shutdown.
///
/// # Errors
///
/// Returns an error if the TCP listener cannot bind or if the server fails.
pub async fn serve_on_addr<B: RpcBackend>(
    addr: std::net::SocketAddr,
    backend: B,
) -> anyhow::Result<()> {
    let app = router(backend);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_rpc<B: RpcBackend>(
    State(state): State<AppState<B>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let response = if let Some(arr) = body.as_array() {
        let mut responses = Vec::new();
        for item in arr {
            responses.push(process_single(&state.backend, item.clone()).await);
        }
        Json(Value::Array(
            responses
                .into_iter()
                .map(|r| {
                    serde_json::to_value(&r).unwrap_or_else(|e| {
                        // Fallback: return a JSON-RPC error object as Value
                        let fallback = JsonRpcResponse::error(
                            r.id.clone(),
                            RpcErrorCode::InternalError,
                            format!("Internal serialization error: {e}"),
                            None,
                        );
                        // This should never fail, but if it does, return a minimal error object
                        serde_json::to_value(fallback).unwrap_or_else(|_| {
                            serde_json::json!({
                                "jsonrpc": "2.0",
                                "error": {
                                    "code": -32603,
                                    "message": "Internal error: failed to serialize error response"
                                },
                                "id": r.id
                            })
                        })
                    })
                })
                .collect(),
        ))
        .into_response()
    } else {
        let resp = process_single(&state.backend, body).await;
        let status = if let Some(err) = &resp.error {
            http_status_for_error(err.code)
        } else {
            StatusCode::OK
        };
        (status, Json(resp)).into_response()
    };

    response
}

#[allow(clippy::too_many_lines)]
async fn process_single<B: RpcBackend>(backend: &B, raw: Value) -> JsonRpcResponse {
    let req: JsonRpcRequest = match serde_json::from_value(raw.clone()) {
        Ok(r) => r,
        Err(err) => {
            let id = raw.get("id").cloned();
            return JsonRpcResponse::error(
                id,
                RpcErrorCode::ParseError,
                format!("parse error: {err}"),
                None,
            );
        }
    };

    if req.jsonrpc != "2.0" || req.method.is_empty() {
        return JsonRpcResponse::error(
            req.id.clone(),
            RpcErrorCode::InvalidRequest,
            "Invalid request: missing fields or wrong jsonrpc version",
            None,
        );
    }

    match req.method.as_str() {
        "ping" => match backend.ping().await {
            Ok(p) => JsonRpcResponse::success(req.id.clone(), json!(p)),
            Err(e) => JsonRpcResponse::error(
                req.id.clone(),
                RpcErrorCode::InternalError,
                e.to_string(),
                None,
            ),
        },
        "get_block_height" => match backend.get_block_height().await {
            Ok(h) => JsonRpcResponse::success(req.id.clone(), json!(h)),
            Err(e) => JsonRpcResponse::error(
                req.id.clone(),
                RpcErrorCode::InternalError,
                e.to_string(),
                None,
            ),
        },
        "submit_transaction" => {
            let Some(params) = req.params else {
                return JsonRpcResponse::error(
                    req.id.clone(),
                    RpcErrorCode::InvalidParams,
                    "missing params",
                    None,
                );
            };
            let tx: mbongo_core::Transaction = match serde_json::from_value(params) {
                Ok(t) => t,
                Err(e) => {
                    return JsonRpcResponse::error(
                        req.id.clone(),
                        RpcErrorCode::InvalidParams,
                        format!("invalid transaction: {e}"),
                        None,
                    )
                }
            };
            match backend.submit_transaction(tx).await {
                Ok(hash) => JsonRpcResponse::success(req.id.clone(), json!(hash)),
                Err(e) => JsonRpcResponse::error(
                    req.id.clone(),
                    RpcErrorCode::InternalError,
                    e.to_string(),
                    None,
                ),
            }
        }
        "produce_block" => match backend.produce_block().await {
            Ok(hash) => JsonRpcResponse::success(req.id.clone(), json!(hash)),
            Err(e) => JsonRpcResponse::error(
                req.id.clone(),
                RpcErrorCode::InternalError,
                e.to_string(),
                None,
            ),
        },
        "get_latest_block_hash" => match backend.get_latest_block_hash().await {
            Ok(hash) => JsonRpcResponse::success(req.id.clone(), json!(hash)),
            Err(e) => JsonRpcResponse::error(
                req.id.clone(),
                RpcErrorCode::InternalError,
                e.to_string(),
                None,
            ),
        },
        "get_block_by_height" => {
            let Some(params) = req.params else {
                return JsonRpcResponse::error(
                    req.id.clone(),
                    RpcErrorCode::InvalidParams,
                    "missing params",
                    None,
                );
            };
            let height: u64 = match serde_json::from_value(
                params.get("height").cloned().unwrap_or(params.clone()),
            ) {
                Ok(h) => h,
                Err(e) => {
                    return JsonRpcResponse::error(
                        req.id.clone(),
                        RpcErrorCode::InvalidParams,
                        format!("invalid height: {e}"),
                        None,
                    )
                }
            };
            match backend.get_block_by_height(height).await {
                Ok(block) => JsonRpcResponse::success(req.id.clone(), block),
                Err(e) => JsonRpcResponse::error(
                    req.id.clone(),
                    RpcErrorCode::InternalError,
                    e.to_string(),
                    None,
                ),
            }
        }
        _ => JsonRpcResponse::error(
            req.id.clone(),
            RpcErrorCode::MethodNotFound,
            format!("Method not found: {}", req.method),
            None,
        ),
    }
}
