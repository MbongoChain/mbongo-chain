use axum::http::StatusCode;
use mbongo_network::server::router;
use mbongo_network::rpc::{BackendError, RpcBackend};
use serde_json::{json, Value};
use tower::ServiceExt; // for oneshot()
use async_trait::async_trait;

#[derive(Clone)]
struct MockBackend;

#[async_trait]
impl RpcBackend for MockBackend {
    async fn get_block_height(&self) -> Result<u64, BackendError> {
        Ok(1234)
    }
}

#[tokio::test]
async fn test_ping() {
    let app = router(MockBackend);
    let body = json!({"jsonrpc":"2.0","method":"ping","id":1});
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/rpc")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["result"], json!("pong"));
    assert_eq!(v["jsonrpc"], json!("2.0"));
    assert_eq!(v["id"], json!(1));
}

#[tokio::test]
async fn test_get_block_height() {
    let app = router(MockBackend);
    let body = json!({"jsonrpc":"2.0","method":"get_block_height","id":"h"});
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/rpc")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["result"], json!(1234));
    assert_eq!(v["id"], json!("h"));
}

#[tokio::test]
async fn test_method_not_found() {
    let app = router(MockBackend);
    let body = json!({"jsonrpc":"2.0","method":"nope","id":2});
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/rpc")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"]["code"], json!(-32601));
    assert_eq!(v["id"], json!(2));
}

#[tokio::test]
async fn test_invalid_request_version() {
    let app = router(MockBackend);
    let body = json!({"jsonrpc":"1.0","method":"ping","id":3});
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/rpc")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"]["code"], json!(-32600));
}

#[tokio::test]
async fn test_batch_requests() {
    let app = router(MockBackend);
    let body = json!([
        {"jsonrpc":"2.0","method":"ping","id":1},
        {"jsonrpc":"2.0","method":"get_block_height","id":2},
        {"jsonrpc":"2.0","method":"nope","id":3}
    ]);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/rpc")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.is_array());
    assert_eq!(v.as_array().unwrap()[0]["result"], json!("pong"));
    assert_eq!(v.as_array().unwrap()[1]["result"], json!(1234));
    assert_eq!(v.as_array().unwrap()[2]["error"]["code"], json!(-32601));
}