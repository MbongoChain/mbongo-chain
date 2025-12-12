use axum::http::StatusCode;
use mbongo_api::rest::{self, ApiBackend, ApiError, Account, BlockDetail, BlockSummary, Transaction, Validator};
use serde_json::json;
use tower::ServiceExt;

#[derive(Clone)]
struct MockBackend;

#[allow(async_fn_in_trait)]
impl ApiBackend for MockBackend {
    async fn list_blocks(&self, limit: u32) -> Result<Vec<BlockSummary>, ApiError> {
        Ok((0..limit)
            .map(|i| BlockSummary { hash: format!("h{i}"), height: i as u64, timestamp: 1000 + i as u64 })
            .collect())
    }
    async fn get_block(&self, hash: String) -> Result<BlockDetail, ApiError> {
        if hash == "missing" { return Err(ApiError::NotFound); }
        Ok(BlockDetail { hash: hash.clone(), height: 1, timestamp: 1234, parent_hash: "p".into(), tx_count: 2 })
    }
    async fn get_transaction(&self, hash: String) -> Result<Transaction, ApiError> {
        if hash == "missing" { return Err(ApiError::NotFound); }
        Ok(Transaction { hash, from: "a".into(), to: Some("b".into()), value: "0x1".into(), block_hash: Some("bh".into()), block_height: Some(1) })
    }
    async fn get_account(&self, address: String) -> Result<Account, ApiError> {
        if address == "missing" { return Err(ApiError::NotFound); }
        Ok(Account { address, balance: "0x10".into(), nonce: 7 })
    }
    async fn list_validators(&self) -> Result<Vec<Validator>, ApiError> {
        Ok(vec![Validator { address: "v1".into(), voting_power: 100, status: "active".into() }])
    }
}

#[tokio::test]
async fn test_get_blocks_default_limit() {
    let app = rest::router(MockBackend);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/blocks")
                .method("GET")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.is_array());
    assert_eq!(v.as_array().unwrap().len(), 10);
}

#[tokio::test]
async fn test_get_block_by_hash() {
    let app = rest::router(MockBackend);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/blocks/0xabc")
                .method("GET")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["hash"], json!("0xabc"));
}

#[tokio::test]
async fn test_transaction_not_found() {
    let app = rest::router(MockBackend);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/transactions/missing")
                .method("GET")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_account_ok() {
    let app = rest::router(MockBackend);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/accounts/0x1")
                .method("GET")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_validators_list() {
    let app = rest::router(MockBackend);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/validators")
                .method("GET")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.is_array());
    assert_eq!(v.as_array().unwrap().len(), 1);
}
