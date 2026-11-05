//! P3-C1: the client and the provider across a real TLS boundary.
//!
//! `client ──TLS──> terminator ──> gateway ──> provider service ──> runner`
//!
//! The terminator is built here on `rustls`, so these tests run on Linux and
//! Windows alike. The gateway is plain HTTP and misbehaves on request: it is
//! where a real terminator's failures and an attacker's changes are simulated.
//! The certificates are made at test time, and the job is a generic fixture.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use axum::Router;
use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::chain::{scan_tasks, ChainClient};
use mbongo_compute::clock::SystemClock;
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::inference::canonical;
use mbongo_compute::job::{JobSource, MeteringSink, RuntimeAdapter, RuntimeIdentity};
use mbongo_compute::provider::client::{
    run_job, JobClientError, JobReport, ProviderClient, WaitPolicy,
};
use mbongo_compute::provider::durable::{DurableStore, RetentionPolicy};
use mbongo_compute::provider::endpoint::ProviderEndpoint;
use mbongo_compute::provider::runner::{RunnerConfig, SubprocessRuntime};
use mbongo_compute::provider::service::{
    MeteringLog, ProviderConfig, ProviderParts, ProviderService, ProviderStore,
};
use mbongo_core::Address;
use rcgen::{BasicConstraints, Certificate, CertificateParams, IsCa};
use rustls::crypto::ring::default_provider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, SupportedProtocolVersion};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const CLIENT_SEED: [u8; 32] = [0xAA; 32];
const WAIT: WaitPolicy = WaitPolicy {
    timeout: Duration::from_secs(40),
    poll: Duration::from_millis(20),
};

fn scratch() -> PathBuf {
    let run = hex::encode(mbongo_compute::mvp::client::random_bytes::<8>());
    let dir = std::env::temp_dir().join(format!("mbongo-provider-transport-{run}"));
    std::fs::create_dir_all(dir.join("artifacts")).unwrap();
    std::fs::create_dir_all(dir.join("work")).unwrap();
    dir
}

fn job_spec() -> Vec<u8> {
    let text = std::fs::read_to_string(format!("{ROOT}/test-vectors/job/job-contract-v1.json"));
    let vectors: Value = serde_json::from_str(&text.unwrap()).unwrap();
    let specs = vectors["specs"].as_array().unwrap();
    let spec = &specs.iter().find(|s| s["name"] == "fixture-sum").unwrap()["spec"];
    canonical::canonicalize(&spec.to_string(), 1 << 20).unwrap()
}

fn node() -> String {
    let name = format!("node{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|d| d.join(&name))
        .find(|p| p.is_file())
        .expect("node on PATH")
        .display()
        .to_string()
}

// ── certificates ─────────────────────────────────────────────────────

/// A private certificate authority, as a provider would run.
struct Authority(Certificate);

impl Authority {
    fn new() -> Self {
        let mut params = CertificateParams::new(Vec::new());
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        Self(Certificate::from_params(params).unwrap())
    }

    fn pem(&self) -> String {
        self.0.serialize_pem().unwrap()
    }

    fn leaf(&self, name: &str, expired: bool) -> Leaf {
        let mut params = CertificateParams::new(vec![name.to_owned()]);
        if expired {
            params.not_before = rcgen::date_time_ymd(2020, 1, 1);
            params.not_after = rcgen::date_time_ymd(2021, 1, 1);
        }
        let cert = Certificate::from_params(params).unwrap();
        Leaf {
            der: cert.serialize_der_with_signer(&self.0).unwrap(),
            key: cert.serialize_private_key_der(),
        }
    }
}

struct Leaf {
    der: Vec<u8>,
    key: Vec<u8>,
}

// ── the terminator ───────────────────────────────────────────────────

/// Accepts TLS and forwards bytes. Counts what got through the handshake.
struct Terminator {
    address: SocketAddr,
    forwarded: Arc<AtomicUsize>,
}

async fn terminator(
    leaf: &Leaf,
    versions: &[&'static SupportedProtocolVersion],
    backend: SocketAddr,
) -> Terminator {
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf.key.clone()));
    let config = ServerConfig::builder_with_provider(Arc::new(default_provider()))
        .with_protocol_versions(versions)
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![CertificateDer::from(leaf.der.clone())], key)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let forwarded = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&forwarded);
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let (acceptor, counter) = (acceptor.clone(), Arc::clone(&counter));
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(stream).await else {
                    return;
                };
                counter.fetch_add(1, Ordering::SeqCst);
                if let Ok(mut upstream) = TcpStream::connect(backend).await {
                    let _ = tokio::io::copy_bidirectional(&mut tls, &mut upstream).await;
                }
            });
        }
    });
    Terminator { address, forwarded }
}

