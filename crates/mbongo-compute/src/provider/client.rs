//! The generic client: submits a job to one provider, then verifies the
//! result against the chain. It trusts neither the provider's status nor its
//! capability declaration: the receipt and the bytes decide.
//!
//! What a success proves stops at T1: the named executor signed and anchored
//! a receipt for this task, and the bytes match its output commitment and the
//! job's declared type and size. It does not prove the output is right for
//! the client's purpose, or that the declared runtime ran.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mbongo_core::{Address, ComputeTask, Receipt, COMPUTE_TASK_VERSION};
use mbongo_verification::verify_receipt_signature;
use reqwest::redirect::Policy;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};

use crate::chain::{find_receipt, ChainClient};
use crate::clock::{Clock, SystemClock};
use crate::data_plane::LocalKey;
use crate::execution::reference_input_commitment;
use crate::job::{verify_result, JobSpec, TechnicalLevel};
use crate::mvp::client::{find_task, random_bytes, submit_task};

use super::auth::{sign_request, SignedRequest};
use super::endpoint::ProviderEndpoint;
use super::service::{
    Capabilities, ErrorBody, JobSpecAccepted, JobStatus, RegisterRequest, UploadRequest,
    UploadResponse,
};

/// Lifetime of a signed request.
const REQUEST_WINDOW_SECS: u64 = 60;

/// Why a job did not produce a verified result. Carries codes, not content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JobClientError {
    /// The provider could not be reached.
    #[error("provider unavailable: {0}")]
    Unavailable(&'static str),
    /// Plain HTTP to anything but loopback, or a URL this client cannot use.
    #[error("PROVIDER_INSECURE_URL")]
    InsecureUrl,
    /// The TLS handshake failed: no request was sent.
    #[error("PROVIDER_TLS_REFUSED")]
    TlsRefused,
    /// The provider answered with a redirect. It is not followed.
    #[error("PROVIDER_REDIRECT_REFUSED")]
    Redirected,
    /// The provider refused the request.
    #[error("refused ({status}): {code}")]
    Rejected {
        /// HTTP status.
        status: u16,
        /// The code.
        code: String,
    },
    /// The chain could not be reached.
    #[error("chain: {0}")]
    Chain(String),
    /// The provider stopped trying.
    #[error("job failed after {attempts} attempt(s): {code}")]
    JobFailed {
        /// Attempts issued.
        attempts: u32,
        /// The failure code.
        code: String,
    },
    /// The provider's identity is not the one the client pinned.
    #[error("the provider declares another executor")]
    WrongProvider,
    /// The job did not finish in time.
    #[error("timed out: {0}")]
    TimedOut(&'static str),
    /// The result failed verification.
    #[error("verification failed: {0}")]
    Verification(String),
}

impl JobClientError {
    /// A failure another attempt may not repeat: the provider or a gateway in
    /// front of it is out of reach for now.
    fn is_transient(&self) -> bool {
        match self {
            Self::Unavailable(_) => true,
            Self::Rejected { status, .. } => matches!(status, 502..=504),
            _ => false,
        }
    }
}

/// A signing client for one provider.
pub struct ProviderClient {
    http: reqwest::Client,
    base: String,
    key: LocalKey,
    recipient: Address,
    clock: Arc<dyn Clock>,
}

/// A failed handshake surfaces as invalid data, possibly wrapped in another
/// I/O error, whose `source` skips what it wraps.
fn is_invalid_data(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::InvalidData
        || error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<std::io::Error>())
            .is_some_and(is_invalid_data)
}

fn is_tls_failure(e: &reqwest::Error) -> bool {
    // Only while connecting: a malformed body is invalid data too.
    if !e.is_connect() {
        return false;
    }
    let mut source = std::error::Error::source(e);
    while let Some(cause) = source {
        if cause.downcast_ref::<std::io::Error>().is_some_and(is_invalid_data) {
            return true;
        }
        source = cause.source();
    }
    false
}

fn class(e: &reqwest::Error) -> JobClientError {
    if is_tls_failure(e) {
        return JobClientError::TlsRefused;
    }
    JobClientError::Unavailable(if e.is_connect() {
        "connection refused or unreachable"
    } else if e.is_timeout() {
        "timed out"
    } else {
        "request failed"
    })
}

