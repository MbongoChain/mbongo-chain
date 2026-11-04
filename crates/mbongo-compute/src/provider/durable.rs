//! Durable provider state: what survives a process restart.
//!
//! Every transition is one [`Record`], written to the [`Journal`] and flushed
//! **before** it takes effect in memory or is acknowledged. A record that
//! cannot be written refuses the transition: this store fails closed.
//!
//! **Identity.** Executions are recorded per `task_id`, which covers all six
//! fields of the task, salt included. A stored output serves that task and no
//! other: two tasks with the same job and input never share one.
//!
//! **Execution guarantee.** At most one receipt per task, because the chain
//! refuses a second and a stored output is reused rather than recomputed.
//! A runner may still run more than once: a crash between its start and its
//! recorded output cannot tell whether it finished. **Exactly-once execution
//! of an external runtime is not claimed.**
//!
//! **Retention.** Replay entries live until their window ends. Inputs are
//! dropped when a task reaches a terminal state, or at `input_ttl_secs` if
//! never registered. Terminal tasks and their results are dropped after
//! `result_retention_secs`. `max_tasks` and `max_state_bytes` bound the rest:
//! a new upload is refused, never silently evicted.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Mutex;

use mbongo_core::Address;
use serde::{Deserialize, Serialize};

use crate::control_plane::DurableState;
use crate::mvp::client::random_bytes;

use super::auth::MAX_REPLAY_ENTRIES;
use super::journal::{Journal, JournalError};
use super::lock::StateLock;

/// How long state is kept, and how much of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// An unregistered upload is dropped after this long.
    pub input_ttl_secs: u64,
    /// A terminal task and its result are dropped after this long.
    pub result_retention_secs: u64,
    /// Most tasks and uploads held at once.
    pub max_tasks: usize,
    /// Most bytes of inputs and outputs held at once.
    pub max_state_bytes: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            input_ttl_secs: 3_600,
            result_retention_secs: 86_400,
            max_tasks: 4_096,
            max_state_bytes: 256 * 1024 * 1024,
        }
    }
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        hex::decode(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// A task's outcome, once it has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terminal {
    /// A receipt is on-chain. `None`: its output was not held.
    Completed {
        /// Height of the receipt.
        receipt_height: u64,
        /// The output, if held.
        output: Option<HexBytes>,
    },
    /// The service stopped offering it.
    Parked {
        /// The failure code.
        code: String,
        /// Attempts issued.
        attempts: u32,
    },
}

/// Bytes, as hex in the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HexBytes(#[serde(with = "hex_bytes")] pub Vec<u8>);

/// A registered task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEntry {
    /// The input, until a terminal state.
    pub input: Option<HexBytes>,
    /// The input commitment.
    pub input_commitment: [u8; 32],
    /// The reference the client registered with.
    pub object_ref: [u8; 32],
    /// Its outcome.
    pub terminal: Option<Terminal>,
    /// When it became terminal.
    pub terminal_at: Option<u64>,
}

/// An upload awaiting registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadEntry {
    /// The uploader.
    pub caller: Address,
    /// The reference handed to the client.
    pub object_ref: [u8; 32],
    /// The input commitment.
    pub input_commitment: [u8; 32],
    /// The input.
    pub input: HexBytes,
    /// When it was uploaded.
    pub uploaded_at: u64,
}

/// One transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum Record {
    Accepted {
        signer: Address,
        request_id: [u8; 16],
        expires_at: u64,
    },
    JobSpec {
        bytes: HexBytes,
    },
    Upload {
        task_id: [u8; 32],
        upload: UploadEntry,
    },
    Registered {
        task_id: [u8; 32],
        entry: TaskEntry,
    },
    Started {
        task_id: [u8; 32],
    },
    Output {
        task_id: [u8; 32],
        output: HexBytes,
    },
    AttemptFailed {
        task_id: [u8; 32],
        code: String,
    },
    Terminal {
        task_id: [u8; 32],
        terminal: Terminal,
        at: u64,
    },
    ControlPlane {
        state: DurableState,
    },
    Forget {
        task_id: [u8; 32],
    },
}

