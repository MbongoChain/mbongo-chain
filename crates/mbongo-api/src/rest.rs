use axum::{
    extract::{Path, Query, State},
    http::Method,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tower_http::cors::{Any, CorsLayer};
use async_trait::async_trait;

#[async_trait]
pub trait ApiBackend: Clone + Send + Sync + 'static {
    async fn list_blocks(&self, limit: u32) -> Result<Vec<BlockSummary>, ApiError>;
    async fn get_block(&self, hash: String) -> Result<BlockDetail, ApiError>;
    async fn get_transaction(&self, hash: String) -> Result<Transaction, ApiError>;
    async fn get_account(&self, address: String) -> Result<Account, ApiError>;
    async fn list_validators(&self) -> Result<Vec<Validator>, ApiError>;
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("not found")] 
    NotFound,
    #[error("invalid input: {0}")] 
    Invalid(String),
    #[error("internal error: {0}")] 
    Internal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BlockSummary {
    pub hash: String,
    pub height: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BlockDetail {
    pub hash: String,
    pub height: u64,
    pub timestamp: u64,
    pub parent_hash: String,
    pub tx_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Transaction {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub block_hash: Option<String>,
    pub block_height: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Account {
    pub address: String,
    pub balance: String,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Validator {
    pub address: String,
    pub voting_power: u64,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct BlocksQuery { pub limit: Option<u32> }

#[derive(Clone)]
struct AppState<B: ApiBackend> { backend: B }

#[utoipa::path(
    get,
    path = "/blocks",
    params(BlocksQuery),
    responses(
        (status = 200, description = "Recent blocks", body = [BlockSummary])
    )
)]
async fn get_blocks<B: ApiBackend>(State(state): State<AppState<B>>, Query(q): Query<BlocksQuery>) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(10).min(1000);
    match state.backend.list_blocks(limit).await {
        Ok(list) => (axum::http::StatusCode::OK, Json(list)).into_response(),
        Err(ApiError::Invalid(msg)) => (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response(),
        Err(ApiError::NotFound) => (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
        Err(ApiError::Internal(msg)) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/blocks/{hash}",
    params(("hash" = String, Path, description = "Block hash")),
    responses(
        (status = 200, description = "Block details", body = BlockDetail),
        (status = 404, description = "Block not found")
    )
)]
async fn get_block<B: ApiBackend>(State(state): State<AppState<B>>, Path(hash): Path<String>) -> impl IntoResponse {
    match state.backend.get_block(hash).await {
        Ok(block) => (axum::http::StatusCode::OK, Json(block)).into_response(),
        Err(ApiError::NotFound) => (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
        Err(ApiError::Invalid(msg)) => (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response(),
        Err(ApiError::Internal(msg)) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/transactions/{hash}",
    params(("hash" = String, Path, description = "Transaction hash")),
    responses(
        (status = 200, description = "Transaction", body = Transaction),
        (status = 404, description = "Not found")
    )
)]
async fn get_transaction<B: ApiBackend>(State(state): State<AppState<B>>, Path(hash): Path<String>) -> impl IntoResponse {
    match state.backend.get_transaction(hash).await {
        Ok(tx) => (axum::http::StatusCode::OK, Json(tx)).into_response(),
        Err(ApiError::NotFound) => (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
        Err(ApiError::Invalid(msg)) => (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response(),
        Err(ApiError::Internal(msg)) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/accounts/{address}",
    params(("address" = String, Path, description = "Account address")),
    responses(
        (status = 200, description = "Account info", body = Account),
        (status = 404, description = "Not found")
    )
)]
async fn get_account<B: ApiBackend>(State(state): State<AppState<B>>, Path(address): Path<String>) -> impl IntoResponse {
    match state.backend.get_account(address).await {
        Ok(acc) => (axum::http::StatusCode::OK, Json(acc)).into_response(),
        Err(ApiError::NotFound) => (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
        Err(ApiError::Invalid(msg)) => (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response(),
        Err(ApiError::Internal(msg)) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/validators",
    responses(
        (status = 200, description = "Validators", body = [Validator])
    )
)]
async fn get_validators<B: ApiBackend>(State(state): State<AppState<B>>) -> impl IntoResponse {
    match state.backend.list_validators().await {
        Ok(list) => (axum::http::StatusCode::OK, Json(list)).into_response(),
        Err(ApiError::Internal(msg)) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response(),
        Err(ApiError::Invalid(msg)) => (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response(),
        Err(ApiError::NotFound) => (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
    }
}

#[derive(utoipa::OpenApi)]
#[openapi(
    paths(get_blocks, get_block, get_transaction, get_account, get_validators),
    components(schemas(BlockSummary, BlockDetail, Transaction, Account, Validator)),
    tags((name = "mbongo-api", description = "Mbongo REST API"))
)]
struct ApiDoc;

pub fn router<B: ApiBackend>(backend: B) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET])
        .allow_headers(Any);

    let state = AppState { backend };

    let openapi = ApiDoc::openapi();
    let swagger = utoipa_swagger_ui::SwaggerUi::new("/docs").url("/openapi.json", openapi.clone());

    Router::new()
        .route("/blocks", get(get_blocks::<B>))
        .route("/blocks/{hash}", get(get_block::<B>))
        .route("/transactions/:hash", get(get_transaction::<B>))
        .route("/accounts/:address", get(get_account::<B>))
        .route("/validators", get(get_validators::<B>))
        .merge(swagger)
        .route("/openapi.json", get(|| async move { Json(openapi) }))
        .with_state(state)
        .layer(cors)
}

pub async fn serve_on_addr<B: ApiBackend>(addr: std::net::SocketAddr, backend: B) -> anyhow::Result<()> {
    let app = router(backend);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .map_err(|e| anyhow::anyhow!(e))
}