impl ProviderClient {
    fn build(
        builder: reqwest::ClientBuilder,
        base: &str,
        key: LocalKey,
        recipient: Address,
    ) -> Result<Self, JobClientError> {
        // A redirect would carry a signed request, and its input, elsewhere.
        let http = builder
            .redirect(Policy::none())
            .timeout(Duration::from_secs(600))
            .no_proxy()
            .build()
            .map_err(|_| JobClientError::Unavailable("cannot build HTTP client"))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            key,
            recipient,
            clock: Arc::new(SystemClock),
        })
    }

    /// A client for a provider on this host, over plain HTTP, whose executor is
    /// `recipient`. Anything off loopback goes through [`Self::secure`].
    pub fn new(base: &str, key: LocalKey, recipient: Address) -> Result<Self, JobClientError> {
        let url = Url::parse(base).map_err(|_| JobClientError::InsecureUrl)?;
        // An address, not a name: a name can resolve somewhere else.
        let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
        let loopback = host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
        if url.scheme() != "http" || !loopback {
            return Err(JobClientError::InsecureUrl);
        }
        Self::build(reqwest::Client::builder(), base, key, recipient)
    }

    /// A client for a provider across a network: TLS 1.3 only, and the
    /// authorities its endpoint file names, in a store built from nothing else.
    /// The chain and the host name are verified, and no option turns that off.
    /// HTTP/1.1: HTTP/2 would let the client resend a signed request by itself.
    pub fn secure(endpoint: &ProviderEndpoint, key: LocalKey) -> Result<Self, JobClientError> {
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| JobClientError::Unavailable("cannot build TLS configuration"))?
        .with_root_certificates(endpoint.roots().clone())
        .with_no_client_auth();
        let builder = reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .https_only(true)
            .http1_only();
        Self::build(builder, &endpoint.base(), key, endpoint.executor())
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
    ) -> Result<reqwest::Response, JobClientError> {
        let issued_at = self.clock.now();
        let headers = sign_request(
            &self.key,
            &SignedRequest {
                recipient: self.recipient,
                method: method.as_str(),
                path,
                body: &body,
                issued_at,
                expires_at: issued_at + REQUEST_WINDOW_SECS,
                request_id: random_bytes::<16>(),
            },
        );
        let mut req = self.http.request(method, format!("{}{path}", self.base));
        for (name, value) in headers {
            req = req.header(name, value);
        }
        let resp = req.body(body).send().await.map_err(|e| class(&e))?;
        Self::checked(resp).await
    }

    async fn checked(resp: reqwest::Response) -> Result<reqwest::Response, JobClientError> {
        if resp.status().is_success() {
            return Ok(resp);
        }
        if resp.status().is_redirection() {
            return Err(JobClientError::Redirected);
        }
        let status = resp.status().as_u16();
        let code = resp
            .json::<ErrorBody>()
            .await
            .map_or_else(|_| format!("HTTP_{status}"), |b| b.error);
        Err(JobClientError::Rejected { status, code })
    }

    async fn json<T: for<'de> Deserialize<'de>>(
        resp: reqwest::Response,
    ) -> Result<T, JobClientError> {
        resp.json()
            .await
            .map_err(|_| JobClientError::Unavailable("unreadable response"))
    }

    /// `GET /v1/capabilities`: claims, not proofs.
    pub async fn capabilities(&self) -> Result<Capabilities, JobClientError> {
        let resp = self
            .http
            .get(format!("{}/v1/capabilities", self.base))
            .send()
            .await
            .map_err(|e| class(&e))?;
        Self::json(Self::checked(resp).await?).await
    }

    /// `PUT /v1/job-specs`: the `execution_spec` to commit.
    pub async fn put_job_spec(&self, spec: &[u8]) -> Result<Vec<u8>, JobClientError> {
        let resp = self.send(Method::PUT, "/v1/job-specs", spec.to_vec()).await?;
        let accepted: JobSpecAccepted = Self::json(resp).await?;
        hex::decode(accepted.execution_spec)
            .map_err(|_| JobClientError::Unavailable("unreadable response"))
    }

    /// `PUT /v1/inputs`.
    pub async fn upload(
        &self,
        task_id: [u8; 32],
        input: &[u8],
    ) -> Result<UploadResponse, JobClientError> {
        let body = serde_json::to_vec(&UploadRequest {
            task_id: hex::encode(task_id),
            input: hex::encode(input),
        })
        .map_err(|_| JobClientError::Unavailable("cannot encode upload"))?;
        Self::json(self.send(Method::PUT, "/v1/inputs", body).await?).await
    }

    /// `POST /v1/tasks/{task_id}/register`.
    pub async fn register(
        &self,
        task_id: [u8; 32],
        object_ref: &str,
    ) -> Result<JobStatus, JobClientError> {
        let body = serde_json::to_vec(&RegisterRequest {
            object_ref: object_ref.to_owned(),
        })
        .map_err(|_| JobClientError::Unavailable("cannot encode registration"))?;
        let path = format!("/v1/tasks/{}/register", hex::encode(task_id));
        Self::json(self.send(Method::POST, &path, body).await?).await
    }

    /// `GET /v1/tasks/{task_id}`.
    pub async fn status(&self, task_id: [u8; 32]) -> Result<JobStatus, JobClientError> {
        let path = format!("/v1/tasks/{}", hex::encode(task_id));
        Self::json(self.send(Method::GET, &path, Vec::new()).await?).await
    }

    /// `GET /v1/tasks/{task_id}/result`: the exact bytes, never re-serialized.
    pub async fn result(&self, task_id: [u8; 32]) -> Result<Vec<u8>, JobClientError> {
        let path = format!("/v1/tasks/{}/result", hex::encode(task_id));
        let resp = self.send(Method::GET, &path, Vec::new()).await?;
        let bytes = resp.bytes().await.map_err(|e| class(&e))?;
        Ok(bytes.to_vec())
    }
}

