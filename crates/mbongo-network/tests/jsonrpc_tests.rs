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

// ── Mutating JSON-RPC wire contracts ─────────────────────────────────
//
// `submit_transaction` and `produce_block` are the two dispatched methods
// that mutate state, and until now neither had a wire-shape test. They are
// also the two whose shapes diverge most from the historical rpc_v0.1
// description, so what the node actually accepts and returns was pinned
// nowhere. These tests record the current boundary behaviour. They assert
// no new behaviour and change none.

/// A backend that remembers what the RPC layer handed it, so a test can
/// prove a request reached the backend rather than merely producing a
/// plausible response.
#[derive(Clone, Default)]
struct RecordingBackend {
    submitted: std::sync::Arc<std::sync::Mutex<Vec<Transaction>>>,
    blocks_produced: std::sync::Arc<std::sync::Mutex<usize>>,
}

impl RpcBackend for RecordingBackend {
    async fn get_block_height(&self) -> Result<u64, BackendError> {
        Ok(0)
    }

    async fn submit_transaction(&self, tx: Transaction) -> Result<String, BackendError> {
        self.submitted.lock().unwrap().push(tx);
        Ok("0xrecordedtxhash".to_string())
    }

    async fn produce_block(&self) -> Result<String, BackendError> {
        *self.blocks_produced.lock().unwrap() += 1;
        Ok("0xrecordedblockhash".to_string())
    }

    async fn get_latest_block_hash(&self) -> Result<String, BackendError> {
        Ok("0xrecordedtiphash".to_string())
    }

    async fn get_block_by_height(&self, _height: u64) -> Result<Value, BackendError> {
        Ok(json!(null))
    }
}

/// The canonical signed transaction used by these tests, as the node
/// serialises it. Produced by `cargo run -p mbongo-wallet --example
/// sign_tx`, which signs with the fixed key `[0xAA; 32]`, so the bytes are
/// deterministic and the signature is genuinely valid — the fixture is not
/// weakened to make construction easier.
fn signed_transaction_params() -> Value {
    json!({
        "tx_type": "Transfer",
        "sender": "0xe734ea6c2b6257de72355e472aa05a4c487e6b463c029ed306df2f01b5636b58",
        "receiver": "0x2222222222222222222222222222222222222222222222222222222222222222",
        "amount": 100,
        "nonce": 0,
        "payload": "None",
        "signature": "0x1c37e5d2236bba0eb9017ca49cf67ead73a8e30fa7a5afa982aeedb3c4b20485c9031e974dad586e9e4e9134d22ef003541018101c877867170fd568984cee0a"
    })
}

/// Sends one JSON-RPC request against `backend` and returns
/// (HTTP status, parsed body).
async fn post_rpc<B: RpcBackend + Clone + Send + Sync + 'static>(
    backend: B,
    body: Value,
) -> (StatusCode, Value) {
    let response = router(backend)
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
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn submit_transaction_accepts_a_structured_transaction_object() {
    let backend = RecordingBackend::default();
    let (status, v) = post_rpc(
        backend.clone(),
        json!({
            "jsonrpc": "2.0",
            "method": "submit_transaction",
            "params": signed_transaction_params(),
            "id": 41
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["jsonrpc"], json!("2.0"));
    assert_eq!(v["id"], json!(41), "request id must be preserved");
    assert!(
        v["error"].is_null(),
        "a well-formed transaction must be accepted"
    );
    // The current result is a bare hash string, not an envelope object.
    assert_eq!(v["result"], json!("0xrecordedtxhash"));

    // The transaction reached the backend, intact and still verifiable.
    let submitted = backend.submitted.lock().unwrap();
    assert_eq!(
        submitted.len(),
        1,
        "exactly one transaction must reach the backend"
    );
    let tx = &submitted[0];
    assert_eq!(tx.amount, 100);
    assert_eq!(tx.nonce, 0);
    assert_eq!(tx.receiver, mbongo_core::Address([0x22u8; 32]));
    assert!(matches!(tx.tx_type, mbongo_core::TransactionType::Transfer));
    assert!(matches!(tx.payload, mbongo_core::TransactionPayload::None));
    assert!(
        tx.verify_signature(),
        "the fixture must be a genuinely signed transaction, not a placeholder"
    );
}

#[tokio::test]
async fn submit_transaction_does_not_accept_the_historical_hex_string_form() {
    // rpc_v0.1 described params as `[signed_tx: string]`, a hex-encoded
    // SCALE blob. That is not the shape the node accepts. Both the bare
    // string and the single-element array form are rejected, and neither
    // reaches the backend.
    let backend = RecordingBackend::default();
    let hex_blob = json!("0x00e734ea6c2b6257de72355e472aa05a4c487e6b463c029ed306df2f01b5636b58");

    for params in [hex_blob.clone(), json!([hex_blob])] {
        let (_, v) = post_rpc(
            backend.clone(),
            json!({
                "jsonrpc": "2.0",
                "method": "submit_transaction",
                "params": params,
                "id": 42
            }),
        )
        .await;
        // The code is what matters; the message text is not a contract.
        assert_eq!(v["error"]["code"], json!(-32602), "params: {params}");
        assert_eq!(v["id"], json!(42));
        assert!(v["result"].is_null());
    }

    assert!(
        backend.submitted.lock().unwrap().is_empty(),
        "a rejected request must not reach the backend"
    );
}

#[tokio::test]
async fn produce_block_takes_no_parameters_and_returns_a_hash_string() {
    // The canonical request carries no params. This test deliberately does
    // not exercise passing one: the backend method takes no argument, so
    // accepting-and-ignoring a parameter is not part of the contract.
    let backend = RecordingBackend::default();
    let (status, v) = post_rpc(
        backend.clone(),
        json!({"jsonrpc": "2.0", "method": "produce_block", "id": 43}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["jsonrpc"], json!("2.0"));
    assert_eq!(v["id"], json!(43), "request id must be preserved");
    assert!(v["error"].is_null());
    // The current result is a bare hash string, not an envelope object.
    assert_eq!(v["result"], json!("0xrecordedblockhash"));

    assert_eq!(
        *backend.blocks_produced.lock().unwrap(),
        1,
        "the state-mutating backend path must be exercised exactly once"
    );
}
