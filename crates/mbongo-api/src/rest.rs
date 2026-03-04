use async_trait::async_trait;
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
use utoipa::OpenApi;

/// Backend trait that REST handlers delegate to for chain data access.
#[async_trait]
pub trait ApiBackend: Clone + Send + Sync + 'static {
    /// Returns the most recent blocks, up to `limit`.
    async fn list_blocks(&self, limit: u32) -> Result<Vec<BlockSummary>, ApiError>;
    /// Returns full details for the block identified by `hash`.
    async fn get_block(&self, hash: String) -> Result<BlockDetail, ApiError>;
    /// Returns the transaction identified by `hash`.
    async fn get_transaction(&self, hash: String) -> Result<Transaction, ApiError>;
    /// Returns account state for the given `address`.
    async fn get_account(&self, address: String) -> Result<Account, ApiError>;
    /// Returns the current validator set.
    async fn list_validators(&self) -> Result<Vec<Validator>, ApiError>;
}

/// Errors produced by REST API operations.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The requested resource does not exist.
    #[error("not found")]
    NotFound,
    /// The caller supplied invalid input.
    #[error("invalid input: {0}")]
    Invalid(String),
    /// An unexpected internal failure.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Abbreviated block data returned by the list endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BlockSummary {
    /// Hex-encoded block hash.
    pub hash: String,
    /// Block height (zero-indexed).
    pub height: u64,
    /// Unix timestamp of block production.
    pub timestamp: u64,
}

/// Full block metadata returned by the detail endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BlockDetail {
    /// Hex-encoded block hash.
    pub hash: String,
    /// Block height (zero-indexed).
    pub height: u64,
    /// Unix timestamp of block production.
    pub timestamp: u64,
    /// Hex-encoded hash of the parent block.
    pub parent_hash: String,
    /// Number of transactions included in this block.
    pub tx_count: u32,
}

/// A single transaction with optional inclusion metadata.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Transaction {
    /// Hex-encoded transaction hash.
    pub hash: String,
    /// Sender address.
    pub from: String,
    /// Recipient address, if any.
    pub to: Option<String>,
    /// Transferred value as a decimal string.
    pub value: String,
    /// Hash of the block containing this transaction, if confirmed.
    pub block_hash: Option<String>,
    /// Height of the block containing this transaction, if confirmed.
    pub block_height: Option<u64>,
}

/// Account balance and nonce snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Account {
    /// Hex-encoded account address.
    pub address: String,
    /// Current balance as a decimal string.
    pub balance: String,
    /// Number of transactions sent from this account.
    pub nonce: u64,
}

/// Validator status entry.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Validator {
    /// Hex-encoded validator address.
    pub address: String,
    /// Voting power weight.
    pub voting_power: u64,
    /// Current status label (e.g. `"active"`, `"jailed"`).
    pub status: String,
}

/// Query parameters for the block-list endpoint.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct BlocksQuery {
    /// Maximum number of blocks to return.
    pub limit: Option<u32>,
}

#[derive(Clone)]
struct AppState<B: ApiBackend> {
    backend: B,
}

#[utoipa::path(
    get,
    path = "/blocks",
    params(BlocksQuery),
    responses(
        (status = 200, description = "Recent blocks", body = [BlockSummary])
    )
)]
async fn get_blocks<B: ApiBackend>(
    State(state): State<AppState<B>>,
    Query(q): Query<BlocksQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(10).min(1000);
    match state.backend.list_blocks(limit).await {
        Ok(list) => (axum::http::StatusCode::OK, Json(list)).into_response(),
        Err(ApiError::Invalid(msg)) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({"error": msg})),
        )
            .into_response(),
        Err(ApiError::NotFound) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": "not found"})),
        )
            .into_response(),
        Err(ApiError::Internal(msg)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": msg})),
        )
            .into_response(),
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
async fn get_block<B: ApiBackend>(
    State(state): State<AppState<B>>,
    Path(hash): Path<String>,
) -> impl IntoResponse {
    match state.backend.get_block(hash).await {
        Ok(block) => (axum::http::StatusCode::OK, Json(block)).into_response(),
        Err(ApiError::NotFound) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": "not found"})),
        )
            .into_response(),
        Err(ApiError::Invalid(msg)) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({"error": msg})),
        )
            .into_response(),
        Err(ApiError::Internal(msg)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": msg})),
        )
            .into_response(),
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
async fn get_transaction<B: ApiBackend>(
    State(state): State<AppState<B>>,
    Path(hash): Path<String>,
) -> impl IntoResponse {
    match state.backend.get_transaction(hash).await {
        Ok(tx) => (axum::http::StatusCode::OK, Json(tx)).into_response(),
        Err(ApiError::NotFound) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": "not found"})),
        )
            .into_response(),
        Err(ApiError::Invalid(msg)) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({"error": msg})),
        )
            .into_response(),
        Err(ApiError::Internal(msg)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": msg})),
        )
            .into_response(),
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
async fn get_account<B: ApiBackend>(
    State(state): State<AppState<B>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    match state.backend.get_account(address).await {
        Ok(acc) => (axum::http::StatusCode::OK, Json(acc)).into_response(),
        Err(ApiError::NotFound) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": "not found"})),
        )
            .into_response(),
        Err(ApiError::Invalid(msg)) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({"error": msg})),
        )
            .into_response(),
        Err(ApiError::Internal(msg)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": msg})),
        )
            .into_response(),
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
        Err(ApiError::Internal(msg)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": msg})),
        )
            .into_response(),
        Err(ApiError::Invalid(msg)) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({"error": msg})),
        )
            .into_response(),
        Err(ApiError::NotFound) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": "not found"})),
        )
            .into_response(),
    }
}

#[derive(utoipa::OpenApi)]
#[openapi(
    paths(get_blocks, get_block, get_transaction, get_account, get_validators),
    components(schemas(BlockSummary, BlockDetail, Transaction, Account, Validator)),
    tags((name = "mbongo-api", description = "Mbongo REST API"))
)]
struct ApiDoc;

/// Builds an Axum [`Router`] with all REST endpoints, Swagger UI, and CORS middleware.
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
        .route("/blocks/:hash", get(get_block::<B>))
        .route("/transactions/:hash", get(get_transaction::<B>))
        .route("/accounts/:address", get(get_account::<B>))
        .route("/validators", get(get_validators::<B>))
        .merge(swagger)
        .with_state(state)
        .layer(cors)
}

/// Binds a TCP listener on `addr` and serves the REST router until shutdown.
///
/// # Errors
///
/// Returns an error if the TCP listener cannot bind or if the server fails.
pub async fn serve_on_addr<B: ApiBackend>(
    addr: std::net::SocketAddr,
    backend: B,
) -> anyhow::Result<()> {
    let app = router(backend);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await.map_err(anyhow::Error::from)
}
