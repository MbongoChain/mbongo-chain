//! The real backend: one long-lived `llama-server` child process on
//! loopback, CPU only (MVP-B §12–§14, §40, §44–§45, §62–§64).
//!
//! Start-up verifies the frozen profile, the executable (pinned SHA-256 and
//! self-reported revision), and the model file (size and SHA-256), then
//! spawns the server with structured arguments — no shell, no task data —
//! waits for `/health`, and checks `/props` names the same build and model.
//! Before each request the server is checked alive and restarted if it died
//! or hung; a request that fails never yields a completion.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use crate::inference::job_spec::InferenceJobSpecV1;
use crate::inference::ContractError;

use super::backend::{BackendIdentity, ChatRequest, Completion, InferenceBackend};
use super::config::{RuntimeConfig, LLAMA_CPP_BUILD_NUMBER, LOOPBACK};
use super::http::{self, HttpError};
use super::verify::{
    self, commit_matches, parse_build_info, verify_model, verify_profile_assets,
    verify_runtime_binary,
};

/// Safe facts about a start-up, for readiness logs and the harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupReport {
    /// Self-reported version string.
    pub runtime_version: String,
    /// Self-reported build number.
    pub runtime_build: u64,
    /// Self-reported commit prefix.
    pub runtime_commit_prefix: String,
    /// SHA-256 of the executable.
    pub runtime_sha256: String,
    /// SHA-256 of the model file.
    pub model_sha256: String,
    /// Time spent hashing the model.
    pub model_verify_ms: u128,
    /// Spawn → healthy.
    pub cold_start_ms: u128,
}

struct Process {
    child: Option<Child>,
    restarts: u32,
}

/// The managed `llama-server`.
pub struct LlamaServer {
    cfg: RuntimeConfig,
    identity: BackendIdentity,
    model_fingerprint: (u64, Option<SystemTime>),
    process: Arc<Mutex<Process>>,
}

impl LlamaServer {
    /// Verifies everything, then starts the server. Fails closed: on any
    /// mismatch no process is left running and no backend exists.
    pub fn start(
        cfg: RuntimeConfig,
        spec: &InferenceJobSpecV1,
    ) -> Result<(Self, StartupReport), ContractError> {
        verify_profile_assets(spec)?;
        let reported = verify_runtime_binary(&cfg.runtime_path, &cfg.runtime_sha256, spec)?;
        let t = Instant::now();
        let model_sha256 = verify_model(&cfg.model_path, spec)?;
        let model_verify_ms = t.elapsed().as_millis();
        let model_fingerprint = fingerprint(&cfg.model_path)?;
        let server = Self {
            identity: BackendIdentity {
                runtime_family: spec.runtime.family.clone(),
                runtime_mode: spec.runtime.mode.clone(),
                runtime_revision: spec.runtime.revision.clone(),
                runtime_commit: spec.runtime.commit.clone(),
                model_artifact_sha256: model_sha256.clone(),
            },
            model_fingerprint,
            process: Arc::new(Mutex::new(Process {
                child: None,
                restarts: 0,
            })),
            cfg,
        };
        if probe_health(server.cfg.port) {
            return Err(ContractError::InferenceFailed(
                "the configured port is already serving; refusing to share it".into(),
            ));
        }
        let t = Instant::now();
        server.spawn_and_wait()?;
        let report = StartupReport {
            runtime_version: reported.version,
            runtime_build: reported.build,
            runtime_commit_prefix: reported.commit_prefix,
            runtime_sha256: server.cfg.runtime_sha256.clone(),
            model_sha256,
            model_verify_ms,
            cold_start_ms: t.elapsed().as_millis(),
        };
        log::info!(
            "cpu-inference ready: runtime llama.cpp {} build {} commit {}; model sha256 {}…; threads {}; port {}",
            report.runtime_version,
            report.runtime_build,
            report.runtime_commit_prefix,
            &report.model_sha256[..12],
            server.cfg.threads,
            server.cfg.port
        );
        Ok((server, report))
    }

    /// The configuration in use.
    pub fn config(&self) -> &RuntimeConfig {
        &self.cfg
    }

    /// The OS process id of the running server, if any (memory measurement).
    pub fn pid(&self) -> Option<u32> {
        self.process.lock().unwrap().child.as_ref().map(Child::id)
    }

    /// How many times the server was restarted after start-up.
    pub fn restarts(&self) -> u32 {
        self.process.lock().unwrap().restarts
    }

