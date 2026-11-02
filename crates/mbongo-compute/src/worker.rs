//! The reference worker: one process, one executor key, one profile.
//!
//! An attempt follows E §7 exactly — session, lease, capability, fetch,
//! verify, start, execute, persist, receipt — and every step that touches
//! private bytes or the executor key happens here and nowhere else. The
//! control plane coordinates; the data plane stores; the chain settles.
//!
//! Recovery follows E §9.3 and §11.2: a new attempt gets a new lease and a
//! **fresh** capability, never a reopened one; a durable result from an
//! earlier attempt is reused rather than recomputed; an ambiguous
//! submission is resolved by looking the receipt up on-chain before
//! resubmitting the **same signed bytes**, and a second, different receipt
//! is never signed for the same task.
//!
//! This is ordinary execution: the process sees plaintext. It does not
//! claim the output is correct; it claims, by signature, that this
//! executor answered this task over this committed input with this output
//! commitment.

use mbongo_core::{Address, Hash, Receipt, Transaction, TransactionPayload, TransactionType};
use mbongo_verification::RECEIPT_VERSION;

use crate::chain::{find_receipt, transaction_hash, ChainClient, ChainError};
use crate::control_plane::{
    AttemptEvent, ControlPlane, ControlPlaneError, FailureClass, Lease, Session,
};
use crate::data_plane::{DataPlaneError, InMemoryDataPlane, Presentation, ResultRef};
use crate::execution::{
    reference_input_commitment, reference_output_commitment, ExecutionProfile, Plaintext,
};
use crate::identity::{ExecutorKey, IdSource, WorkerInstanceId};

/// Domain tag for session possession proofs (must match the control plane).
const DOMAIN_SESSION: &str = "mbongo:ref-session:v1";

/// Fault injection points, for tests. Each crash returns
/// [`AttemptOutcome::Crashed`] at that phase and abandons in-memory state,
/// which is what a real crash does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Lease and capability obtained; crash before presenting.
    CrashBeforeFetch,
    /// Input fetched (capability consumed); crash before verification.
    CrashAfterFetch,
    /// Execution started; crash before the result is persisted.
    CrashDuringExecution,
    /// Result persisted; crash before any receipt is built.
    CrashAfterPersist,
    /// Receipt submitted; crash before reporting or observing it.
    CrashAfterSubmit,
    /// Flip one bit of the fetched input before verification.
    CorruptInput,
}

/// What one `run_once` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptOutcome {
    /// Nothing admissible was offered.
    Idle,
    /// A receipt for the task is observed on-chain.
    Completed {
        /// The task.
        task_id: [u8; 32],
        /// The block carrying the receipt.
        height: u64,
    },
    /// The anchoring transaction was accepted by the node.
    Submitted {
        /// The task.
        task_id: [u8; 32],
        /// The hash the node reported.
        tx_hash: Hash,
    },
    /// The attempt failed and reported it.
    Failed {
        /// The task.
        task_id: [u8; 32],
        /// Why.
        class: FailureClass,
    },
    /// An injected crash.
    Crashed {
        /// The task.
        task_id: [u8; 32],
        /// Where.
        phase: &'static str,
    },
}

/// Why the worker could not proceed. Never carries private bytes.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum WorkerError {
    /// The control plane refused.
    #[error("control plane: {0}")]
    ControlPlane(#[from] ControlPlaneError),
    /// The data plane refused.
    #[error("data plane: {0}")]
    DataPlane(#[from] DataPlaneError),
    /// The chain could not be reached or refused.
    #[error("chain: {0}")]
    Chain(#[from] ChainError),
}

/// A submitted anchoring transaction, kept so a retry resubmits the same
/// bytes and never a second receipt (E §11.2).
#[derive(Debug, Clone)]
struct PendingSubmission {
    task_id: [u8; 32],
    observed_height: u64,
    tx: Transaction,
    tx_hash: Hash,
}

/// The reference worker.
pub struct Worker {
    instance_id: WorkerInstanceId,
    executor: ExecutorKey,
    profile: Box<dyn ExecutionProfile>,
    session: Option<Session>,
    fault: Option<Fault>,
    pending: Option<PendingSubmission>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Worker(instance {}, executor {})",
            self.instance_id,
            self.executor.address()
        )
    }
}