// ── the gateway ──────────────────────────────────────────────────────

/// What the gateway does to one matching request.
#[derive(Clone)]
enum Fault {
    Status(u16),
    Redirect(String),
    Respond(Vec<u8>),
    AddQuery,
}

struct Rule {
    method: &'static str,
    path_contains: &'static str,
    /// Applied in turn. The last one repeats if `forever`.
    faults: VecDeque<Fault>,
    forever: bool,
}

#[derive(Default)]
struct Gateway {
    backend: String,
    rules: Mutex<Vec<Rule>>,
    seen: Mutex<Vec<(String, String)>>,
    http: reqwest::Client,
}

impl Gateway {
    fn fault_for(&self, method: &str, path: &str) -> Option<Fault> {
        let mut rules = self.rules.lock().unwrap();
        let rule = rules
            .iter_mut()
            .find(|r| r.method == method && path.contains(r.path_contains))?;
        if rule.forever && rule.faults.len() == 1 {
            return rule.faults.front().cloned();
        }
        rule.faults.pop_front()
    }

    fn count(&self, method: &str, path_ends_with: &str) -> usize {
        let seen = self.seen.lock().unwrap();
        seen.iter().filter(|(m, p)| m == method && p.ends_with(path_ends_with)).count()
    }

    fn add(&self, method: &'static str, path_contains: &'static str, faults: &[Fault]) {
        self.rules.lock().unwrap().push(Rule {
            method,
            path_contains,
            faults: faults.iter().cloned().collect(),
            forever: false,
        });
    }

    fn add_forever(&self, method: &'static str, path_contains: &'static str, fault: Fault) {
        self.rules.lock().unwrap().push(Rule {
            method,
            path_contains,
            faults: VecDeque::from([fault]),
            forever: true,
        });
    }
}

async fn forward(State(gateway): State<Arc<Gateway>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 1 << 20).await.unwrap();
    let method = parts.method.as_str().to_owned();
    let path = parts.uri.path().to_owned();
    gateway.seen.lock().unwrap().push((method.clone(), path.clone()));
    let mut target = format!("{}{path}", gateway.backend);
    match gateway.fault_for(&method, &path) {
        Some(Fault::Status(code)) => {
            let html = "<html>bad gateway</html>";
            return Response::builder().status(code).body(Body::from(html)).unwrap();
        }
        Some(Fault::Redirect(location)) => {
            return Response::builder()
                .status(StatusCode::TEMPORARY_REDIRECT)
                .header("location", location)
                .body(Body::empty())
                .unwrap();
        }
        Some(Fault::Respond(bytes)) => return Response::new(Body::from(bytes)),
        Some(Fault::AddQuery) => target.push_str("?injected=1"),
        None => {}
    }
    let mut forwarded = gateway.http.request(parts.method, target).body(bytes.to_vec());
    for (name, value) in &parts.headers {
        if name.as_str().starts_with("x-mbongo-") {
            forwarded = forwarded.header(name.as_str(), value.as_bytes());
        }
    }
    let answer = forwarded.send().await.unwrap();
    let status = answer.status().as_u16();
    let content_type = answer.headers().get("content-type").cloned();
    let body = answer.bytes().await.unwrap();
    let mut response = Response::builder().status(status).body(Body::from(body)).unwrap();
    if let Some(value) = content_type {
        response.headers_mut().insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_bytes(value.as_bytes()).unwrap(),
        );
    }
    response
}

// ── the stack ────────────────────────────────────────────────────────

struct Stack {
    dir: PathBuf,
    chain: FakeChain,
    service: ProviderService<FakeChain>,
    gateway: Arc<Gateway>,
    gateway_address: SocketAddr,
    authority: Authority,
}