/// What a verified job established. Every field is a fact about commitments
/// and signatures; none is a statement about business correctness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobReport {
    /// Hex task id.
    pub task_id: String,
    /// Hex `execution_spec`.
    pub execution_spec: String,
    /// Hex input commitment.
    pub input_commitment: String,
    /// Hex output commitment, from the anchored receipt.
    pub output_commitment: String,
    /// Height of the anchored receipt.
    pub receipt_height: u64,
    /// Attempts the provider reported. Untrusted.
    pub attempts: u32,
    /// The highest level reached.
    pub technical_level: String,
    /// Always `NOT_ASSESSED`.
    pub business_correctness: String,
    /// Hex output bytes.
    pub output: String,
}

/// How long and how often to wait.
#[derive(Debug, Clone, Copy)]
pub struct WaitPolicy {
    /// Give up after this long.
    pub timeout: Duration,
    /// Time between polls.
    pub poll: Duration,
}

fn chain_error(e: impl std::fmt::Display) -> JobClientError {
    JobClientError::Chain(e.to_string())
}

/// Checks a receipt against the task it should answer.
fn check_receipt(receipt: &Receipt, task: &ComputeTask) -> Result<(), JobClientError> {
    let failed = |what: &str| Err(JobClientError::Verification(what.to_owned()));
    if receipt.task_id != task.task_id() {
        return failed("receipt names another task");
    }
    if receipt.executor != task.executor {
        return failed("receipt names another executor");
    }
    if receipt.input_commitment != task.input_commitment {
        return failed("receipt names another input");
    }
    if !verify_receipt_signature(receipt) {
        return failed("receipt signature");
    }
    Ok(())
}

async fn wait_completed(
    client: &ProviderClient,
    task_id: [u8; 32],
    wait: WaitPolicy,
    started: Instant,
) -> Result<JobStatus, JobClientError> {
    loop {
        let status = match client.status(task_id).await {
            Ok(status) => status,
            // A provider restarting is not a failed job.
            Err(e) if e.is_transient() && started.elapsed() < wait.timeout => {
                tokio::time::sleep(wait.poll).await;
                continue;
            }
            Err(e) => return Err(e),
        };
        if status.state == "completed" {
            return Ok(status);
        }
        if status.parked {
            return Err(JobClientError::JobFailed {
                attempts: status.attempts,
                code: status.failure_code.unwrap_or_else(|| "UNKNOWN".to_owned()),
            });
        }
        if started.elapsed() > wait.timeout {
            return Err(JobClientError::TimedOut("job did not finish"));
        }
        tokio::time::sleep(wait.poll).await;
    }
}

