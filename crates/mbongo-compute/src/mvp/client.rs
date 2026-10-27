//! The client side of MVP-C: signed HTTP calls to the front, and the chain
//! operations the CLI composes with them.
//!
//! Chain operations go through [`ChainClient`] — the crate's `rpc_v0.3`
//! client, the same semantics `@mbongo/sdk` implements (`mbongo_submitTransaction`,
//! blocks by height, account nonce). Nothing here is a new chain protocol:
//! the task is an ordinary `ComputeTask` transaction, the receipt is read
//! from the block that anchors it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use mbongo_core::{
    Address, ComputeTask, Hash, Receipt, Transaction, TransactionPayload, TransactionType,
    COMPUTE_TASK_VERSION,
};
use reqwest::{Method, StatusCode};

use crate::chain::{find_receipt, scan_tasks, ChainClient, ChainError, ObservedTask};
use crate::clock::{Clock, SystemClock};
use crate::data_plane::LocalKey;
use crate::inference::kaayu::INPUT_CONTRACT_V1;

use super::auth::sign_request;
use super::dto::{
    ErrorBody, InputObject, RegisterRequest, StatusView, UploadRequest, UploadResponse,
};
use super::{frozen_execution_spec, MAX_RESULT_BYTES};

/// `n` bytes from the operating system's random source.
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("operating system random source");
    b
}

/// Reads a key seed: a file holding exactly 64 hex characters (32 bytes),
/// optionally followed by a newline. The error never echoes the content.
pub fn load_key_file(path: &Path) -> Result<LocalKey, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read key file {}: {}", path.display(), e.kind()))?;
    let seed: [u8; 32] = hex::decode(text.trim())
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| format!("key file {} is not 64 hex characters", path.display()))?;
    Ok(LocalKey::from_seed(&seed))
}

/// Parses a 32-byte hex value (a task id or an address), with or without
/// a `0x` prefix.
pub fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    hex::decode(s.strip_prefix("0x").unwrap_or(s)).ok()?.try_into().ok()
}

/// Why a call to the front did not succeed. Carries status codes and the
/// front's stable error code; never a request or response body.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The front could not be reached.
    #[error("front unreachable: {0}")]
    Unavailable(String),
    /// The front refused, with its error code.
    #[error("front refused: HTTP {status} {code}")]
    Rejected {
        /// HTTP status.
        status: u16,
        /// The front's error code.
        code: String,
    },
    /// The response was not what the API defines.
    #[error("unexpected response from front: {0}")]
    Protocol(&'static str),
}

impl ClientError {
    /// The front's error code, if it refused.
    pub fn code(&self) -> Option<&str> {
        match self {
            ClientError::Rejected { code, .. } => Some(code),
            _ => None,
        }
    }
}

/// A signed HTTP client for the MVP front.
pub struct FrontClient {
    http: reqwest::Client,
    base: String,
    key: LocalKey,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for FrontClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FrontClient({}, {}, key redacted)",
            self.base,
            self.key.address()
        )
    }
}

impl FrontClient {
    /// A client for the front at `base` (for example `http://127.0.0.1:8740`)
    /// signing with `key`. Inference holds the front for up to a few
    /// minutes, so the request timeout is generous; no proxy is used.
    pub fn new(base: &str, key: LocalKey) -> Result<Self, ClientError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .no_proxy()
            .build()
            .map_err(|_| ClientError::Unavailable("cannot build HTTP client".into()))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            key,
            clock: Arc::new(SystemClock),
        })
    }

    /// Signs with timestamps from `clock` instead of the system clock.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// The address requests are signed by.
    pub fn address(&self) -> Address {
        self.key.address()
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
    ) -> Result<reqwest::Response, ClientError> {
        let headers = sign_request(
            &self.key,
            method.as_str(),
            path,
            self.clock.now(),
            &random_bytes::<16>(),
            &body,
        );
        let mut req = self.http.request(method, format!("{}{path}", self.base));
        for (name, value) in headers {
            req = req.header(name, value);
        }
        if !body.is_empty() {
            req = req.header(reqwest::header::CONTENT_TYPE, "application/json");
        }
        let resp = req
            .body(body)
            .send()
            .await
            .map_err(|e| ClientError::Unavailable(transport_class(&e)))?;
        if resp.status().is_success() {
            return Ok(resp);
        }
        let status = resp.status();
        let code = resp
            .json::<ErrorBody>()
            .await
            .map_or_else(|_| format!("HTTP_{}", status.as_u16()), |b| b.error);
        Err(ClientError::Rejected {
            status: status.as_u16(),
            code,
        })
    }

    /// `PUT /v1/inputs`.
    pub async fn upload(
        &self,
        task_id: [u8; 32],
        cv_text: &str,
    ) -> Result<UploadResponse, ClientError> {
        let body = serde_json::to_vec(&UploadRequest {
            task_id: hex::encode(task_id),
            input: InputObject {
                contract: INPUT_CONTRACT_V1.to_owned(),
                cv_text: cv_text.to_owned(),
            },
        })
        .map_err(|_| ClientError::Protocol("cannot encode upload"))?;
        self.send(Method::PUT, "/v1/inputs", body)
            .await?
            .json()
            .await
            .map_err(|_| ClientError::Protocol("upload response"))
    }

    /// `POST /v1/tasks/{task_id}/register`.
    pub async fn register(
        &self,
        task_id: [u8; 32],
        object_ref: &str,
        executor: Address,
    ) -> Result<StatusView, ClientError> {
        let body = serde_json::to_vec(&RegisterRequest {
            object_ref: object_ref.to_owned(),
            executor: hex::encode(executor.0),
        })
        .map_err(|_| ClientError::Protocol("cannot encode registration"))?;
        let path = format!("/v1/tasks/{}/register", hex::encode(task_id));
        self.send(Method::POST, &path, body)
            .await?
            .json()
            .await
            .map_err(|_| ClientError::Protocol("register response"))
    }

    /// `GET /v1/tasks/{task_id}`.
    pub async fn status(&self, task_id: [u8; 32]) -> Result<StatusView, ClientError> {
        let path = format!("/v1/tasks/{}", hex::encode(task_id));
        self.send(Method::GET, &path, Vec::new())
            .await?
            .json()
            .await
            .map_err(|_| ClientError::Protocol("status response"))
    }

    /// `GET /v1/tasks/{task_id}/result`: the exact bytes the front returned,
    /// never parsed or re-serialized here.
    pub async fn result(&self, task_id: [u8; 32]) -> Result<Vec<u8>, ClientError> {
        let path = format!("/v1/tasks/{}/result", hex::encode(task_id));
        let resp = self.send(Method::GET, &path, Vec::new()).await?;
        if resp.content_length().is_some_and(|n| n > MAX_RESULT_BYTES as u64) {
            return Err(ClientError::Protocol(
                "result larger than the profile allows",
            ));
        }
        let bytes =
            resp.bytes().await.map_err(|e| ClientError::Unavailable(transport_class(&e)))?;
        if bytes.len() > MAX_RESULT_BYTES {
            return Err(ClientError::Protocol(
                "result larger than the profile allows",
            ));
        }
        Ok(bytes.to_vec())
    }
}