async fn start(seed: u8) -> Stack {
    let dir = scratch();
    let chain = FakeChain::new();
    let store = Arc::new(ProviderStore::new(dir.join("artifacts")));
    let runtime = SubprocessRuntime::new(
        RunnerConfig {
            identity: RuntimeIdentity {
                family: "test-fixture".to_owned(),
                revision: "1".to_owned(),
                binary_sha256: None,
                accelerator_class: "cpu".to_owned(),
            },
            command: vec![
                node(),
                format!("{ROOT}/scripts/fixtures/mbongo-runner-fixture.mjs"),
            ],
            pass_env: vec!["SystemRoot".to_owned()],
            work_dir: dir.join("work"),
        },
        Arc::clone(&store) as Arc<dyn JobSource>,
    );
    let durable =
        DurableStore::open(&dir.join("state"), &[7u8; 32], RetentionPolicy::default()).unwrap();
    let service = ProviderService::open(
        chain.clone(),
        Arc::new(SystemClock),
        [seed; 32],
        LocalKey::from_seed(&[seed; 32]),
        ProviderParts {
            store,
            runtimes: vec![Arc::new(runtime) as Arc<dyn RuntimeAdapter>],
            metering: Arc::new(MeteringLog::default()) as Arc<dyn MeteringSink>,
        },
        Arc::new(durable),
        ProviderConfig::default(),
    )
    .await
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend = format!("http://{}", listener.local_addr().unwrap());
    let router = service.router();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let (c, s) = (chain.clone(), service.clone());
    tokio::spawn(async move {
        loop {
            c.produce_block();
            let _ = s.tick().await;
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    });

    let gateway = Arc::new(Gateway {
        backend,
        http: reqwest::Client::builder().no_proxy().build().unwrap(),
        ..Gateway::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway_address = listener.local_addr().unwrap();
    let app = Router::new().fallback(forward).with_state(Arc::clone(&gateway));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Stack {
        dir,
        chain,
        service,
        gateway,
        gateway_address,
        authority: Authority::new(),
    }
}

fn write_endpoint(dir: &Path, executor: Address, url: &str, anchors_pem: &str) -> PathBuf {
    std::fs::write(dir.join("anchors.pem"), anchors_pem).unwrap();
    let file = dir.join("provider.json");
    let text = json!({
        "schema": "provider-endpoint-v1",
        "executor": hex::encode(executor.0),
        "url": url,
        "trust_anchors": "anchors.pem",
    });
    std::fs::write(&file, text.to_string()).unwrap();
    file
}

impl Stack {
    /// A client trusting this stack's authority, for `address`.
    fn client(&self, address: SocketAddr, executor: Address) -> ProviderClient {
        let url = format!("https://localhost:{}", address.port());
        let file = write_endpoint(&self.dir, executor, &url, &self.authority.pem());
        let endpoint = ProviderEndpoint::load(&file).unwrap();
        ProviderClient::secure(&endpoint, LocalKey::from_seed(&CLIENT_SEED)).unwrap()
    }

    async fn secure(&self) -> (Terminator, ProviderClient) {
        let leaf = self.authority.leaf("localhost", false);
        let front = terminator(&leaf, &[&rustls::version::TLS13], self.gateway_address).await;
        let client = self.client(front.address, self.service.executor());
        (front, client)
    }

    async fn job(
        &self,
        client: &ProviderClient,
        wait: WaitPolicy,
    ) -> Result<JobReport, JobClientError> {
        let key = LocalKey::from_seed(&CLIENT_SEED);
        run_job(client, &self.chain, &key, &job_spec(), b"[2,3]", wait).await
    }

    async fn tasks_on_chain(&self) -> usize {
        let latest = self.chain.latest_height().await.unwrap();
        scan_tasks(&self.chain, 0, latest).await.unwrap().len()
    }
}

fn gateway_failure(e: &Result<JobReport, JobClientError>, expected: u16) -> bool {
    matches!(e, Err(JobClientError::Rejected { status, .. }) if *status == expected)
}

// ── a valid connection ───────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_job_across_tls_reaches_t1() {
    let s = start(0xC1).await;
    let (front, client) = s.secure().await;
    let report = s.job(&client, WAIT).await.unwrap();
    assert_eq!(report.technical_level, "T1");
    assert_eq!(report.output, hex::encode(b"{\"count\":2,\"sum\":5}"));
    assert!(front.forwarded.load(Ordering::SeqCst) > 0);
    assert_eq!(s.gateway.count("PUT", "/v1/inputs"), 1);
    assert_eq!(s.tasks_on_chain().await, 1);
}

// ── certificates ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_certificate_the_client_was_not_told_to_trust_is_refused() {
    let s = start(0xC2).await;
    let stranger = Authority::new();
    let cases = [
        ("another authority", stranger.leaf("localhost", false)),
        ("expired", s.authority.leaf("localhost", true)),
        (
            "another host name",
            s.authority.leaf("provider.invalid", false),
        ),
    ];
    for (name, leaf) in &cases {
        let front = terminator(leaf, &[&rustls::version::TLS13], s.gateway_address).await;
        let client = s.client(front.address, s.service.executor());
        let refused = s.job(&client, WAIT).await;
        assert_eq!(refused, Err(JobClientError::TlsRefused), "{name}");
        assert_eq!(front.forwarded.load(Ordering::SeqCst), 0, "{name}");
    }
    assert!(
        s.gateway.seen.lock().unwrap().is_empty(),
        "a request got through"
    );
    assert_eq!(s.tasks_on_chain().await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tls_older_than_1_3_is_refused() {
    let s = start(0xC3).await;
    let leaf = s.authority.leaf("localhost", false);
    let front = terminator(&leaf, &[&rustls::version::TLS12], s.gateway_address).await;
    let client = s.client(front.address, s.service.executor());
    assert_eq!(s.job(&client, WAIT).await, Err(JobClientError::TlsRefused));
    assert!(s.gateway.seen.lock().unwrap().is_empty());
}

// ── configuration ────────────────────────────────────────────────────

#[test]
fn plain_http_is_for_loopback_only() {
    let key = || LocalKey::from_seed(&CLIENT_SEED);
    let executor = Address([1; 32]);
    for allowed in ["http://127.0.0.1:1", "http://[::1]:1", "http://localhost:1"] {
        assert!(
            ProviderClient::new(allowed, key(), executor).is_ok(),
            "{allowed}"
        );
    }
    for refused in [
        "http://provider.invalid",
        "http://192.0.2.1:8080",
        "https://provider.invalid",
        "ftp://127.0.0.1",
        "not a url",
    ] {
        assert!(
            matches!(
                ProviderClient::new(refused, key(), executor),
                Err(JobClientError::InsecureUrl)
            ),
            "{refused}"
        );
    }
}

#[test]
fn an_endpoint_file_says_exactly_what_it_must() {
    let dir = scratch();
    let authority = Authority::new();
    let executor = Address([1; 32]);
    let good = "https://provider.invalid:8443";
    let file = write_endpoint(&dir, executor, good, &authority.pem());
    let loaded = ProviderEndpoint::load(&file).unwrap();
    assert_eq!(loaded.executor(), executor);
    for url in [
        "http://provider.invalid",
        "https://provider.invalid/path",
        "https://provider.invalid?x=1",
        "https://user@provider.invalid",
        "https://provider.invalid#fragment",
    ] {
        let file = write_endpoint(&dir, executor, url, &authority.pem());
        assert!(ProviderEndpoint::load(&file).is_err(), "{url}");
    }
    let file = write_endpoint(&dir, executor, good, "");
    assert!(ProviderEndpoint::load(&file).is_err(), "no anchors");
    let file = write_endpoint(&dir, executor, good, &authority.pem());
    let mut value: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    value["schema"] = json!("provider-endpoint-v2");
    std::fs::write(&file, value.to_string()).unwrap();
    assert!(ProviderEndpoint::load(&file).is_err(), "another schema");
    value["schema"] = json!("provider-endpoint-v1");
    value["verify"] = json!(false);
    std::fs::write(&file, value.to_string()).unwrap();
    assert!(ProviderEndpoint::load(&file).is_err(), "an unknown field");
}

// ── identity ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_wrong_provider_gets_no_job() {
    let s = start(0xC4).await;
    let leaf = s.authority.leaf("localhost", false);
    let front = terminator(&leaf, &[&rustls::version::TLS13], s.gateway_address).await;
    // A valid certificate, for an endpoint that is not this executor's.
    let client = s.client(front.address, Address([9; 32]));
    assert_eq!(
        s.job(&client, WAIT).await,
        Err(JobClientError::WrongProvider)
    );
    assert_eq!(s.gateway.count("PUT", "/v1/job-specs"), 0);
    assert_eq!(s.gateway.count("PUT", "/v1/inputs"), 0);
    assert_eq!(s.tasks_on_chain().await, 0);
    // Even a client that skips that check is refused by the service.
    assert_eq!(
        client.put_job_spec(&job_spec()).await,
        Err(JobClientError::Rejected {
            status: 401,
            code: "AUTH_WRONG_RECIPIENT".to_owned()
        })
    );
}

// ── redirects and tampering ──────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_redirect_is_refused_and_forwards_nothing() {
    let s = start(0xC5).await;
    let (_front, client) = s.secure().await;
    let elsewhere = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!(
        "https://localhost:{}/stolen",
        elsewhere.local_addr().unwrap().port()
    );
    let reached = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&reached);
    tokio::spawn(async move {
        while elsewhere.accept().await.is_ok() {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    s.gateway.add_forever("PUT", "/v1/inputs", Fault::Redirect(target.clone()));
    s.gateway.add_forever("GET", "/v1/capabilities", Fault::Redirect(target));

    assert_eq!(
        client.upload([1; 32], b"[2,3]").await.map(|_| ()),
        Err(JobClientError::Redirected)
    );
    assert_eq!(s.job(&client, WAIT).await, Err(JobClientError::Redirected));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        reached.load(Ordering::SeqCst),
        0,
        "a request followed the redirect"
    );
    assert_eq!(s.tasks_on_chain().await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_query_string_added_in_transit_is_refused_on_every_signed_route() {
    let s = start(0xC6).await;
    let (_front, client) = s.secure().await;
    let refused = Err::<(), _>(JobClientError::Rejected {
        status: 400,
        code: "AUTH_QUERY_NOT_ALLOWED".to_owned(),
    });
    let id = [3; 32];
    s.gateway.add_forever("PUT", "/v1/job-specs", Fault::AddQuery);
    s.gateway.add_forever("PUT", "/v1/inputs", Fault::AddQuery);
    s.gateway.add_forever("POST", "/register", Fault::AddQuery);
    s.gateway.add_forever("GET", "/v1/tasks/", Fault::AddQuery);
    assert_eq!(client.put_job_spec(&job_spec()).await.map(|_| ()), refused);
    assert_eq!(client.upload(id, b"[1]").await.map(|_| ()), refused);
    assert_eq!(
        client.register(id, &"0".repeat(64)).await.map(|_| ()),
        refused
    );
    assert_eq!(client.status(id).await.map(|_| ()), refused);
    assert_eq!(client.result(id).await.map(|_| ()), refused);
}

// ── gateway failures ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tracking_recovers_from_transient_gateway_failures() {
    let s = start(0xC7).await;
    let (_front, client) = s.secure().await;
    let failing = [Fault::Status(502), Fault::Status(503), Fault::Status(504)];
    s.gateway.add("GET", "/result", &failing);
    s.gateway.add("POST", "/register", &[Fault::Status(502)]);
    s.gateway.add("GET", "/v1/tasks/", &failing);
    let report = s.job(&client, WAIT).await.unwrap();
    assert_eq!(report.technical_level, "T1");
    assert_eq!(
        s.gateway.count("PUT", "/v1/inputs"),
        1,
        "the input was sent twice"
    );
    assert_eq!(s.gateway.count("POST", "/register"), 2);
    assert_eq!(s.tasks_on_chain().await, 1, "the job was submitted twice");
}

/// Retries stop at the wait's bound, and none submits the job again.
async fn gives_up_at_its_bound(seed: u8, method: &'static str, path: &'static str) {
    let s = start(seed).await;
    let (_front, client) = s.secure().await;
    s.gateway.add_forever(method, path, Fault::Status(503));
    let wait = WaitPolicy {
        timeout: Duration::from_secs(2),
        poll: Duration::from_millis(100),
    };
    let started = Instant::now();
    let outcome = s.job(&client, wait).await;
    assert!(gateway_failure(&outcome, 503), "{outcome:?}");
    assert!(started.elapsed() < Duration::from_secs(10));
    let attempts = s.gateway.count(method, "");
    assert!((2..=40).contains(&attempts), "{attempts} attempts");
    assert_eq!(s.gateway.count("PUT", "/v1/inputs"), 1);
    assert_eq!(s.tasks_on_chain().await, 1, "the job was submitted again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tracking_gives_up_at_its_bound() {
    gives_up_at_its_bound(0xC8, "GET", "/v1/tasks/").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn registration_gives_up_at_its_bound() {
    gives_up_at_its_bound(0xCB, "POST", "/register").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_upload_is_not_repeated_and_nothing_reaches_the_chain() {
    let s = start(0xC9).await;
    let (_front, client) = s.secure().await;
    s.gateway.add("PUT", "/v1/inputs", &[Fault::Status(502)]);
    let outcome = s.job(&client, WAIT).await;
    assert!(gateway_failure(&outcome, 502), "{outcome:?}");
    assert_eq!(s.gateway.count("PUT", "/v1/inputs"), 1);
    assert_eq!(s.tasks_on_chain().await, 0);
}

// ── the result does not depend on the transport ──────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_valid_certificate_does_not_make_a_wrong_result_right() {
    let s = start(0xCA).await;
    let (_front, client) = s.secure().await;
    let forged = b"{\"count\":2,\"sum\":6}".to_vec();
    s.gateway.add_forever("GET", "/result", Fault::Respond(forged));
    let outcome = s.job(&client, WAIT).await;
    assert!(
        matches!(outcome, Err(JobClientError::Verification(_))),
        "{outcome:?}"
    );
}
