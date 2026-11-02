//! The inference backend boundary (MVP-B §61): the profile asks a backend
//! for one chat completion and knows nothing about processes or HTTP. The
//! real backend is [`super::llama::LlamaServer`]; tests use
//! [`testing::FakeBackend`]. Neither ever logs a prompt or a completion.

use crate::inference::job_spec::InferenceJobSpecV1;
use crate::inference::ContractError;

/// One chat completion request, built only from the committed job spec, the
/// frozen prompt template and the validated input (MVP-B §24–§25).
#[derive(Clone, PartialEq, Eq)]
pub struct ChatRequest {
    /// System message: the frozen prompt template, verbatim.
    pub system: String,
    /// User message: the CV text, verbatim.
    pub user: String,
    /// From `generation.temperature_milli`.
    pub temperature_milli: u64,
    /// From `generation.top_p_milli`.
    pub top_p_milli: u64,
    /// From `generation.seed`.
    pub seed: u64,
    /// From `generation.max_tokens`.
    pub max_tokens: u64,
    /// The frozen output schema, parsed from its pinned bytes.
    pub schema: serde_json::Value,
    /// The pinned schema bytes, sent verbatim when present. Property order
    /// matters: llama.cpp v0.7.0 builds the grammar with required properties
    /// in the order the schema lists them, so a re-serialized (key-sorted)
    /// schema would change the order in which the model writes fields.
    pub schema_text: Option<String>,
}

impl std::fmt::Debug for ChatRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatRequest")
            .field("system_bytes", &self.system.len())
            .field("user_bytes", &self.user.len())
            .field("temperature_milli", &self.temperature_milli)
            .field("top_p_milli", &self.top_p_milli)
            .field("seed", &self.seed)
            .field("max_tokens", &self.max_tokens)
            .finish_non_exhaustive()
    }
}

impl ChatRequest {
    /// Builds the request for `spec`. Every sampling value comes from the
    /// job spec; nothing is added (MVP-B §25).
    pub fn new(
        spec: &InferenceJobSpecV1,
        system: &str,
        user: &str,
        schema: serde_json::Value,
    ) -> Self {
        Self {
            system: system.to_owned(),
            user: user.to_owned(),
            temperature_milli: spec.generation.temperature_milli,
            top_p_milli: spec.generation.top_p_milli,
            seed: spec.generation.seed,
            max_tokens: spec.generation.max_tokens,
            schema,
            schema_text: None,
        }
    }

    /// Sends `text` — the pinned schema file — verbatim as the schema.
    #[must_use]
    pub fn with_schema_text(mut self, text: &str) -> Self {
        self.schema_text = Some(text.to_owned());
        self
    }

    /// The exact request bytes: [`Self::body`] serialized, with the pinned
    /// schema bytes spliced in verbatim when [`Self::schema_text`] is set.
    pub fn body_bytes(&self) -> Vec<u8> {
        const SLOT: &str = "__mbongo_pinned_schema__";
        let Some(text) = &self.schema_text else {
            return self.body().to_string().into_bytes();
        };
        let mut body = self.body();
        body["response_format"]["json_schema"]["schema"] = serde_json::Value::String(SLOT.into());
        body.to_string()
            .replacen(&format!("\"{SLOT}\""), text.trim_end(), 1)
            .into_bytes()
    }

    /// The `/v1/chat/completions` body for llama.cpp v0.7.0. Besides the
    /// job-spec values it sets only `stream: false` (one response) and
    /// `cache_prompt: false` (no KV reuse across requests: the server
    /// documents that reuse can change logits, and the job spec does not
    /// say "reuse").
    #[allow(clippy::cast_precision_loss)]
    pub fn body(&self) -> serde_json::Value {
        serde_json::json!({
            "messages": [
                { "role": "system", "content": self.system },
                { "role": "user", "content": self.user },
            ],
            "temperature": self.temperature_milli as f64 / 1000.0,
            "top_p": self.top_p_milli as f64 / 1000.0,
            "seed": self.seed,
            "max_tokens": self.max_tokens,
            "stream": false,
            "cache_prompt": false,
            "response_format": {
                "type": "json_schema",
                "json_schema": { "schema": self.schema },
            },
        })
    }
}

/// One completion. `content` is private; the counters are untrusted
/// operational observations (MVP-B §37).
#[derive(Clone, PartialEq, Eq)]
pub struct Completion {
    /// The model's message content, raw.
    pub content: String,
    /// `finish_reason` as reported.
    pub finish_reason: Option<String>,
    /// Prompt tokens as reported.
    pub prompt_tokens: Option<u64>,
    /// Completion tokens as reported.
    pub completion_tokens: Option<u64>,
}

