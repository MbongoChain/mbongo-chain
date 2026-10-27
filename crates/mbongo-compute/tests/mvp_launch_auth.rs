//! Mbongo MVP Launch Auth Hardening: the named CI test group for the
//! request-signing review of `docs/security/MVP_LAUNCH_SECURITY_REVIEW.md`
//! (MVP-E). E-A01–E-A14 are the review's adversarial cases, against the
//! real axum front on `127.0.0.1:0`; E-A15–E-A17 bound its behaviour under
//! floods. Time is a manual clock, so the validity window is tested at its
//! exact edges.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::clock::{Clock, ManualClock};
use mbongo_compute::cpu_inference::backend::testing::FakeBackend;
use mbongo_compute::cpu_inference::{InferenceBackend, InferenceProfile};
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::identity::ExecutorKey;
use mbongo_compute::inference::kaayu::CvExtractionInputV1;
use mbongo_compute::mvp::auth::{sign_request, MAX_SKEW_SECS};
use mbongo_compute::mvp::client::{build_task, submit_task, FrontClient};
use mbongo_compute::mvp::front::{Front, FrontConfig};

const START: u64 = 1_790_000_000;
const CV: &str = "Synthetic auth-test CV.\n\nExperience\nWarehouse Lead, Example Depot Ltd (2020-01 to present)\n  Runs the Qorvanthe night shift.\n";
const CV_FRAGMENT: &str = "Qorvanthe";

/// A signed request's (method, path, body).
type Req<'a> = (&'a str, &'a str, &'a [u8]);
/// Header name and value.
type Header = (&'static str, String);

static LOG: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
struct Capture;
impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, r: &log::Record<'_>) {
        LOG.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().push(format!(
            "{} {}",
            r.target(),
            r.args()
        ));
    }
    fn flush(&self) {}
}
fn captured() -> Vec<String> {
    LOG.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().clone()
}

struct Rig {
    chain: FakeChain,
    front: Front<FakeChain>,
    base: String,
    clock: ManualClock,
    alice: LocalKey,
    bob: LocalKey,
    executor: ExecutorKey,
}

async fn rig() -> Rig {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = log::set_logger(&Capture);
        log::set_max_level(log::LevelFilter::Trace);
    });
    let clock = ManualClock::starting_at(START);
    let chain = FakeChain::new();
    let backend = FakeBackend::pinned();
    backend.push_content(
        serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../test-vectors/inference/output-canonicalization-v1.json"
            ))
            .unwrap(),
        )
        .unwrap()["raw_model_output"]
            .as_str()
            .unwrap(),
    );
    let profile = InferenceProfile::new(Arc::new(backend) as Arc<dyn InferenceBackend>).unwrap();
    let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
    let front = Front::new(
        chain.clone(),
        Arc::new(clock.clone()) as Arc<dyn Clock>,
        [0x42u8; 32],
        executor.clone(),
        Arc::new(profile),
        None,
        FrontConfig::default(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = front.router();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Rig {
        chain,
        front,
        base,
        clock,
        alice: LocalKey::from_seed(&[0xAAu8; 32]),
        bob: LocalKey::from_seed(&[0xBBu8; 32]),
        executor,
    }
}

/// A request signed for (`sign_method`, `sign_path`, `sign_body`, `ts`,
/// `nonce`) but sent as (`method`, `path`, `body`).
#[allow(clippy::too_many_arguments)]
async fn send(
    base: &str,
    key: &LocalKey,
    (sign_method, sign_path, sign_body): Req<'_>,
    (method, path, body): Req<'_>,
    ts: u64,
    nonce: [u8; 16],
) -> (u16, String) {
    let mut req = reqwest::Client::new().request(
        reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
        format!("{base}{path}"),
    );
    for (k, v) in sign_request(key, sign_method, sign_path, ts, &nonce, sign_body) {
        req = req.header(k, v);
    }
    let resp = req.body(body.to_vec()).send().await.unwrap();
    (resp.status().as_u16(), resp.text().await.unwrap())
}

fn code(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn upload_body(task_id: [u8; 32]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "task_id": hex::encode(task_id),
        "input": {"contract": "kaayu-cv-extract-input:v1", "cv_text": CV},
    }))
    .unwrap()
}

fn nonce(n: u8) -> [u8; 16] {
    [n; 16]
}

/// Alice's committed, registered and completed task.
async fn alice_task(r: &Rig) -> ([u8; 32], String) {
    let input = CvExtractionInputV1::new(CV.to_owned()).unwrap();
    let task = build_task(
        r.alice.address(),
        r.executor.address(),
        [0x01u8; 32],
        input.input_commitment(),
    );
    let id = task.task_id();
    let client = FrontClient::new(&r.base, r.alice.clone())
        .unwrap()
        .with_clock(Arc::new(r.clock.clone()));
    let up = client.upload(id, CV).await.unwrap();
    submit_task(&r.chain, &r.alice, &task).await.unwrap();
    r.chain.produce_block();
    client.register(id, &up.object_ref, r.executor.address()).await.unwrap();
    for _ in 0..4 {
        r.chain.produce_block();
        r.front.tick().await.unwrap();
    }
    (id, up.object_ref)
}

