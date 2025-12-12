use axum::{
    extract::State,
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};

use crate::rpc::{http_status_for_error, JsonRpcRequest, JsonRpcResponse, RpcBackend, RpcErrorCode};

#[derive(Clone)]
pub struct AppState<B: RpcBackend> {
    backend: B,
}

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

pub async fn serve_on_addr<B: RpcBackend>(addr: std::net::SocketAddr, backend: B) -> anyhow::Result<()> {
    let app = router(backend);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

async fn handle_rpc<B: RpcBackend>(State(state): State<AppState<B>>, Json(body): Json<Value>) -> impl IntoResponse {
    let response = if body.is_array() {
        let mut responses = Vec::new();
        for item in body.as_array().unwrap() {
            responses.push(process_single(&state.backend, item.clone()).await);
        }
        Json(Value::Array(
            responses
                .into_iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect(),
        ))
        .into_response()
    } else {
        let resp = process_single(&state.backend, body).await;
        let status = if let Some(err) = &resp.error { http_status_for_error(err.code) } else { StatusCode::OK };
        (status, Json(resp)).into_response()
    };

    response
}

async fn process_single<B: RpcBackend>(backend: &B, raw: Value) -> JsonRpcResponse {
    let id = raw.get("id").cloned();
    let req: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(err) => {
            return JsonRpcResponse::error(id, RpcErrorCode::ParseError, format!("parse error: {}", err), None);
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
            Err(e) => JsonRpcResponse::error(req.id.clone(), RpcErrorCode::InternalError, e.to_string(), None),
        },
        "get_block_height" => match backend.get_block_height().await {
            Ok(h) => JsonRpcResponse::success(req.id.clone(), json!(h)),
            Err(e) => JsonRpcResponse::error(req.id.clone(), RpcErrorCode::InternalError, e.to_string(), None),
        },
        _ => JsonRpcResponse::error(
            req.id.clone(),
            RpcErrorCode::MethodNotFound,
            format!("Method not found: {}", req.method),
            None,
        ),
    }
}

pub fn json_error(code: RpcErrorCode, message: impl Into<String>, id: Option<Value>) -> Response {
    let resp = JsonRpcResponse::error(id, code, message, None);
    let status = http_status_for_error(resp.error.as_ref().unwrap().code);
    (status, Json(resp)).into_response()
}