/// The durable state, in memory.
#[derive(Debug, Default)]
pub struct State {
    /// Accepted requests and the end of their window.
    pub replay: HashMap<(Address, [u8; 16]), u64>,
    /// Job specs.
    pub specs: Vec<Vec<u8>>,
    /// Uploads awaiting registration, per uploader: one client cannot
    /// replace another's.
    pub uploads: HashMap<([u8; 32], Address), UploadEntry>,
    /// Registered tasks.
    pub tasks: BTreeMap<[u8; 32], TaskEntry>,
    /// Outputs produced for a task that has no outcome yet, by `task_id`.
    pub outputs: HashMap<[u8; 32], Vec<u8>>,
    /// Attempts started with no recorded end, by `task_id`.
    pub started: HashMap<[u8; 32], u32>,
    /// The last failure code, by `task_id`.
    pub last_failure: HashMap<[u8; 32], String>,
    /// The last control-plane snapshot.
    pub control_plane: Option<DurableState>,
}

impl State {
    fn apply(&mut self, record: Record) {
        match record {
            Record::Accepted {
                signer,
                request_id,
                expires_at,
            } => {
                self.replay.insert((signer, request_id), expires_at);
            }
            Record::JobSpec { bytes } => {
                if !self.specs.contains(&bytes.0) {
                    self.specs.push(bytes.0);
                }
            }
            Record::Upload { task_id, upload } => {
                self.uploads.insert((task_id, upload.caller), upload);
            }
            Record::Registered { task_id, entry } => {
                self.uploads.retain(|(id, _), _| *id != task_id);
                self.tasks.insert(task_id, entry);
            }
            Record::Started { task_id } => {
                *self.started.entry(task_id).or_insert(0) += 1;
            }
            Record::Output { task_id, output } => {
                self.started.remove(&task_id);
                self.last_failure.remove(&task_id);
                self.outputs.insert(task_id, output.0);
            }
            Record::AttemptFailed { task_id, code } => {
                self.started.remove(&task_id);
                self.last_failure.insert(task_id, code);
            }
            Record::Terminal {
                task_id,
                terminal,
                at,
            } => {
                if let Some(task) = self.tasks.get_mut(&task_id) {
                    task.input = None;
                    task.terminal = Some(terminal);
                    task.terminal_at = Some(at);
                }
                self.outputs.remove(&task_id);
                self.started.remove(&task_id);
                self.last_failure.remove(&task_id);
            }
            Record::ControlPlane { state } => self.control_plane = Some(state),
            Record::Forget { task_id } => {
                self.uploads.retain(|(id, _), _| *id != task_id);
                self.tasks.remove(&task_id);
            }
        }
    }

    fn held_bytes(&self) -> usize {
        let inputs: usize =
            self.tasks.values().filter_map(|t| t.input.as_ref().map(|i| i.0.len())).sum();
        let results: usize = self
            .tasks
            .values()
            .filter_map(|t| match &t.terminal {
                Some(Terminal::Completed {
                    output: Some(o), ..
                }) => Some(o.0.len()),
                _ => None,
            })
            .sum();
        let uploads: usize = self.uploads.values().map(|u| u.input.0.len()).sum();
        let outputs: usize = self.outputs.values().map(Vec::len).sum();
        inputs + results + uploads + outputs
    }

    /// The whole state as records, for a rewrite.
    fn records(&self) -> Vec<Record> {
        let mut out = Vec::new();
        for ((signer, request_id), expires_at) in &self.replay {
            out.push(Record::Accepted {
                signer: *signer,
                request_id: *request_id,
                expires_at: *expires_at,
            });
        }
        for bytes in &self.specs {
            out.push(Record::JobSpec {
                bytes: HexBytes(bytes.clone()),
            });
        }
        for ((task_id, _), upload) in &self.uploads {
            out.push(Record::Upload {
                task_id: *task_id,
                upload: upload.clone(),
            });
        }
        for (task_id, entry) in &self.tasks {
            let mut open = entry.clone();
            let terminal = open.terminal.take();
            let at = open.terminal_at.take();
            out.push(Record::Registered {
                task_id: *task_id,
                entry: open,
            });
            if let (Some(terminal), Some(at)) = (terminal, at) {
                out.push(Record::Terminal {
                    task_id: *task_id,
                    terminal,
                    at,
                });
            }
        }
        for (task_id, output) in &self.outputs {
            out.push(Record::Output {
                task_id: *task_id,
                output: HexBytes(output.clone()),
            });
        }
        // In this order: replaying a failure clears a start, and a retry that
        // crashed has both.
        for (task_id, code) in &self.last_failure {
            out.push(Record::AttemptFailed {
                task_id: *task_id,
                code: code.clone(),
            });
        }
        for (task_id, count) in &self.started {
            for _ in 0..*count {
                out.push(Record::Started { task_id: *task_id });
            }
        }
        if let Some(state) = &self.control_plane {
            out.push(Record::ControlPlane {
                state: state.clone(),
            });
        }
        out
    }
}