#[tokio::test]
async fn e_a01_a02_valid_request_then_exact_replay() {
    let r = rig().await;
    let b = upload_body([0x10u8; 32]);
    let req = ("PUT", "/v1/inputs", &b[..]);
    assert_eq!(
        send(&r.base, &r.alice, req, req, START, nonce(1)).await.0,
        201
    );
    let (s, body) = send(&r.base, &r.alice, req, req, START, nonce(1)).await;
    assert_eq!((s, code(&body).as_str()), (401, "AUTH_REPLAYED"));
}

#[tokio::test]
async fn e_a03_a04_validity_window_edges() {
    let r = rig().await;
    let b = upload_body([0x11u8; 32]);
    let req = ("PUT", "/v1/inputs", &b[..]);
    let cases = [
        (START - MAX_SKEW_SECS - 1, 401, "AUTH_STALE"),
        (START + MAX_SKEW_SECS + 1, 401, "AUTH_STALE"),
        (START - MAX_SKEW_SECS, 201, ""),
        (START + MAX_SKEW_SECS, 201, ""),
        (0, 401, "AUTH_STALE"),
        (u64::MAX, 401, "AUTH_STALE"),
    ];
    assert_eq!(cases.len(), 6);
    for (i, (ts, status, c)) in cases.into_iter().enumerate() {
        let (s, body) = send(&r.base, &r.alice, req, req, ts, nonce(20 + i as u8)).await;
        assert_eq!((s, code(&body).as_str()), (status, c), "ts offset case {i}");
    }
    // A request that was valid becomes stale once the clock moves past it.
    r.clock.advance(MAX_SKEW_SECS + 1);
    let (s, body) = send(&r.base, &r.alice, req, req, START, nonce(40)).await;
    assert_eq!((s, code(&body).as_str()), (401, "AUTH_STALE"));
}

#[tokio::test]
async fn e_a05_a06_a07_a08_route_method_body_and_task_binding() {
    let r = rig().await;
    let (task_a, _) = alice_task(&r).await;
    let b = upload_body([0x12u8; 32]);
    let path_a = format!("/v1/tasks/{}", hex::encode(task_a));
    let result_a = format!("{path_a}/result");
    let path_b = format!("/v1/tasks/{}", hex::encode([0x77u8; 32]));
    let mut tampered = b.clone();
    *tampered.last_mut().unwrap() ^= 0x01;
    let cases: [(&str, Req<'_>, Req<'_>); 5] = [
        (
            "E-A05 upload signature on the result route",
            ("PUT", "/v1/inputs", &b),
            ("GET", &result_a, b""),
        ),
        (
            "E-A05 status signature on the result route",
            ("GET", &path_a, b""),
            ("GET", &result_a, b""),
        ),
        (
            "E-A06 GET signature sent as PUT",
            ("GET", "/v1/inputs", &b),
            ("PUT", "/v1/inputs", &b),
        ),
        (
            "E-A07 one body byte changed",
            ("PUT", "/v1/inputs", &b),
            ("PUT", "/v1/inputs", &tampered),
        ),
        (
            "E-A08 task A signature on task B",
            ("GET", &path_a, b""),
            ("GET", &path_b, b""),
        ),
    ];
    for (i, (name, signed, sent)) in cases.into_iter().enumerate() {
        let (s, body) = send(&r.base, &r.alice, signed, sent, START, nonce(50 + i as u8)).await;
        assert_eq!(
            (s, code(&body).as_str()),
            (401, "AUTH_BAD_SIGNATURE"),
            "{name}"
        );
    }
    // The genuine requests, with fresh nonces, are accepted.
    let ok = send(
        &r.base,
        &r.alice,
        ("GET", &path_a, b""),
        ("GET", &path_a, b""),
        START,
        nonce(60),
    )
    .await;
    assert_eq!(ok.0, 200);
}