/// A transport failure, as a class — reqwest's own message is not printed,
/// since nothing guarantees what it may quote.
fn transport_class(e: &reqwest::Error) -> String {
    let class = if e.is_connect() {
        "connection refused or unreachable"
    } else if e.is_timeout() {
        "timed out"
    } else if e.is_body() || e.is_decode() {
        "response body"
    } else {
        "request failed"
    };
    class.to_owned()
}

/// Whether a refusal is "not yet" rather than "no": the chain has not shown
/// the task to the front yet, or the result is not ready.
pub fn is_not_yet(e: &ClientError) -> bool {
    matches!(
        e,
        ClientError::Rejected { status, code }
            if (*status == StatusCode::NOT_FOUND.as_u16() && code == "TASK_NOT_ON_CHAIN")
                || (*status == StatusCode::CONFLICT.as_u16() && code == "RESULT_NOT_READY")
    )
}

// ── chain operations ────────────────────────────────────────────────────

/// The frozen MVP task: `mbongo-infer:v1` over the active profile `kaayu-cv-extract:v3`,
/// naming `executor`, committing `input_commitment`. Only the existing
/// `ComputeTask` fields; `task_id` is the chain's own derivation.
pub fn build_task(
    submitter: Address,
    executor: Address,
    salt: [u8; 32],
    input_commitment: [u8; 32],
) -> ComputeTask {
    ComputeTask {
        version: COMPUTE_TASK_VERSION,
        submitter,
        executor,
        salt,
        input_commitment,
        execution_spec: frozen_execution_spec(),
    }
}

/// The signed `ComputeTask` transaction.
pub fn task_transaction(key: &LocalKey, task: &ComputeTask, nonce: u64) -> Transaction {
    let mut tx = Transaction {
        tx_type: TransactionType::ComputeTask,
        sender: key.address(),
        receiver: Address::zero(),
        amount: 0,
        nonce,
        payload: TransactionPayload::ComputeTask(Box::new(task.clone())),
        signature: [0u8; 64],
    };
    tx.signature = key.sign(&tx.signing_payload());
    tx
}

/// Submits `task` signed by `key` at the account's next nonce.
pub async fn submit_task<C: ChainClient>(
    chain: &C,
    key: &LocalKey,
    task: &ComputeTask,
) -> Result<Hash, ChainError> {
    let nonce = chain.account_nonce(&key.address()).await?;
    chain.submit_transaction(&task_transaction(key, task, nonce)).await
}

/// The committed task with this id, scanning blocks `from..=latest`.
pub async fn find_task<C: ChainClient>(
    chain: &C,
    task_id: [u8; 32],
    from: u64,
) -> Result<Option<ObservedTask>, ChainError> {
    let latest = chain.latest_height().await?;
    Ok(scan_tasks(chain, from.max(1), latest)
        .await?
        .into_iter()
        .find(|t| t.task_id == task_id))
}

/// The anchored receipt for `task_id`, scanning blocks `from..=latest`.
pub async fn find_anchored_receipt<C: ChainClient>(
    chain: &C,
    task_id: [u8; 32],
    from: u64,
) -> Result<Option<(u64, Receipt)>, ChainError> {
    let latest = chain.latest_height().await?;
    find_receipt(chain, task_id, from, latest).await
}