/// Why a request could not be accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcceptError {
    /// This identifier was already accepted.
    #[error("AUTH_REPLAYED")]
    Replayed,
    /// The replay store is full.
    #[error("AUTH_REPLAY_STORE_FULL")]
    Full,
    /// The acceptance could not be made durable.
    #[error("{0}")]
    State(JournalError),
}

/// Why an upload could not be held.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// A retention limit is reached.
    #[error("STORE_FULL")]
    Full,
    /// The transition could not be made durable.
    #[error("{0}")]
    State(JournalError),
}

impl From<JournalError> for StoreError {
    fn from(e: JournalError) -> Self {
        Self::State(e)
    }
}

struct Inner {
    journal: Journal,
    state: State,
    compacted_at: u64,
}

/// The durable store. Its methods are the provider's only state transitions.
pub struct DurableStore {
    inner: Mutex<Inner>,
    retention: RetentionPolicy,
    _lock: StateLock,
}

const JOURNAL_FILE: &str = "provider-state.journal";
/// A rewrite is considered once the journal grew this much past its last size.
const COMPACT_GROWTH_BYTES: u64 = 4 * 1024 * 1024;

fn encode(record: &Record) -> Result<Vec<u8>, JournalError> {
    serde_json::to_vec(record).map_err(|_| JournalError::Corrupt("encoding"))
}

impl Inner {
    fn commit(&mut self, record: Record) -> Result<(), JournalError> {
        self.journal.append(&encode(&record)?, random_bytes::<12>())?;
        self.state.apply(record);
        Ok(())
    }
}