#[tokio::test]
async fn e_a09_a10_unrelated_signer_and_cross_user_result_access() {
    let r = rig().await;
    let (task_a, object_ref) = alice_task(&r).await;
    let status = format!("/v1/tasks/{}", hex::encode(task_a));
    let result = format!("{status}/result");
    let register = format!("{status}/register");
    let reg_body = serde_json::to_vec(&serde_json::json!({
        "object_ref": object_ref,
        "executor": hex::encode(r.executor.address().0),
    }))
    .unwrap();
    // Bob signs validly and knows task_id and object_ref: authenticated, not authorized.
    for (i, (method, path, body)) in [
        ("GET", &status, &b""[..]),
        ("GET", &result, &b""[..]),
        ("POST", &register, &reg_body[..]),
    ]
    .into_iter()
    .enumerate()
    {
        let req = (method, path.as_str(), body);
        let (s, b) = send(&r.base, &r.bob, req, req, START, nonce(70 + i as u8)).await;
        assert_eq!(
            (s, code(&b).as_str()),
            (403, "NOT_TASK_SUBMITTER"),
            "{method} {path}"
        );
    }
    // Alice still can.
    let req = ("GET", result.as_str(), &b""[..]);
    assert_eq!(
        send(&r.base, &r.alice, req, req, START, nonce(80)).await.0,
        200
    );
    // The result route returns bytes and fixed headers only: no grant, no locator.
    let mut req = reqwest::Client::new().get(format!("{}{result}", r.base));
    for (k, v) in sign_request(&r.alice, "GET", &result, START, &nonce(81), b"") {
        req = req.header(k, v);
    }
    let resp = req.send().await.unwrap();
    let mut names: Vec<String> = resp.headers().keys().map(|k| k.as_str().to_owned()).collect();
    names.sort();
    assert_eq!(
        names,
        ["cache-control", "content-length", "content-type", "date"]
    );
}

#[tokio::test]
async fn e_a11_malformed_and_foreign_signatures() {
    let r = rig().await;
    let b = upload_body([0x13u8; 32]);
    let send_raw = |headers: Vec<(&'static str, String)>| {
        let mut req = reqwest::Client::new().put(format!("{}/v1/inputs", r.base));
        for (k, v) in headers {
            req = req.header(k, v);
        }
        req.body(b.clone()).send()
    };
    let good = sign_request(&r.alice, "PUT", "/v1/inputs", START, &nonce(90), &b);
    let with = |i: usize, v: &str| {
        let mut h = good.to_vec();
        h[i].1 = v.to_owned();
        h
    };
    let bob_sig = sign_request(&r.bob, "PUT", "/v1/inputs", START, &nonce(90), &b)[3].1.clone();
    let cases: Vec<(&str, Vec<Header>, &str)> = vec![
        ("short signature", with(3, "abcd"), "AUTH_MALFORMED"),
        (
            "non-hex signature",
            with(3, &"zz".repeat(64)),
            "AUTH_MALFORMED",
        ),
        ("short address", with(0, "aa"), "AUTH_MALFORMED"),
        ("non-numeric timestamp", with(1, "soon"), "AUTH_MALFORMED"),
        ("short nonce", with(2, "00"), "AUTH_MALFORMED"),
        ("missing signature", good[..3].to_vec(), "AUTH_MISSING"),
        (
            "bob's signature under alice's address",
            with(3, &bob_sig),
            "AUTH_BAD_SIGNATURE",
        ),
        (
            "zero signature",
            with(3, &"00".repeat(64)),
            "AUTH_BAD_SIGNATURE",
        ),
    ];
    assert_eq!(cases.len(), 8);
    for (name, headers, expected) in cases {
        let resp = send_raw(headers).await.unwrap();
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap();
        assert_eq!((status, code(&body).as_str()), (401, expected), "{name}");
        assert!(
            !body.contains(CV_FRAGMENT),
            "{name}: the error echoes the body"
        );
    }
}

#[tokio::test]
async fn e_a12_oversized_bodies_refused_with_and_without_auth() {
    let r = rig().await;
    let big = vec![b'x'; 70 * 1024];
    let req = ("PUT", "/v1/inputs", &big[..]);
    assert_eq!(
        send(&r.base, &r.alice, req, req, START, nonce(100)).await.0,
        413
    );
    let resp = reqwest::Client::new()
        .put(format!("{}/v1/inputs", r.base))
        .body(big.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 413, "refused before authentication");
}

#[tokio::test]
async fn e_a13_nonce_reuse_by_the_same_signer() {
    let r = rig().await;
    let first = upload_body([0x14u8; 32]);
    let second = upload_body([0x15u8; 32]);
    let a = ("PUT", "/v1/inputs", &first[..]);
    let b = ("PUT", "/v1/inputs", &second[..]);
    assert_eq!(
        send(&r.base, &r.alice, a, a, START, nonce(110)).await.0,
        201
    );
    // A different, validly signed request under the same nonce is refused…
    let (s, body) = send(&r.base, &r.alice, b, b, START, nonce(110)).await;
    assert_eq!((s, code(&body).as_str()), (401, "AUTH_REPLAYED"));
    // …while another signer's identical nonce is independent.
    assert_eq!(send(&r.base, &r.bob, b, b, START, nonce(110)).await.0, 201);
    // And the window ends: once the pair has aged out, the nonce is free again
    // (the timestamp must then be current, so it is no longer the same request).
    r.clock.advance(2 * MAX_SKEW_SECS + 1);
    let now = START + 2 * MAX_SKEW_SECS + 1;
    assert_eq!(send(&r.base, &r.alice, b, b, now, nonce(110)).await.0, 201);
}