impl std::fmt::Debug for Completion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Completion")
            .field("content_bytes", &self.content.len())
            .field("finish_reason", &self.finish_reason)
            .field("prompt_tokens", &self.prompt_tokens)
            .field("completion_tokens", &self.completion_tokens)
            .finish()
    }
}

impl Completion {
    /// Parses an OpenAI-shaped chat completion body. A body that is not that
    /// shape is a server malfunction (`INFERENCE_FAILED`), distinct from model
    /// output that is not JSON (`OUTPUT_NOT_JSON`, decided later by `finish`).
    pub fn from_body(body: &[u8]) -> Result<Self, ContractError> {
        let v: serde_json::Value = serde_json::from_slice(body)
            .map_err(|_| ContractError::InferenceFailed("malformed server response".into()))?;
        let choice = v.get("choices").and_then(|c| c.get(0)).ok_or_else(|| {
            ContractError::InferenceFailed("server response has no choice".into())
        })?;
        let content = choice
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ContractError::InferenceFailed("server response has no content".into()))?
            .to_owned();
        let usage = v.get("usage");
        let count = |k: &str| usage.and_then(|u| u.get(k)).and_then(serde_json::Value::as_u64);
        Ok(Self {
            content,
            finish_reason: choice
                .get("finish_reason")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            prompt_tokens: count("prompt_tokens"),
            completion_tokens: count("completion_tokens"),
        })
    }
}

/// Identity facts a backend proved at start-up; the profile compares them to
/// every task's job spec (contract §17 steps 5–6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendIdentity {
    /// `llama.cpp`.
    pub runtime_family: String,
    /// `llama-server`.
    pub runtime_mode: String,
    /// The release tag the verified binary belongs to.
    pub runtime_revision: String,
    /// The full commit the verified binary was built from.
    pub runtime_commit: String,
    /// SHA-256 of the model file the server loaded.
    pub model_artifact_sha256: String,
}

/// A local inference backend.
pub trait InferenceBackend: Send + Sync {
    /// What the backend proved about itself at start-up.
    fn identity(&self) -> BackendIdentity;
    /// One completion. Errors are application-level and carry no content.
    fn complete(&self, request: &ChatRequest) -> Result<Completion, ContractError>;
}

/// Test doubles. Not a production path.
pub mod testing {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::{BackendIdentity, ChatRequest, Completion, InferenceBackend};
    use crate::inference::{kaayu, ContractError};

    /// A scripted backend: answers from a queue (the last answer repeats)
    /// and records every request it received.
    pub struct FakeBackend {
        identity: BackendIdentity,
        answers: Mutex<VecDeque<Result<Completion, ContractError>>>,
        last: Mutex<Option<Result<Completion, ContractError>>>,
        seen: Mutex<Vec<ChatRequest>>,
    }

    impl FakeBackend {
        /// A backend whose identity is the frozen job spec's runtime and model.
        pub fn pinned() -> Self {
            let spec = kaayu::job_spec();
            Self::with_identity(BackendIdentity {
                runtime_family: spec.runtime.family,
                runtime_mode: spec.runtime.mode,
                runtime_revision: spec.runtime.revision,
                runtime_commit: spec.runtime.commit,
                model_artifact_sha256: spec.model.artifact_sha256,
            })
        }

        /// A backend claiming `identity`.
        pub fn with_identity(identity: BackendIdentity) -> Self {
            Self {
                identity,
                answers: Mutex::new(VecDeque::new()),
                last: Mutex::new(None),
                seen: Mutex::new(Vec::new()),
            }
        }

        /// Queues an answer.
        pub fn push(&self, answer: Result<Completion, ContractError>) {
            self.answers.lock().unwrap().push_back(answer);
        }

        /// Queues a successful completion with `content`.
        pub fn push_content(&self, content: &str) {
            self.push(Ok(Completion {
                content: content.to_owned(),
                finish_reason: Some("stop".into()),
                prompt_tokens: Some(573),
                completion_tokens: Some(337),
            }));
        }

        /// Every request received, in order.
        pub fn requests(&self) -> Vec<ChatRequest> {
            self.seen.lock().unwrap().clone()
        }

        /// How many completions were requested.
        pub fn calls(&self) -> usize {
            self.seen.lock().unwrap().len()
        }
    }

    impl InferenceBackend for FakeBackend {
        fn identity(&self) -> BackendIdentity {
            self.identity.clone()
        }

        fn complete(&self, request: &ChatRequest) -> Result<Completion, ContractError> {
            self.seen.lock().unwrap().push(request.clone());
            let next = self.answers.lock().unwrap().pop_front();
            let mut last = self.last.lock().unwrap();
            if let Some(a) = next {
                *last = Some(a);
            }
            last.clone()
                .unwrap_or_else(|| Err(ContractError::InferenceFailed("no scripted answer".into())))
        }
    }
}