impl DurableStore {
    /// Opens the store in `dir`, taking exclusive ownership first. **Fails
    /// closed:** a directory another process owns, a journal that does not
    /// verify, or a record that does not parse is an error, and the service
    /// must not start. A refused lock changes nothing in the directory.
    pub fn open(
        dir: &Path,
        key: &[u8; 32],
        retention: RetentionPolicy,
    ) -> Result<Self, JournalError> {
        std::fs::create_dir_all(dir).map_err(|e| JournalError::Io(format!("{:?}", e.kind())))?;
        let lock = StateLock::acquire(dir)?;
        let (journal, records) = Journal::open(&dir.join(JOURNAL_FILE), key)?;
        let mut state = State::default();
        for bytes in records {
            let record: Record =
                serde_json::from_slice(&bytes).map_err(|_| JournalError::Corrupt("record body"))?;
            state.apply(record);
        }
        let compacted_at = journal.bytes();
        Ok(Self {
            inner: Mutex::new(Inner {
                journal,
                state,
                compacted_at,
            }),
            retention,
            _lock: lock,
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, JournalError> {
        self.inner.lock().map_err(|_| JournalError::Unavailable)
    }

    /// Reads the state.
    pub fn read<T>(&self, f: impl FnOnce(&State) -> T) -> Result<T, JournalError> {
        Ok(f(&self.lock()?.state))
    }

    /// Records an accepted request, durably, before it is acted on.
    pub fn accept_request(
        &self,
        signer: Address,
        request_id: [u8; 16],
        expires_at: u64,
        now: u64,
    ) -> Result<(), AcceptError> {
        let mut inner = self.lock().map_err(AcceptError::State)?;
        inner.state.replay.retain(|_, end| *end >= now);
        if inner.state.replay.contains_key(&(signer, request_id)) {
            return Err(AcceptError::Replayed);
        }
        if inner.state.replay.len() >= MAX_REPLAY_ENTRIES {
            return Err(AcceptError::Full);
        }
        inner
            .commit(Record::Accepted {
                signer,
                request_id,
                expires_at,
            })
            .map_err(AcceptError::State)
    }

    /// Records a job spec.
    pub fn add_spec(&self, bytes: &[u8]) -> Result<(), JournalError> {
        self.lock()?.commit(Record::JobSpec {
            bytes: HexBytes(bytes.to_vec()),
        })
    }

    /// Records an upload, unless a retention limit is reached.
    pub fn add_upload(&self, task_id: [u8; 32], upload: UploadEntry) -> Result<(), StoreError> {
        let mut inner = self.lock()?;
        let tasks = inner.state.tasks.len() + inner.state.uploads.len();
        let bytes = inner.state.held_bytes() + upload.input.0.len();
        if tasks >= self.retention.max_tasks || bytes > self.retention.max_state_bytes {
            return Err(StoreError::Full);
        }
        Ok(inner.commit(Record::Upload { task_id, upload })?)
    }

    /// Records a registration.
    pub fn register(&self, task_id: [u8; 32], entry: TaskEntry) -> Result<(), JournalError> {
        self.lock()?.commit(Record::Registered { task_id, entry })
    }

    /// Records that an execution is about to start.
    pub fn started(&self, task_id: [u8; 32]) -> Result<(), JournalError> {
        self.lock()?.commit(Record::Started { task_id })
    }

    /// Records an output, before any receipt can exist for it.
    pub fn output(&self, task_id: [u8; 32], output: &[u8]) -> Result<(), JournalError> {
        self.lock()?.commit(Record::Output {
            task_id,
            output: HexBytes(output.to_vec()),
        })
    }

    /// Records a failed attempt.
    pub fn attempt_failed(&self, task_id: [u8; 32], code: &str) -> Result<(), JournalError> {
        self.lock()?.commit(Record::AttemptFailed {
            task_id,
            code: code.to_owned(),
        })
    }

    /// Records a task's outcome.
    pub fn terminal(
        &self,
        task_id: [u8; 32],
        terminal: Terminal,
        at: u64,
    ) -> Result<(), JournalError> {
        self.lock()?.commit(Record::Terminal {
            task_id,
            terminal,
            at,
        })
    }

    /// Records the control plane's durable state (E §16).
    pub fn control_plane(&self, state: DurableState) -> Result<(), JournalError> {
        self.lock()?.commit(Record::ControlPlane { state })
    }

    /// Applies retention, and rewrites the journal if it has grown.
    pub fn maintain(&self, now: u64) -> Result<(), JournalError> {
        let mut inner = self.lock()?;
        let input_ttl = self.retention.input_ttl_secs;
        let keep = self.retention.result_retention_secs;
        let mut expired: Vec<[u8; 32]> = inner
            .state
            .uploads
            .iter()
            .filter(|(_, u)| u.uploaded_at.saturating_add(input_ttl) < now)
            .map(|((id, _), _)| *id)
            .collect();
        expired.extend(
            inner
                .state
                .tasks
                .iter()
                .filter(|(_, t)| t.terminal_at.is_some_and(|at| at.saturating_add(keep) < now))
                .map(|(id, _)| *id),
        );
        for task_id in expired {
            inner.commit(Record::Forget { task_id })?;
        }
        if inner.journal.bytes() > inner.compacted_at + COMPACT_GROWTH_BYTES {
            inner.state.replay.retain(|_, end| *end >= now);
            let records =
                inner.state.records().iter().map(encode).collect::<Result<Vec<_>, _>>()?;
            inner.journal.rewrite(&records, random_bytes::<12>)?;
            inner.compacted_at = inner.journal.bytes();
        }
        Ok(())
    }

    /// Rewrites the journal now.
    #[doc(hidden)]
    pub fn compact(&self) -> Result<(), JournalError> {
        let mut inner = self.lock()?;
        let records = inner.state.records().iter().map(encode).collect::<Result<Vec<_>, _>>()?;
        inner.journal.rewrite(&records, random_bytes::<12>)?;
        inner.compacted_at = inner.journal.bytes();
        Ok(())
    }

    /// Makes every later write fail, as a failed device would.
    #[doc(hidden)]
    pub fn poison(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.journal.poison();
        }
    }
}