#[tokio::test]
async fn e_a14_logs_name_the_signer_and_nothing_secret() {
    let r = rig().await;
    let (task_a, _) = alice_task(&r).await;
    let b = upload_body([0x16u8; 32]);
    let req = ("PUT", "/v1/inputs", &b[..]);
    let headers = sign_request(&r.alice, "PUT", "/v1/inputs", START, &nonce(120), &b);
    let _ = send(&r.base, &r.alice, req, req, START, nonce(120)).await;
    let _ = send(&r.base, &r.alice, req, req, START, nonce(120)).await; // replay
    let result = format!("/v1/tasks/{}/result", hex::encode(task_a));
    let rq = ("GET", result.as_str(), &b""[..]);
    let _ = send(&r.base, &r.bob, rq, rq, START, nonce(121)).await; // forbidden
    let logs = captured();
    let front: Vec<&String> = logs.iter().filter(|l| l.contains("mvp-front:")).collect();
    let alice = hex::encode(&r.alice.address().0[..8]);
    assert!(front
        .iter()
        .any(|l| l.contains(&format!("signer {alice}")) && l.contains("201 OK")));
    assert!(front.iter().any(|l| l.contains("signer -") && l.contains("AUTH_REPLAYED")));
    assert!(front.iter().any(|l| l.contains("NOT_TASK_SUBMITTER")));
    let all = logs.join("\n");
    for (name, secret) in [
        ("signature", headers[3].1.as_str()),
        ("nonce", headers[2].1.as_str()),
        ("client seed", &"aa".repeat(32)),
        ("executor seed", &"e1".repeat(32)),
        ("cv text", CV_FRAGMENT),
    ] {
        assert!(!all.contains(secret), "{name} in logs");
    }
}

#[tokio::test]
async fn e_a15_status_flood_is_answered_and_bounded() {
    let r = rig().await;
    let (task_a, _) = alice_task(&r).await;
    let path = format!("/v1/tasks/{}", hex::encode(task_a));
    let t = Instant::now();
    let n = 300u16;
    for i in 0..n {
        let mut nn = [0u8; 16];
        nn[..2].copy_from_slice(&i.to_le_bytes());
        nn[15] = 0xF0;
        let req = ("GET", path.as_str(), &b""[..]);
        assert_eq!(send(&r.base, &r.alice, req, req, START, nn).await.0, 200);
    }
    assert!(
        t.elapsed() < Duration::from_secs(60),
        "300 status requests took {:?}",
        t.elapsed()
    );
}

#[tokio::test]
async fn e_a16_bad_auth_flood_is_refused_without_state() {
    let r = rig().await;
    let b = upload_body([0x17u8; 32]);
    let mut tampered = b.clone();
    tampered[0] = b'[';
    for i in 0..200u8 {
        let (s, body) = send(
            &r.base,
            &r.alice,
            ("PUT", "/v1/inputs", &b),
            ("PUT", "/v1/inputs", &tampered),
            START,
            [i; 16],
        )
        .await;
        assert_eq!((s, code(&body).as_str()), (401, "AUTH_BAD_SIGNATURE"));
    }
    // None of the refused nonces was recorded: each is still usable.
    let req = ("PUT", "/v1/inputs", &b[..]);
    for i in [0u8, 99, 199] {
        assert_eq!(
            send(&r.base, &r.alice, req, req, START, [i; 16]).await.0,
            201
        );
    }
}

#[tokio::test]
async fn e_a17_repeated_registration_and_executor_substitution() {
    let r = rig().await;
    let (task_a, object_ref) = alice_task(&r).await;
    let client = FrontClient::new(&r.base, r.alice.clone())
        .unwrap()
        .with_clock(Arc::new(r.clock.clone()));
    for _ in 0..5 {
        let s = client.register(task_a, &object_ref, r.executor.address()).await.unwrap();
        assert_eq!(s.attempts, 1, "re-registration changes nothing");
    }
    let e = client
        .register(task_a, &object_ref, mbongo_core::Address([0x66u8; 32]))
        .await
        .unwrap_err();
    // Already registered: idempotent only for the same facts.
    assert_eq!(e.code(), Some("EXECUTOR_MISMATCH"));
    let e = client
        .register(task_a, &hex::encode([0x55u8; 32]), r.executor.address())
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("OBJECT_REF_MISMATCH"));
}