impl Worker {
    /// A worker instance holding `executor`'s key and running `profile`.
    pub fn new(
        ids: &mut IdSource,
        executor: ExecutorKey,
        profile: Box<dyn ExecutionProfile>,
    ) -> Self {
        Self {
            instance_id: WorkerInstanceId(ids.next("worker-instance")),
            executor,
            profile,
            session: None,
            fault: None,
            pending: None,
        }
    }

    /// This process's identity. Not the executor.
    pub fn instance_id(&self) -> WorkerInstanceId {
        self.instance_id
    }

    /// The executor identity this worker answers for.
    pub fn executor(&self) -> Address {
        self.executor.address()
    }

    /// Arms a fault for the next attempt.
    pub fn inject(&mut self, fault: Fault) {
        self.fault = Some(fault);
    }

    fn take_fault(&mut self, which: Fault) -> bool {
        if self.fault == Some(which) {
            self.fault = None;
            true
        } else {
            false
        }
    }

    /// Ensures an authenticated session (E §5): proves possession of the
    /// executor key over a fresh control-plane challenge.
    pub fn ensure_session(&mut self, cp: &mut ControlPlane) -> Result<Session, WorkerError> {
        if let Some(s) = &self.session {
            return Ok(s.clone());
        }
        let executor = self.executor.address();
        let challenge = cp.session_challenge(executor);
        let proof = self.executor.prove_possession(DOMAIN_SESSION, &self.instance_id.0, &challenge);
        let session = cp.open_session(
            self.instance_id,
            executor,
            self.profile.spec_tags(),
            challenge,
            &proof,
        )?;
        self.session = Some(session.clone());
        Ok(session)
    }

    /// One attempt at whatever the control plane offers.
    pub async fn run_once<C: ChainClient>(
        &mut self,
        cp: &mut ControlPlane,
        dp: &mut InMemoryDataPlane,
        chain: &C,
    ) -> Result<AttemptOutcome, WorkerError> {
        let session = match self.ensure_session(cp) {
            Ok(s) => s,
            Err(WorkerError::ControlPlane(
                ControlPlaneError::SessionExpired | ControlPlaneError::UnknownSession,
            )) => {
                self.session = None;
                self.ensure_session(cp)?
            }
            Err(e) => return Err(e),
        };
        let Some(lease) = cp.offer(session.session_id, dp)? else {
            return Ok(AttemptOutcome::Idle);
        };
        self.attempt(cp, dp, chain, &session, &lease).await
    }

