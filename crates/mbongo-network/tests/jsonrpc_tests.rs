use axum::body::to_bytes;
use axum::http::StatusCode;
use mbongo_core::Transaction;
use mbongo_network::rpc::{BackendError, RpcBackend};
use mbongo_network::server::router;
use serde_json::{json, Value};
use tower::ServiceExt; // for oneshot()

#[derive(Clone)]
struct MockBackend;

impl RpcBackend for MockBackend {
    async fn get_block_height(&self) -> Result<u64, BackendError> {
        Ok(1234)
    }

    async fn submit_transaction(&self, _tx: Transaction) -> Result<String, BackendError> {
        Ok("0xmockhash".to_string())
    }

    async fn produce_block(&self) -> Result<String, BackendError> {
        Ok("0xmockblockhash".to_string())
    }

    async fn get_latest_block_hash(&self) -> Result<String, BackendError> {
        Ok("0xmocktiphash".to_string())
    }

    async fn get_block_by_height(&self, height: u64) -> Result<Value, BackendError> {
        Ok(json!({
            "header": {
                "parent_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "state_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "transactions_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "timestamp": 0,
                "height": height
            },
            "body": { "transactions": [] }
        }))
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.is_array());
    assert_eq!(v.as_array().unwrap()[0]["result"], json!("pong"));
    assert_eq!(v.as_array().unwrap()[1]["result"], json!(1234));
    assert_eq!(v.as_array().unwrap()[2]["error"]["code"], json!(-32601));
}

#[tokio::test]
async fn test_get_block_by_height() {
    let app = router(MockBackend);
    let body =
        json!({"jsonrpc":"2.0","method":"get_block_by_height","params":{"height":5},"id":"blk"});
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["result"]["header"]["height"], json!(5));
    assert_eq!(v["id"], json!("blk"));
}

#[tokio::test]
async fn test_get_latest_block_hash() {
    let app = router(MockBackend);
    let body = json!({"jsonrpc":"2.0","method":"get_latest_block_hash","id":"tip"});
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["result"], json!("0xmocktiphash"));
    assert_eq!(v["id"], json!("tip"));
}

// ── Reserved compute RPC surface (COMPUTE_INTERFACE_v0.1 §3) ──────────
//
// These five names are reserved, not implemented. The point of the tests
// is the reservation: if someone later adds a real handler for one of
// them, the corresponding assertion fails and the change becomes
// deliberate. They assert unavailability, never compute semantics.

/// The reserved names, in the order COMPUTE_INTERFACE_v0.1 §3 lists them.
const RESERVED_COMPUTE_METHODS: [&str; 5] = [
    "submit_compute_task",
    "get_compute_task",
    "get_compute_receipt",
    "list_compute_tasks",
    "get_compute_node_status",
];

async fn call_method(method: &str, params: Value, id: Value) -> Value {
    let app = router(MockBackend);
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": id});
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
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method}");
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn reserved_compute_methods_return_method_not_found() {
    for (i, method) in RESERVED_COMPUTE_METHODS.iter().enumerate() {
        let id = json!(100 + i as u64);
        let v = call_method(method, json!({}), id.clone()).await;
        assert_eq!(v["jsonrpc"], json!("2.0"), "{method}");
        assert_eq!(v["error"]["code"], json!(-32601), "{method}");
        assert_eq!(v["id"], id, "{method} must preserve the request id");
        assert!(v["result"].is_null(), "{method} must not return a result");
    }
}

#[tokio::test]
async fn reserved_compute_methods_ignore_their_documented_params() {
    // COMPUTE_INTERFACE_v0.1 §3 documents parameter shapes for the eventual
    // implementations. In v0.3 the reservation is decided before any
    // parameter is examined, so well-formed, malformed and absent params all
    // produce the same unavailability.
    let cases = [
        json!({"task_id": "0xdeadbeef"}),
        json!({"unexpected": 1}),
        json!(null),
        json!([]),
    ];
    for method in RESERVED_COMPUTE_METHODS {
        for params in &cases {
            let v = call_method(method, params.clone(), json!(7)).await;
            assert_eq!(v["error"]["code"], json!(-32601), "{method} / {params}");
            assert_eq!(v["id"], json!(7), "{method} / {params}");
        }
    }
}

#[tokio::test]
async fn reserved_compute_methods_do_not_shadow_implemented_methods() {
    // Guards the arm's placement: adding the reserved names must not have
    // captured any existing method.
    let app = router(MockBackend);
    let body = json!({"jsonrpc": "2.0", "method": "ping", "params": {}, "id": 9});
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v["error"].is_null(), "ping must still succeed");
    assert_eq!(v["id"], json!(9));
}