    /// Kills the server process. Used by fault drills; the next request
    /// restarts it.
    pub fn kill_server(&self) {
        let mut p = self.process.lock().unwrap();
        if let Some(mut c) = p.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    fn args(&self) -> Vec<String> {
        let c = &self.cfg;
        vec![
            "--model".into(),
            c.model_path.to_string_lossy().into_owned(),
            "--host".into(),
            LOOPBACK.into(),
            "--port".into(),
            c.port.to_string(),
            "--n-gpu-layers".into(),
            "0".into(),
            "--threads".into(),
            c.threads.to_string(),
            "--threads-batch".into(),
            c.threads.to_string(),
            "--ctx-size".into(),
            c.ctx_size.to_string(),
            "--parallel".into(),
            "1".into(),
            "--no-webui".into(),
            "--no-slots".into(),
            "--no-cache-prompt".into(),
            "--log-verbosity".into(),
            "2".into(),
        ]
    }

    fn spawn_and_wait(&self) -> Result<(), ContractError> {
        std::fs::create_dir_all(&self.cfg.log_dir).map_err(|_| {
            ContractError::InferenceFailed("cannot create the log directory".into())
        })?;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.cfg.log_dir.join("llama-server.log"))
            .map_err(|_| ContractError::InferenceFailed("cannot open the server log".into()))?;
        let log_err = log
            .try_clone()
            .map_err(|_| ContractError::InferenceFailed("cannot open the server log".into()))?;
        let child = Command::new(&self.cfg.runtime_path)
            .args(self.args())
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .spawn()
            .map_err(|_| ContractError::InferenceFailed("cannot start llama-server".into()))?;
        self.process.lock().unwrap().child = Some(child);
        let deadline = Instant::now() + self.cfg.startup_timeout;
        loop {
            if self.exited() {
                self.kill_server();
                return Err(ContractError::InferenceFailed(
                    "llama-server exited during start-up".into(),
                ));
            }
            if probe_health(self.cfg.port) {
                break;
            }
            if Instant::now() > deadline {
                self.kill_server();
                return Err(ContractError::InferenceFailed(
                    "llama-server not healthy before the start-up timeout".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        if let Err(e) = self.check_props() {
            self.kill_server();
            return Err(e);
        }
        Ok(())
    }

    /// `/props` must name the pinned build and the configured model file:
    /// the process answering on the port is the one this worker verified.
    fn check_props(&self) -> Result<(), ContractError> {
        let r = http::request(
            self.cfg.port,
            "GET",
            "/props",
            None,
            Duration::from_secs(10),
            1 << 20,
        )
        .map_err(|_| ContractError::InferenceFailed("cannot read server properties".into()))?;
        let v: serde_json::Value = serde_json::from_slice(&r.body)
            .map_err(|_| ContractError::InferenceFailed("malformed server properties".into()))?;
        let (build, commit) = v
            .get("build_info")
            .and_then(serde_json::Value::as_str)
            .and_then(parse_build_info)
            .ok_or(ContractError::RuntimeRevisionMismatch)?;
        if build != LLAMA_CPP_BUILD_NUMBER
            || !commit_matches(&commit, &self.identity.runtime_commit)
        {
            return Err(ContractError::RuntimeRevisionMismatch);
        }
        let served = v
            .get("model_path")
            .and_then(serde_json::Value::as_str)
            .map(|p| Path::new(p).file_name().map(std::ffi::OsStr::to_os_string));
        if served.flatten() != self.cfg.model_path.file_name().map(std::ffi::OsStr::to_os_string) {
            return Err(ContractError::ModelDigestMismatch);
        }
        Ok(())
    }

    fn exited(&self) -> bool {
        let mut p = self.process.lock().unwrap();
        match p.child.as_mut() {
            None => true,
            Some(c) => !matches!(c.try_wait(), Ok(None)),
        }
    }

    /// Alive and healthy, or restarted (MVP-B §44). A changed model file
    /// is re-hashed before any restart.
    fn ensure_ready(&self) -> Result<(), ContractError> {
        if !self.exited() && probe_health(self.cfg.port) {
            return Ok(());
        }
        self.kill_server();
        if fingerprint(&self.cfg.model_path)? != self.model_fingerprint {
            let actual = verify::sha256_file(&self.cfg.model_path)
                .map_err(|_| ContractError::ModelNotFound)?;
            if actual != self.identity.model_artifact_sha256 {
                return Err(ContractError::ModelDigestMismatch);
            }
        }
        log::warn!("cpu-inference: llama-server not alive; restarting");
        self.process.lock().unwrap().restarts += 1;
        self.spawn_and_wait()
    }
}

impl InferenceBackend for LlamaServer {
    fn identity(&self) -> BackendIdentity {
        self.identity.clone()
    }

    fn complete(&self, request: &ChatRequest) -> Result<Completion, ContractError> {
        self.ensure_ready()?;
        let body = serde_json::to_vec(&request.body())
            .map_err(|_| ContractError::InferenceFailed("cannot encode request".into()))?;
        let r = http::request(
            self.cfg.port,
            "POST",
            "/v1/chat/completions",
            Some(&body),
            self.cfg.inference_timeout,
            self.cfg.max_response_bytes,
        );
        match r {
            Ok(resp) if resp.status == 200 => Completion::from_body(&resp.body),
            Ok(resp) => Err(ContractError::InferenceFailed(format!(
                "server status {}",
                resp.status
            ))),
            Err(e) => {
                // A hung or dead server is not reused: the next request restarts it.
                if matches!(
                    e,
                    HttpError::Timeout | HttpError::Connect(_) | HttpError::Io(_)
                ) {
                    self.kill_server();
                }
                Err(ContractError::InferenceFailed(match e {
                    HttpError::Timeout => "inference timed out".into(),
                    HttpError::TooLarge(n) => format!("server response exceeds {n} bytes"),
                    other => format!("server unavailable: {other}"),
                }))
            }
        }
    }
}

impl Drop for LlamaServer {
    fn drop(&mut self) {
        self.kill_server();
    }
}

fn probe_health(port: u16) -> bool {
    matches!(
        http::request(port, "GET", "/health", None, Duration::from_secs(2), 4096),
        Ok(r) if r.status == 200
    )
}

fn fingerprint(path: &Path) -> Result<(u64, Option<SystemTime>), ContractError> {
    let m = std::fs::metadata(path).map_err(|_| ContractError::ModelNotFound)?;
    Ok((m.len(), m.modified().ok()))
}
