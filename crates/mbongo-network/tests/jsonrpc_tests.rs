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

// ── rpc_v0.3: ComputeTask on the wire ─────────────────────────────────
//
// RFC 0005 added `TransactionPayload::ComputeTask`, and the JSON the node
// serves is a pure function of the serde types, so the payload union rpc_v0.2
// §4.1 pinned as `None | AnchorReceipt(Receipt)` now carries a third variant.
// That widening is what rpc_v0.3 documents. These tests pin it at the wire
// boundary using the neutral fixture `test-vectors/rpc/compute-task-rpc-v1.json`,
// whose objects were assembled from the serde annotations and whose bytes come
// from the protocol fixtures — never from this crate. They assert
// representation only: admission and consensus are covered in the node.

const RPC_FIXTURE: &str = include_str!("../../../test-vectors/rpc/compute-task-rpc-v1.json");

fn rpc_doc() -> Value {
    let v: Value = serde_json::from_str(RPC_FIXTURE).expect("rpc fixture parses");
    assert_eq!(v["fixture_version"].as_u64(), Some(1));
    v
}

fn fixture_hex(v: &Value) -> Vec<u8> {
    let s = v.as_str().expect("hex string");
    assert!(!s.starts_with("0x"), "fixture hex carries no 0x prefix");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn fixture_32(v: &Value) -> [u8; 32] {
    fixture_hex(v).try_into().expect("32 bytes")
}

/// The named transaction entry of the RPC fixture. Exactly one must match.
fn rpc_transaction(doc: &Value, name: &str) -> Value {
    let all = doc["transactions"].as_array().expect("transactions");
    let matches: Vec<&Value> = all.iter().filter(|t| t["name"].as_str() == Some(name)).collect();
    assert_eq!(matches.len(), 1, "transaction vector {name:?}");
    matches[0].clone()
}

/// The transaction hash rule mirrored from the node: BLAKE3 over the full
/// SCALE encoding, signature included.
fn transaction_hash(tx: &Transaction) -> [u8; 32] {
    use parity_scale_codec::Encode;
    mbongo_core::crypto::blake3_hash(&tx.encode())
}

/// A backend that serves one fixed block, exactly as the node would: the
/// JSON is `serde_json::to_value` of the protocol `Block`.
#[derive(Clone)]
struct BlockBackend {
    block: mbongo_core::Block,
}

impl RpcBackend for BlockBackend {
    async fn get_block_height(&self) -> Result<u64, BackendError> {
        Ok(self.block.header.height)
    }

    async fn submit_transaction(&self, _tx: Transaction) -> Result<String, BackendError> {
        Err(BackendError::Internal("read-only backend".to_string()))
    }

    async fn produce_block(&self) -> Result<String, BackendError> {
        Err(BackendError::Internal("read-only backend".to_string()))
    }

    async fn get_latest_block_hash(&self) -> Result<String, BackendError> {
        Ok("0xblockbackend".to_string())
    }

    async fn get_block_by_height(&self, height: u64) -> Result<Value, BackendError> {
        if height == self.block.header.height {
            serde_json::to_value(&self.block).map_err(|e| BackendError::Internal(e.to_string()))
        } else {
            Err(BackendError::Internal(format!(
                "block not found at height {height}"
            )))
        }
    }
}

#[tokio::test]
async fn submit_transaction_accepts_compute_task_objects_from_minimal_to_maximal() {
    use parity_scale_codec::Encode;
    let doc = rpc_doc();
    for name in ["minimal", "canonical", "maximal"] {
        let entry = rpc_transaction(&doc, name);
        let backend = RecordingBackend::default();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "submit_transaction",
            "params": entry["object"],
            "id": 51
        });
        let (status, v) = post_rpc(backend.clone(), request).await;
        assert_eq!(status, StatusCode::OK, "{name}");
        assert!(v["error"].is_null(), "{name}: {}", v["error"]);
        assert_eq!(v["result"], json!("0xrecordedtxhash"), "{name}");
        assert_eq!(v["id"], json!(51), "{name}");

        // The object deserialised to exactly the protocol transaction the
        // fixture pins: same bytes, same hash, same task identity, and the
        // signature the submitter produced still verifies.
        let submitted = backend.submitted.lock().unwrap();
        assert_eq!(submitted.len(), 1, "{name}");
        let tx = &submitted[0];
        assert!(matches!(
            tx.tx_type,
            mbongo_core::TransactionType::ComputeTask
        ));
        let mbongo_core::TransactionPayload::ComputeTask(task) = &tx.payload else {
            panic!("{name}: expected a ComputeTask payload");
        };
        assert_eq!(
            tx.encode(),
            fixture_hex(&entry["expected"]["full_transaction"]),
            "{name}: SCALE"
        );
        assert_eq!(
            transaction_hash(tx),
            fixture_32(&entry["expected"]["transaction_hash"]),
            "{name}: transaction hash"
        );
        assert_eq!(
            task.task_id(),
            fixture_32(&entry["expected"]["task_id"]),
            "{name}: task_id"
        );
        assert!(tx.verify_signature(), "{name}: signature");
        // Byte fields survived the JSON array form exactly.
        let wire = &entry["object"]["payload"]["ComputeTask"];
        let spec: Vec<u8> = wire["execution_spec"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
            .collect();
        assert_eq!(task.execution_spec, spec, "{name}: execution_spec");
        assert_eq!(wire["salt"].as_array().unwrap().len(), 32, "{name}");
        assert_eq!(
            wire["input_commitment"].as_array().unwrap().len(),
            32,
            "{name}"
        );
        // Addresses use the existing canonical form.
        assert_eq!(
            wire["submitter"],
            json!(task.submitter.to_string()),
            "{name}"
        );
        assert_eq!(wire["executor"], json!(task.executor.to_string()), "{name}");
        // And the wire object carries no task_id: identity is derived.
        assert!(
            wire.get("task_id").is_none(),
            "{name}: task_id is not a wire field"
        );
    }
}