    /// One attempt, as one linear function on purpose: the order of these
    /// steps is the contract (E §7), and splitting them would hide it.
    #[allow(clippy::too_many_lines)]
    async fn attempt<C: ChainClient>(
        &mut self,
        cp: &mut ControlPlane,
        dp: &mut InMemoryDataPlane,
        chain: &C,
        session: &Session,
        lease: &Lease,
    ) -> Result<AttemptOutcome, WorkerError> {
        let sid = session.session_id;
        let task_id = lease.task_id;
        let rec = cp.task(task_id).ok_or(ControlPlaneError::UnknownTask)?.clone();
        let task = rec.task.clone();
        let observed_height = rec.observed_height;
        debug_assert_eq!(
            task.executor,
            self.executor.address(),
            "the control plane never offers another executor's task"
        );

        // ── Lookup first (E §11.2): is this task already answered on-chain?
        let latest = chain.latest_height().await?;
        if let Some((height, _)) = find_receipt(chain, task_id, observed_height, latest).await? {
            cp.mark_completed(task_id, height);
            return Ok(AttemptOutcome::Completed { task_id, height });
        }

        // ── A durable result from an earlier attempt is reused, never
        //    recomputed (E §9.3): the receipt must commit to *that* result.
        let result: ResultRef = if let Some(existing) =
            dp.result_ref(task_id, self.executor.address())
        {
            log::info!(
                "worker: task {} has a durable result from an earlier attempt; skipping to receipt",
                hex::encode(&task_id[..8])
            );
            existing
        } else {
            // ── Lease → capability → fetch → verify → start → execute.
            let cap = cp.authorize_fetch(sid, lease.lease_id, dp)?;
            if self.take_fault(Fault::CrashBeforeFetch) {
                return Ok(AttemptOutcome::Crashed {
                    task_id,
                    phase: "before-fetch",
                });
            }
            let challenge = dp.issue_challenge(self.executor.address());
            let presentation = Presentation::sign(cap, challenge, &self.executor);
            let input = dp.fetch_input(&presentation)?;
            if self.take_fault(Fault::CrashAfterFetch) {
                drop(input);
                return Ok(AttemptOutcome::Crashed {
                    task_id,
                    phase: "after-fetch",
                });
            }
            let input = if self.take_fault(Fault::CorruptInput) {
                let mut bytes = input.as_bytes().to_vec();
                if let Some(b) = bytes.first_mut() {
                    *b ^= 0x01;
                }
                Plaintext::new(bytes)
            } else {
                input
            };
            // The worker verifies the commitment before anything else
            // (F §7); on mismatch: no execution, no result, no receipt.
            if reference_input_commitment(input.as_bytes()) != task.input_commitment {
                log::warn!("worker: input for task {} does not match input_commitment; refusing to execute", hex::encode(&task_id[..8]));
                cp.report(
                    sid,
                    lease.lease_id,
                    AttemptEvent::Failed(FailureClass::Input),
                    dp,
                )?;
                return Ok(AttemptOutcome::Failed {
                    task_id,
                    class: FailureClass::Input,
                });
            }
            cp.report(sid, lease.lease_id, AttemptEvent::InputConsumed, dp)?;
            if !self.profile.accepts(&task.execution_spec) {
                cp.report(
                    sid,
                    lease.lease_id,
                    AttemptEvent::Failed(FailureClass::Execution),
                    dp,
                )?;
                return Ok(AttemptOutcome::Failed {
                    task_id,
                    class: FailureClass::Execution,
                });
            }
            cp.report(sid, lease.lease_id, AttemptEvent::Started, dp)?;
            let output = match self.profile.execute_task(
                &task.execution_spec,
                &task.input_commitment,
                input.as_bytes(),
            ) {
                Ok(o) => Plaintext::new(o),
                Err(e) => {
                    log::warn!(
                        "worker: execution failed for task {}: {e}",
                        hex::encode(&task_id[..8])
                    );
                    cp.report(
                        sid,
                        lease.lease_id,
                        AttemptEvent::Failed(FailureClass::Execution),
                        dp,
                    )?;
                    return Ok(AttemptOutcome::Failed {
                        task_id,
                        class: FailureClass::Execution,
                    });
                }
            };
            drop(input);
            if self.take_fault(Fault::CrashDuringExecution) {
                return Ok(AttemptOutcome::Crashed {
                    task_id,
                    phase: "executing",
                });
            }
            let output_commitment = reference_output_commitment(output.as_bytes());

            // ── Persist before any receipt exists (F §10, E §11).
            let put = cp.authorize_put(sid, lease.lease_id, dp)?;
            let challenge = dp.issue_challenge(self.executor.address());
            let presentation = Presentation::sign(put, challenge, &self.executor);
            match dp.put_result(&presentation, output.as_bytes().to_vec(), output_commitment) {
                Ok(r) => r,
                Err(DataPlaneError::ResultAlreadyExists(existing)) => {
                    // A concurrent attempt persisted first (E §10): the
                    // durable result wins; no second result, no second
                    // commitment.
                    log::info!(
                        "worker: a result already exists for task {}; using it",
                        hex::encode(&task_id[..8])
                    );
                    *existing
                }
                Err(e) => {
                    log::warn!("worker: result persistence failed for task {}: {e}; no receipt will be produced", hex::encode(&task_id[..8]));
                    cp.report(
                        sid,
                        lease.lease_id,
                        AttemptEvent::Failed(FailureClass::Persistence),
                        dp,
                    )?;
                    return Ok(AttemptOutcome::Failed {
                        task_id,
                        class: FailureClass::Persistence,
                    });
                }
            }
        };
        if self.take_fault(Fault::CrashAfterPersist) {
            return Ok(AttemptOutcome::Crashed {
                task_id,
                phase: "after-persist",
            });
        }
        cp.report(
            sid,
            lease.lease_id,
            AttemptEvent::ResultReady {
                output_commitment: result.output_commitment,
            },
            dp,
        )?;

        // ── The bound receipt, signed by the executor and anchored by it.
        let receipt = self.bound_receipt(&task, result.output_commitment);
        let nonce = chain.account_nonce(&self.executor.address()).await?;
        let tx = self.anchor_transaction(receipt, nonce);
        let tx_hash = transaction_hash(&tx);
        self.pending = Some(PendingSubmission {
            task_id,
            observed_height,
            tx: tx.clone(),
            tx_hash,
        });
        match chain.submit_transaction(&tx).await {
            Ok(hash) => {
                debug_assert_eq!(hash, tx_hash);
                if self.take_fault(Fault::CrashAfterSubmit) {
                    return Ok(AttemptOutcome::Crashed {
                        task_id,
                        phase: "after-submit",
                    });
                }
                cp.report(
                    sid,
                    lease.lease_id,
                    AttemptEvent::ReceiptSubmitted { tx_hash },
                    dp,
                )?;
                Ok(AttemptOutcome::Submitted { task_id, tx_hash })
            }
            Err(ChainError::Transport(_)) => {
                // Ambiguous: look up, then resubmit the same bytes.
                log::warn!(
                    "worker: submission response lost for task {}; looking up before resubmitting",
                    hex::encode(&task_id[..8])
                );
                self.resolve_pending(cp, dp, chain, session, lease).await
            }
            Err(ChainError::Rejected(msg)) if msg.contains("already anchored") => {
                let latest = chain.latest_height().await?;
                match find_receipt(chain, task_id, observed_height, latest).await? {
                    Some((height, _)) => {
                        cp.mark_completed(task_id, height);
                        Ok(AttemptOutcome::Completed { task_id, height })
                    }
                    None => Ok(AttemptOutcome::Submitted { task_id, tx_hash }),
                }
            }
            Err(ChainError::Rejected(msg)) => {
                log::warn!(
                    "worker: anchoring rejected for task {}: {msg}",
                    hex::encode(&task_id[..8])
                );
                cp.report(
                    sid,
                    lease.lease_id,
                    AttemptEvent::Failed(FailureClass::Receipt),
                    dp,
                )?;
                Ok(AttemptOutcome::Failed {
                    task_id,
                    class: FailureClass::Receipt,
                })
            }
        }
    }

