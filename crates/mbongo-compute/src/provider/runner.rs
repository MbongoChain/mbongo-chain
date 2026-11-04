//! [`SubprocessRuntime`]: `mbongo-runner:v1`, a provider's runtime as an OS
//! process. Any language can implement it, and no workload is compiled in.
//!
//! **Alone, this is interoperability, not isolation.** The environment is
//! cleared, but that gives no filesystem, network or OS isolation: a runner
//! under the worker's account can still read the executor key file, and only
//! the direct child is killed on a limit. [`SubprocessRuntime::contained`]
//! adds what [`super::containment`] enforces.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::containment::{Containment, JobCgroup};
use crate::job::{
    JobSource, ResolvedJob, RunOutput, RuntimeAdapter, RuntimeFailed, RuntimeIdentity,
    UntrustedMetering,
};

/// The version every frame opens with.
pub const RUNNER_CONTRACT: &str = "mbongo-runner:v1";
/// Longest header line, newline included.
pub const MAX_HEADER_BYTES: usize = 128;
/// Added to `max_wall_ms` before the kill, so that the worker's own deadline
/// reports `JOB_LIMIT_WALL_TIME` first.
const KILL_GRACE: Duration = Duration::from_millis(50);
const POLL: Duration = Duration::from_millis(5);

/// How a provider runs one runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerConfig {
    /// What the runtime states about itself. Self-declared.
    pub identity: RuntimeIdentity,
    /// The executable, then its arguments.
    pub command: Vec<String>,
    /// Environment variables passed through from the service, by name.
    pub pass_env: Vec<String>,
    /// Parent of the per-job directories.
    pub work_dir: PathBuf,
}

/// A runtime reached through `mbongo-runner:v1`.
pub struct SubprocessRuntime {
    config: RunnerConfig,
    source: Arc<dyn JobSource>,
    next_dir: AtomicU64,
    containment: Option<Arc<Containment>>,
}

/// A job directory, deleted when dropped.
struct JobDir(PathBuf);

impl Drop for JobDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// What the runner's stdout held.
enum Response {
    Output(Vec<u8>),
    /// More than the limit: `max_bytes + 1` bytes, so the size check refuses it.
    TooLarge(Vec<u8>),
    Malformed,
}

fn read_header(reader: &mut impl BufRead) -> Option<String> {
    let mut line = Vec::new();
    let limit = u64::try_from(MAX_HEADER_BYTES).ok()?;
    reader.take(limit).read_until(b'\n', &mut line).ok()?;
    if line.pop() != Some(b'\n') {
        return None;
    }
    String::from_utf8(line).ok()
}

/// Parses `mbongo-runner:v1 ok <output_len>`.
fn parse_header(line: &str) -> Option<u64> {
    let mut words = line.split(' ');
    let (contract, status, len) = (words.next()?, words.next()?, words.next()?);
    let digits = !len.is_empty() && len.bytes().all(|b| b.is_ascii_digit());
    (contract == RUNNER_CONTRACT && status == "ok" && digits && words.next().is_none())
        .then(|| len.parse().ok())
        .flatten()
}

/// Reads one response, collecting at most `max_bytes + 1` bytes of body.
fn read_response(stdout: ChildStdout, max_bytes: u64) -> Response {
    let mut reader = BufReader::new(stdout);
    let Some(declared) = read_header(&mut reader).as_deref().and_then(parse_header) else {
        return Response::Malformed;
    };
    let over = usize::try_from(max_bytes + 1).unwrap_or(usize::MAX);
    if declared > max_bytes {
        return Response::TooLarge(vec![0; over]);
    }
    let mut body = Vec::new();
    if reader.take(max_bytes + 1).read_to_end(&mut body).is_err() {
        return Response::Malformed;
    }
    if body.len() == over {
        return Response::TooLarge(body);
    }
    if u64::try_from(body.len()).ok() == Some(declared) {
        Response::Output(body)
    } else {
        Response::Malformed
    }
}

fn hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// A contained runner is under another account, so the service cannot signal
/// it, and its descendants are the cgroup's to kill anyway.
fn kill(child: &mut Child, group: Option<&JobCgroup>) {
    match group {
        Some(group) => group.kill(),
        None => {
            let _ = child.kill();
        }
    }
    let _ = child.wait();
}

impl SubprocessRuntime {
    /// A runtime over `config`, reading job specs from `source`.
    pub fn new(config: RunnerConfig, source: Arc<dyn JobSource>) -> Self {
        Self {
            config,
            source,
            next_dir: AtomicU64::new(0),
            containment: None,
        }
    }