/// Reading a result changes nothing, so a gateway failure can be retried.
async fn fetch_result(
    client: &ProviderClient,
    task_id: [u8; 32],
    wait: WaitPolicy,
    started: Instant,
) -> Result<Vec<u8>, JobClientError> {
    loop {
        match client.result(task_id).await {
            Err(e) if e.is_transient() && started.elapsed() < wait.timeout => {
                tokio::time::sleep(wait.poll).await;
            }
            outcome => return outcome,
        }
    }
}

/// Submits a job, waits, and verifies the result up to T1.
///
/// What is retried, until `wait.timeout`, is what repeats safely: tracking, and
/// registration, which the service treats as idempotent. Each retry is a fresh
/// signature. The job spec, the input and the on-chain task are sent once: a
/// failure there ends the job, and nothing submits it again.
pub async fn run_job<C: ChainClient>(
    client: &ProviderClient,
    chain: &C,
    key: &LocalKey,
    job_spec: &[u8],
    input: &[u8],
    wait: WaitPolicy,
) -> Result<JobReport, JobClientError> {
    let started = Instant::now();
    let spec = JobSpec::from_canonical_bytes(job_spec)
        .map_err(|e| JobClientError::Verification(e.to_string()))?;
    let declared = client.capabilities().await?;
    if declared.executor != hex::encode(client.recipient.0) {
        return Err(JobClientError::WrongProvider);
    }
    // Refused here, before anything reaches the chain, if it cannot run.
    let execution_spec = client.put_job_spec(job_spec).await?;

    let task = ComputeTask {
        version: COMPUTE_TASK_VERSION,
        submitter: key.address(),
        executor: client.recipient,
        salt: random_bytes::<32>(),
        input_commitment: reference_input_commitment(input),
        execution_spec: execution_spec.clone(),
    };
    let task_id = task.task_id();
    let upload = client.upload(task_id, input).await?;
    if upload.input_commitment != hex::encode(task.input_commitment) {
        return Err(JobClientError::Verification(
            "the provider computed another input commitment".to_owned(),
        ));
    }
    let start_height = chain.latest_height().await.map_err(chain_error)?;
    submit_task(chain, key, &task).await.map_err(chain_error)?;
    let task_height = loop {
        if let Some(t) = find_task(chain, task_id, start_height).await.map_err(chain_error)? {
            break t.height;
        }
        if started.elapsed() > wait.timeout {
            return Err(JobClientError::TimedOut("task not included"));
        }
        tokio::time::sleep(wait.poll).await;
    };
    loop {
        match client.register(task_id, &upload.object_ref).await {
            Ok(_) => break,
            Err(JobClientError::Rejected { code, .. })
                if code == "TASK_NOT_ON_CHAIN" && started.elapsed() < wait.timeout =>
            {
                tokio::time::sleep(wait.poll).await;
            }
            Err(e) if e.is_transient() && started.elapsed() < wait.timeout => {
                tokio::time::sleep(wait.poll).await;
            }
            Err(e) => return Err(e),
        }
    }
    let status = wait_completed(client, task_id, wait, started).await?;

    let output = fetch_result(client, task_id, wait, started).await?;
    let latest = chain.latest_height().await.map_err(chain_error)?;
    let (receipt_height, receipt) = find_receipt(chain, task_id, task_height, latest)
        .await
        .map_err(chain_error)?
        .ok_or_else(|| JobClientError::Verification("no anchored receipt".to_owned()))?;
    check_receipt(&receipt, &task)?;
    let level = verify_result(&spec, &output, &receipt.output_commitment)
        .map_err(|e| JobClientError::Verification(e.to_string()))?;
    debug_assert_eq!(level, TechnicalLevel::T1);
    Ok(JobReport {
        task_id: hex::encode(task_id),
        execution_spec: hex::encode(execution_spec),
        input_commitment: hex::encode(task.input_commitment),
        output_commitment: hex::encode(receipt.output_commitment),
        receipt_height,
        attempts: status.attempts,
        technical_level: format!("{level:?}"),
        business_correctness: "NOT_ASSESSED".to_owned(),
        output: hex::encode(output),
    })
}