    /// Lookup-first recovery of a pending submission (E §11.2).
    pub async fn resolve_pending<C: ChainClient>(
        &mut self,
        cp: &mut ControlPlane,
        dp: &mut InMemoryDataPlane,
        chain: &C,
        session: &Session,
        lease: &Lease,
    ) -> Result<AttemptOutcome, WorkerError> {
        let Some(pending) = self.pending.clone() else {
            return Ok(AttemptOutcome::Idle);
        };
        let latest = chain.latest_height().await?;
        if let Some((height, _)) =
            find_receipt(chain, pending.task_id, pending.observed_height, latest).await?
        {
            cp.mark_completed(pending.task_id, height);
            self.pending = None;
            return Ok(AttemptOutcome::Completed {
                task_id: pending.task_id,
                height,
            });
        }
        // Not in a block: resubmit the identical signed bytes.
        match chain.submit_transaction(&pending.tx).await {
            Ok(hash) => {
                cp.report(
                    session.session_id,
                    lease.lease_id,
                    AttemptEvent::ReceiptSubmitted { tx_hash: hash },
                    dp,
                )?;
                Ok(AttemptOutcome::Submitted {
                    task_id: pending.task_id,
                    tx_hash: hash,
                })
            }
            Err(ChainError::Rejected(msg))
                if msg.contains("already anchored") || msg.contains("already pending") =>
            {
                Ok(AttemptOutcome::Submitted {
                    task_id: pending.task_id,
                    tx_hash: pending.tx_hash,
                })
            }
            Err(e) => Err(e.into()),
        }
    }

    /// The receipt that answers `task`, bound as RFC 0005 rules (q)–(s)
    /// require: `task_id` derived from the task, `input_commitment` copied
    /// from it, `executor` the one it named — this worker's own key, which
    /// signs the receipt hash. Deterministic: the same task and output
    /// commitment always yield the same bytes and the same signature.
    pub fn bound_receipt(
        &self,
        task: &mbongo_core::ComputeTask,
        output_commitment: [u8; 32],
    ) -> Receipt {
        let mut receipt = Receipt {
            version: RECEIPT_VERSION,
            task_id: task.task_id(),
            input_commitment: task.input_commitment,
            output_commitment,
            executor: self.executor.address(),
            metadata: Vec::new(),
            signature: [0u8; 64],
        };
        receipt.signature = self.executor.sign(&receipt.receipt_hash().0);
        receipt
    }

    /// The anchoring transaction: sender is the executor (RFC 0002 rule g),
    /// signed by the executor over the raw signing payload.
    pub fn anchor_transaction(&self, receipt: Receipt, nonce: u64) -> Transaction {
        let mut tx = Transaction {
            tx_type: TransactionType::AnchorReceipt,
            sender: self.executor.address(),
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
            signature: [0u8; 64],
        };
        tx.signature = self.executor.sign(&tx.signing_payload());
        tx
    }
}