#[tokio::test]
async fn maximal_compute_task_request_is_far_below_the_body_limit() {
    // axum's default request body limit is 2 MiB (axum-core
    // `DefaultBodyLimit`, 2_097_152 bytes). The router applies it as-is and
    // the request below passes through it. Pin the margin so a future
    // encoding change that balloons the array form is noticed here.
    let doc = rpc_doc();
    let entry = rpc_transaction(&doc, "maximal");
    assert_eq!(
        entry["object"]["payload"]["ComputeTask"]["execution_spec"]
            .as_array()
            .unwrap()
            .len(),
        1024
    );
    let request = json!({
        "jsonrpc": "2.0",
        "method": "submit_transaction",
        "params": entry["object"],
        "id": 52
    });
    let bytes = request.to_string().len();
    assert!(bytes < 8 * 1024, "maximal request is {bytes} bytes");
    assert!(bytes < 2_097_152);
    let (status, v) = post_rpc(RecordingBackend::default(), request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(v["error"].is_null());
}

#[tokio::test]
async fn get_block_by_height_returns_compute_task_payloads_intact() {
    let doc = rpc_doc();
    let pinned = doc["block"]["object"].clone();
    // The pinned object is a protocol block: it deserialises, and its
    // transactions_root is the real commitment over its transactions.
    let block: mbongo_core::Block =
        serde_json::from_value(pinned.clone()).expect("fixture block deserialises");
    assert_eq!(
        block.header.transactions_root,
        mbongo_core::compute_transactions_root(&block.body.transactions)
    );
    assert_eq!(block.body.transactions.len(), 3);
    assert!(matches!(
        block.body.transactions[1].payload,
        mbongo_core::TransactionPayload::ComputeTask(_)
    ));

    // Served through the router exactly as the node serialises it.
    let (status, v) = post_rpc(
        BlockBackend {
            block: block.clone(),
        },
        json!({"jsonrpc": "2.0", "method": "get_block_by_height", "params": {"height": 1}, "id": 53}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(v["error"].is_null(), "{}", v["error"]);
    assert_eq!(v["id"], json!(53));
    assert_eq!(
        v["result"], pinned,
        "the served block must be the pinned wire object, byte field for byte field"
    );

    // Round trip: every served transaction converts back to the protocol
    // transaction it came from, including the ComputeTask's byte fields.
    for (i, wire) in v["result"]["body"]["transactions"].as_array().unwrap().iter().enumerate() {
        let back: Transaction = serde_json::from_value(wire.clone()).expect("round trip");
        assert_eq!(back, block.body.transactions[i], "transaction {i}");
    }
    let hashes = doc["block"]["expected"]["transaction_hashes"].as_array().unwrap();
    for (i, tx) in block.body.transactions.iter().enumerate() {
        assert_eq!(transaction_hash(tx), fixture_32(&hashes[i]), "hash {i}");
    }
}

#[tokio::test]
async fn unknown_payload_variants_are_rejected_before_the_backend() {
    // Adding ComputeTask does not open the door to arbitrary variants: a
    // payload the protocol does not define fails to deserialise and yields
    // -32602 without reaching the backend, exactly as before.
    let doc = rpc_doc();
    let base = rpc_transaction(&doc, "canonical")["object"].clone();
    let examples = doc["unknown_variant"]["examples"].as_array().unwrap();
    assert_eq!(examples.len(), 3);
    for example in examples {
        let backend = RecordingBackend::default();
        let mut object = base.clone();
        object["payload"] = example["payload"].clone();
        let (status, v) = post_rpc(
            backend.clone(),
            json!({"jsonrpc": "2.0", "method": "submit_transaction", "params": object, "id": 54}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{}", example["payload"]);
        assert_eq!(
            v["error"]["code"], doc["unknown_variant"]["expected"]["submit_transaction_error_code"],
            "{}",
            example["payload"]
        );
        assert_eq!(v["id"], json!(54));
        assert!(backend.submitted.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn v02_transaction_shapes_are_unchanged_under_v03() {
    // The Transfer and AnchorReceipt objects rpc_v0.2 pinned still submit
    // and still round-trip exactly; only the union gained a member.
    let doc = rpc_doc();
    let block: mbongo_core::Block = serde_json::from_value(doc["block"]["object"].clone()).unwrap();
    for (i, wire) in doc["block"]["object"]["body"]["transactions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if i == 1 {
            continue; // the ComputeTask; covered above
        }
        let backend = RecordingBackend::default();
        let (status, v) = post_rpc(
            backend.clone(),
            json!({"jsonrpc": "2.0", "method": "submit_transaction", "params": wire, "id": 55}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "transaction {i}");
        assert!(v["error"].is_null(), "transaction {i}");
        let submitted = backend.submitted.lock().unwrap();
        assert_eq!(submitted[0], block.body.transactions[i]);
        assert!(submitted[0].verify_signature(), "transaction {i}");
        assert_eq!(
            serde_json::to_value(&submitted[0]).unwrap(),
            *wire,
            "transaction {i}"
        );
    }
    // The receipt inside the anchor keeps its v0.2 mixed byte form: hex for
    // executor and signature, arrays for the four plain byte fields.
    let receipt = &doc["block"]["object"]["body"]["transactions"][2]["payload"]["AnchorReceipt"];
    for field in ["executor", "signature"] {
        assert!(
            receipt[field].as_str().is_some_and(|s| s.starts_with("0x")),
            "{field}"
        );
    }
    for field in [
        "task_id",
        "input_commitment",
        "output_commitment",
        "metadata",
    ] {
        assert!(receipt[field].is_array(), "{field}");
    }
    // And the Transfer's payload is still the bare string.
    assert_eq!(
        doc["block"]["object"]["body"]["transactions"][0]["payload"],
        json!("None")
    );
}

#[tokio::test]
async fn receipt_task_id_on_the_wire_is_the_derived_task_identity() {
    // task_id is never a transaction wire field; it appears only inside a
    // receipt, as an array, and must equal the identity derived from the
    // committed task's bytes.
    let doc = rpc_doc();
    let block: mbongo_core::Block = serde_json::from_value(doc["block"]["object"].clone()).unwrap();
    let mbongo_core::TransactionPayload::AnchorReceipt(receipt) =
        &block.body.transactions[2].payload
    else {
        panic!("expected the anchor");
    };
    let canonical = rpc_transaction(&doc, "canonical");
    assert_eq!(
        receipt.task_id,
        fixture_32(&canonical["expected"]["task_id"])
    );
    let wire_task_id =
        &doc["block"]["object"]["body"]["transactions"][2]["payload"]["AnchorReceipt"]["task_id"];
    assert_eq!(wire_task_id.as_array().unwrap().len(), 32);
    let tx: Transaction = serde_json::from_value(canonical["object"].clone()).unwrap();
    let mbongo_core::TransactionPayload::ComputeTask(task) = &tx.payload else {
        panic!("expected the task");
    };
    assert_eq!(task.task_id(), receipt.task_id);
    assert_eq!(task.input_commitment, receipt.input_commitment);
    assert_eq!(task.executor, receipt.executor);
}