    /// Runs every job inside `containment`. A job that cannot be contained
    /// does not run.
    #[must_use]
    pub fn contained(mut self, containment: Arc<Containment>) -> Self {
        self.containment = Some(containment);
        self
    }

    /// Refuses a command that is not the build the configuration pins.
    pub fn verify_binary(&self) -> Result<(), String> {
        let Some(pinned) = &self.config.identity.binary_sha256 else {
            return Ok(());
        };
        let path = self.config.command.first().ok_or("empty runner command")?;
        let bytes = fs::read(path).map_err(|e| format!("runner binary: {}", e.kind()))?;
        if hex(&Sha256::digest(&bytes)) == *pinned {
            Ok(())
        } else {
            Err("runner binary does not match binary_sha256".to_owned())
        }
    }

    fn job_dir(&self, job: &ResolvedJob) -> io::Result<JobDir> {
        let n = self.next_dir.fetch_add(1, Ordering::SeqCst);
        let name = format!("job-{}-{}-{n}", std::process::id(), hex(&job.digest[..8]));
        let dir = JobDir(self.config.work_dir.join(name));
        fs::create_dir_all(dir.0.join("artifacts"))?;
        let spec = self
            .source
            .job_spec(&job.digest)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        fs::write(dir.0.join("job.json"), spec)?;
        for artifact in &job.spec.artifacts {
            if let Some(bytes) = job.artifacts.get(&artifact.role) {
                fs::write(dir.0.join("artifacts").join(&artifact.sha256), bytes)?;
            }
        }
        Ok(dir)
    }

    fn spawn(&self, dir: &Path, group: Option<&JobCgroup>) -> io::Result<Child> {
        let (program, args) = self
            .config
            .command
            .split_first()
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        let mut command = Command::new(program);
        command
            .args(args)
            .env_clear()
            .env("MBONGO_JOB_DIR", dir)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in &self.config.pass_env {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        if let Some(group) = group {
            group.confine(&mut command);
        }
        command.spawn()
    }
}

impl RuntimeAdapter for SubprocessRuntime {
    fn identity(&self) -> RuntimeIdentity {
        self.config.identity.clone()
    }

    fn run(&self, job: &ResolvedJob, input: &[u8]) -> Result<RunOutput, RuntimeFailed> {
        let started = Instant::now();
        let deadline = started + Duration::from_millis(job.spec.max_wall_ms) + KILL_GRACE;
        let dir = self.job_dir(job).map_err(|_| RuntimeFailed)?;
        // Declared after `dir`: what is left in the cgroup dies first.
        let group = match &self.containment {
            Some(c) => {
                let label = hex(&job.digest[..8]);
                Some(c.job(job.spec.max_memory_mib, &label).map_err(|_| RuntimeFailed)?)
            }
            None => None,
        };
        let group = group.as_ref();
        let mut child = self.spawn(&dir.0, group).map_err(|_| RuntimeFailed)?;

        let (Some(mut stdin), Some(stdout), Some(mut stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            kill(&mut child, group);
            return Err(RuntimeFailed);
        };
        let mut request =
            format!("{RUNNER_CONTRACT} {} {}\n", hex(&job.digest), input.len()).into_bytes();
        request.extend_from_slice(input);
        // A runner may exit without reading: a broken pipe is not a failure here.
        std::thread::spawn(move || {
            let _ = stdin.write_all(&request);
        });
        // Read and dropped: stderr may hold job content.
        std::thread::spawn(move || {
            let _ = io::copy(&mut stderr, &mut io::sink());
        });
        let max_bytes = job.spec.output.max_bytes;
        let (done, response) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(read_response(stdout, max_bytes));
        });

        let collected = loop {
            match response.try_recv() {
                Ok(r) => break Some(r),
                Err(mpsc::TryRecvError::Disconnected) => break None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if Instant::now() >= deadline {
                break None;
            }
            std::thread::sleep(POLL);
        };
        let raw = match collected {
            Some(Response::Output(bytes)) => bytes,
            Some(Response::TooLarge(bytes)) => {
                kill(&mut child, group);
                return Ok(RunOutput {
                    raw: bytes,
                    metering: UntrustedMetering::default(),
                });
            }
            Some(Response::Malformed) | None => {
                kill(&mut child, group);
                return Err(RuntimeFailed);
            }
        };
        let success = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.success(),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL),
                _ => {
                    kill(&mut child, group);
                    break false;
                }
            }
        };
        if !success {
            return Err(RuntimeFailed);
        }
        Ok(RunOutput {
            raw,
            metering: UntrustedMetering {
                wall_ms: u64::try_from(started.elapsed().as_millis()).ok(),
                cpu_ms: None,
                peak_memory_mib: None,
            },
        })
    }
}
