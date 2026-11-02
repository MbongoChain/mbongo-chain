//! The reference lifecycle, end to end and under every fault the contracts
//! name, against an in-memory chain double.
//!
//! The chain double records what it is given and validates nothing: rules
//! (k)–(s) are the node's and are proven in `mbongo-node` and by the live
//! harness. What these tests prove is everything *around* consensus — the
//! boundaries of E and F — and that the worker never asks the chain for
//! something the chain would refuse.

use std::sync::Arc;

use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::chain::{find_receipt, transaction_hash, ChainClient};
use mbongo_compute::clock::{Clock, ManualClock};
use mbongo_compute::control_plane::{
    AttemptEvent, ControlPlane, ControlPlaneConfig, ControlPlaneError, FailureClass, TaskState,
};
use mbongo_compute::data_plane::{
    CapabilityRequest, DataPlaneError, DataPlaneFault, InMemoryDataPlane, LocalKey, ObjectState,
    Operation, Presentation,
};
use mbongo_compute::execution::{
    reference_input_commitment, reference_output_commitment, ReverseBytesProfile,
    REVERSE_BYTES_SPEC,
};
use mbongo_compute::identity::{ExecutorKey, IdSource, ObjectId};
use mbongo_compute::worker::{AttemptOutcome, Fault, Worker};
use mbongo_core::{
    Address, ComputeTask, Transaction, TransactionPayload, TransactionType, COMPUTE_TASK_VERSION,
};
use mbongo_verification::verify_receipt_signature;

/// Everything one scenario needs, wired the way the harness wires it.
struct World {
    clock: ManualClock,
    ids: IdSource,
    chain: FakeChain,
    cp: ControlPlane,
    dp: InMemoryDataPlane,
    client: LocalKey,
    executor: ExecutorKey,
    input: Vec<u8>,
    task: ComputeTask,
    task_id: [u8; 32],
    input_object: ObjectId,
}

const LEASE_SECS: u64 = 60;

impl World {
    fn new() -> Self {
        let clock = ManualClock::starting_at(1_000);
        let clock_arc: Arc<dyn Clock> = Arc::new(clock.clone());
        let mut ids = IdSource::new([0x11u8; 32]);
        let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
        let cp = ControlPlane::new(
            Arc::clone(&clock_arc),
            IdSource::new([0x22u8; 32]),
            issuer.clone(),
            ControlPlaneConfig {
                lease_secs: LEASE_SECS,
                session_secs: 600,
                confirmation_depth: 1,
                capability_secs: 120,
            },
        );
        let mut dp = InMemoryDataPlane::new(Arc::clone(&clock_arc), IdSource::new([0x33u8; 32]));
        let client = LocalKey::from_seed(&[0xAAu8; 32]);
        let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
        let input = b"private input that never touches the chain".to_vec();
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: client.address(),
            executor: executor.address(),
            salt: [0x5Au8; 32],
            input_commitment: reference_input_commitment(&input),
            execution_spec: REVERSE_BYTES_SPEC.to_vec(),
        };
        let task_id = task.task_id();
        let input_object = dp.store_input(
            &client,
            task_id,
            task.input_commitment,
            input.clone(),
            3_600,
        );
        dp.register_task(&client, task_id, executor.address()).unwrap();
        dp.delegate_issuer(&client, task_id, issuer.address()).unwrap();
        let chain = FakeChain::new();
        Self {
            clock,
            ids: {
                ids.next("warm");
                ids
            },
            chain,
            cp,
            dp,
            client,
            executor,
            input,
            task,
            task_id,
            input_object,
        }
    }

    fn task_tx(&self, nonce: u64) -> Transaction {
        let mut tx = Transaction {
            tx_type: TransactionType::ComputeTask,
            sender: self.client.address(),
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::ComputeTask(Box::new(self.task.clone())),
            signature: [0u8; 64],
        };
        tx.signature = self.client.sign(&tx.signing_payload());
        tx
    }

    /// Commits the task in a block, produces one confirmation, and lets the
    /// control plane observe both.
    async fn commit_task(&mut self) {
        self.chain.submit_transaction(&self.task_tx(0)).await.unwrap();
        self.chain.produce_block();
        self.chain.produce_block();
        self.cp.observe(&self.chain).await.unwrap();
        self.cp.register_input(self.task_id, self.input_object).unwrap();
    }

    fn worker(&mut self) -> Worker {
        Worker::new(
            &mut self.ids,
            self.executor.clone(),
            Box::new(ReverseBytesProfile),
        )
    }

    async fn run(&mut self, w: &mut Worker) -> AttemptOutcome {
        w.run_once(&mut self.cp, &mut self.dp, &self.chain).await.unwrap()
    }
}

// ── the full flow ───────────────────────────────────────────────────────

#[tokio::test]
async fn full_private_input_to_receipt_flow() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();

    // Nothing is offered before the confirmation depth is met.
    let mut early = World::new();
    early.chain.submit_transaction(&early.task_tx(0)).await.unwrap();
    early.chain.produce_block();
    early.cp.observe(&early.chain).await.unwrap();
    early.cp.register_input(early.task_id, early.input_object).unwrap();
    let mut ew = early.worker();
    assert_eq!(
        early.run(&mut ew).await,
        AttemptOutcome::Idle,
        "unconfirmed task is not offered"
    );

    // 5–15: lease, capability, fetch, verify, execute, persist, receipt.
    let outcome = w.run(&mut worker).await;
    let AttemptOutcome::Submitted { task_id, tx_hash } = outcome else {
        panic!("expected a submitted receipt, got {outcome:?}");
    };
    assert_eq!(task_id, w.task_id);
    assert_eq!(
        w.cp.task(w.task_id).unwrap().state,
        TaskState::ReceiptSubmitted
    );
    assert_eq!(
        w.dp.object_state(&w.input_object),
        Some(ObjectState::Consumed)
    );

    // The result was durable before the receipt existed.
    let result = w.dp.result_ref(w.task_id, w.executor.address()).expect("result persisted");
    let pending = w.chain.pending();
    assert_eq!(pending.len(), 1);
    assert_eq!(transaction_hash(&pending[0]), tx_hash);
    let TransactionPayload::AnchorReceipt(receipt) = &pending[0].payload else {
        panic!("expected an anchor");
    };
    // Bound as RFC 0005 (q)–(s) require, signed and sent by the executor.
    assert_eq!(receipt.task_id, w.task_id);
    assert_eq!(receipt.input_commitment, w.task.input_commitment);
    assert_eq!(receipt.executor, w.executor.address());
    assert_eq!(receipt.output_commitment, result.output_commitment);
    assert_eq!(pending[0].sender, w.executor.address());
    assert!(verify_receipt_signature(receipt));
    assert!(pending[0].verify_signature());
    assert!(
        receipt.metadata.is_empty(),
        "no result bytes ride in metadata"
    );

    // 16–17: block includes the anchor; the worker and the control plane
    // both derive completion from the chain.
    let height = w.chain.produce_block();
    assert_eq!(
        w.run(&mut worker).await,
        AttemptOutcome::Completed {
            task_id: w.task_id,
            height
        }
    );
    w.cp.observe(&w.chain).await.unwrap();
    assert_eq!(w.cp.task(w.task_id).unwrap().state, TaskState::Completed);
    assert_eq!(
        w.run(&mut worker).await,
        AttemptOutcome::Idle,
        "a completed task is never re-attempted"
    );

    // 18: the client retrieves the private result under its own grant.
    let get =
        w.dp.issue_capability(
            &w.client,
            &CapabilityRequest {
                task_id: w.task_id,
                operation: Operation::GetResult,
                resource: Some(result.object),
                ttl_secs: 60,
                max_uses: 1,
            },
        )
        .unwrap();
    let ch = w.dp.issue_challenge(w.client.address());
    let out = w.dp.get_result(&Presentation::sign(get, ch, &w.client)).unwrap();
    let mut expected = w.input.clone();
    expected.reverse();
    assert_eq!(out.as_bytes(), expected.as_slice());
    assert_eq!(
        reference_output_commitment(out.as_bytes()),
        receipt.output_commitment
    );

    // No private byte is on the chain: the only bytes committed are the
    // task's public fields and the receipt's commitments.
    for h in 0..=height {
        let block = w.chain.block_by_height(h).await.unwrap().unwrap();
        let json = serde_json::to_string(&block).unwrap();
        let input_json = serde_json::to_string(&w.input).unwrap();
        assert!(!json.contains(&input_json[1..input_json.len() - 1]));
        assert!(!json.contains(&hex::encode(&w.input)));
    }
}

// ── executor binding ────────────────────────────────────────────────────

#[tokio::test]
async fn wrong_executor_is_never_offered_the_task_and_cannot_fetch() {
    let mut w = World::new();
    w.commit_task().await;
    let other = ExecutorKey::from_seed(&[0xE9u8; 32]);
    let mut squatter = Worker::new(&mut w.ids, other.clone(), Box::new(ReverseBytesProfile));
    assert_eq!(w.run(&mut squatter).await, AttemptOutcome::Idle);
    assert_eq!(w.cp.task(w.task_id).unwrap().lease, None);

    // Even a capability issued to the named executor is useless to another
    // key: proof of possession fails.
    let session = {
        let mut real = w.worker();
        real.ensure_session(&mut w.cp).unwrap()
    };
    let lease = w.cp.offer(session.session_id, &mut w.dp).unwrap().unwrap();
    let cap = w.cp.authorize_fetch(session.session_id, lease.lease_id, &mut w.dp).unwrap();
    let ch = w.dp.issue_challenge(other.address());
    let stolen = Presentation::sign(cap.clone(), ch, &other);
    assert_eq!(
        w.dp.fetch_input(&stolen).unwrap_err(),
        DataPlaneError::BadChallenge
    );
    // With a challenge issued to the right presenter but the wrong key:
    let ch = w.dp.issue_challenge(w.executor.address());
    let forged = Presentation::sign(cap, ch, &other);
    assert_eq!(
        w.dp.fetch_input(&forged).unwrap_err(),
        DataPlaneError::BadPossessionProof
    );
    assert_eq!(
        w.dp.object_state(&w.input_object),
        Some(ObjectState::Authorized),
        "nothing was consumed"
    );
}

#[tokio::test]
async fn control_plane_cannot_change_the_executor_or_sign_for_it() {
    let mut w = World::new();
    w.commit_task().await;
    // The control plane has no executor key and no field to change; a
    // squatter session gets nothing, and the data plane checks the
    // client-registered executor independently of anything the control
    // plane says: a capability it issues for a task names the registered
    // executor, never the session's claim.
    let other = ExecutorKey::from_seed(&[0xE9u8; 32]);
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
    let cap =
        w.dp.issue_capability(
            &issuer,
            &CapabilityRequest {
                task_id: w.task_id,
                operation: Operation::FetchInput,
                resource: Some(w.input_object),
                ttl_secs: 60,
                max_uses: 1,
            },
        )
        .unwrap();
    assert_eq!(
        cap.presenter,
        w.executor.address(),
        "the grant names the chain-committed executor"
    );
    // Re-registering the task with another executor is refused.
    assert_eq!(
        w.dp.register_task(&w.client, w.task_id, other.address()).unwrap_err(),
        DataPlaneError::ExecutorImmutable
    );
    // A receipt signed by the issuer key is not the executor's receipt.
    let mut fake = w.worker().bound_receipt(&w.task, [0u8; 32]);
    fake.signature = issuer.sign(&fake.receipt_hash().0);
    assert!(!verify_receipt_signature(&fake));
}

#[tokio::test]
async fn task_id_alone_grants_nothing() {
    let mut w = World::new();
    w.commit_task().await;
    // There is no API that takes a bare task_id for content. The closest a
    // caller can get is a capability without a valid issuer signature.
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
    let mut cap =
        w.dp.issue_capability(
            &issuer,
            &CapabilityRequest {
                task_id: w.task_id,
                operation: Operation::FetchInput,
                resource: Some(w.input_object),
                ttl_secs: 60,
                max_uses: 1,
            },
        )
        .unwrap();
    cap.issuer_signature = [0u8; 64];
    let ch = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.fetch_input(&Presentation::sign(cap, ch, &w.executor)).unwrap_err(),
        DataPlaneError::BadIssuerSignature
    );
    // And an issuer the owner never delegated is refused.
    let stranger = LocalKey::from_seed(&[0xDDu8; 32]);
    assert_eq!(
        w.dp.issue_capability(
            &stranger,
            &CapabilityRequest {
                task_id: w.task_id,
                operation: Operation::FetchInput,
                resource: Some(w.input_object),
                ttl_secs: 60,
                max_uses: 1,
            },
        )
        .unwrap_err(),
        DataPlaneError::BadIssuer
    );
}

// ── capability replay, scope, expiry, revocation ───────────────────────

#[tokio::test]
async fn capabilities_are_single_use_scoped_bounded_and_revocable() {
    let mut w = World::new();
    w.commit_task().await;
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
    let issue = |dp: &mut InMemoryDataPlane, task_id, object, ttl| {
        dp.issue_capability(
            &issuer,
            &CapabilityRequest {
                task_id,
                operation: Operation::FetchInput,
                resource: Some(object),
                ttl_secs: ttl,
                max_uses: 1,
            },
        )
        .unwrap()
    };

    // Same capability twice → consumed.
    let cap = issue(&mut w.dp, w.task_id, w.input_object, 60);
    let ch = w.dp.issue_challenge(w.executor.address());
    w.dp.fetch_input(&Presentation::sign(cap.clone(), ch, &w.executor)).unwrap();
    let ch2 = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.fetch_input(&Presentation::sign(cap.clone(), ch2, &w.executor))
            .unwrap_err(),
        DataPlaneError::Consumed
    );
    // Replaying the exact presentation (spent challenge) → bad challenge.
    let replay = Presentation::sign(cap, ch, &w.executor);
    assert!(matches!(
        w.dp.fetch_input(&replay).unwrap_err(),
        DataPlaneError::Consumed | DataPlaneError::BadChallenge
    ));

    // Capability for task A presented against task B's object → refused.
    let other_task = ComputeTask {
        salt: [0x77u8; 32],
        ..w.task.clone()
    };
    let other_id = other_task.task_id();
    let other_obj = w.dp.store_input(
        &w.client,
        other_id,
        other_task.input_commitment,
        w.input.clone(),
        3_600,
    );
    w.dp.register_task(&w.client, other_id, w.executor.address()).unwrap();
    w.dp.delegate_issuer(&w.client, other_id, issuer.address()).unwrap();
    let mut cross = issue(&mut w.dp, w.task_id, w.input_object, 60);
    cross.resource = other_obj; // tamper: breaks the issuer signature
    let ch = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.fetch_input(&Presentation::sign(cross, ch, &w.executor)).unwrap_err(),
        DataPlaneError::BadIssuerSignature
    );
    // A genuine grant for task B cannot be turned into one for task A.
    let mut cross = issue(&mut w.dp, other_id, other_obj, 60);
    cross.task_id = w.task_id;
    let ch = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.fetch_input(&Presentation::sign(cross, ch, &w.executor)).unwrap_err(),
        DataPlaneError::BadIssuerSignature
    );

    // Expired → refused.
    let short = issue(&mut w.dp, other_id, other_obj, 10);
    w.clock.advance(11);
    let ch = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.fetch_input(&Presentation::sign(short, ch, &w.executor)).unwrap_err(),
        DataPlaneError::Expired
    );

    // Revoked → refused, and revocation is final.
    let revoked = issue(&mut w.dp, other_id, other_obj, 60);
    w.dp.revoke(&issuer, other_id, revoked.capability_id).unwrap();
    let ch = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.fetch_input(&Presentation::sign(revoked, ch, &w.executor)).unwrap_err(),
        DataPlaneError::Revoked
    );

    // A put-result capability cannot fetch, and vice versa.
    let put =
        w.dp.issue_capability(
            &issuer,
            &CapabilityRequest {
                task_id: other_id,
                operation: Operation::PutResult,
                resource: None,
                ttl_secs: 60,
                max_uses: 1,
            },
        )
        .unwrap();
    let ch = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.fetch_input(&Presentation::sign(put, ch, &w.executor)).unwrap_err(),
        DataPlaneError::WrongOperation
    );
}

// ── commitment verification ────────────────────────────────────────────

#[tokio::test]
async fn corrupted_input_is_never_executed_persisted_or_anchored() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    worker.inject(Fault::CorruptInput);
    assert_eq!(
        w.run(&mut worker).await,
        AttemptOutcome::Failed {
            task_id: w.task_id,
            class: FailureClass::Input
        }
    );
    assert!(
        w.dp.result_ref(w.task_id, w.executor.address()).is_none(),
        "no result"
    );
    assert!(w.chain.pending().is_empty(), "no receipt");
    assert_eq!(w.cp.task(w.task_id).unwrap().state, TaskState::Failed);
    assert_eq!(w.cp.task(w.task_id).unwrap().lease, None, "lease released");
    // The capability was consumed by the fetch: a retry needs a fresh one.
    assert_eq!(
        w.dp.object_state(&w.input_object),
        Some(ObjectState::Consumed)
    );
}

#[tokio::test]
async fn unsupported_spec_is_not_executed() {
    let mut w = World::new();
    w.task.execution_spec = b"some-other-profile:v9".to_vec();
    w.task_id = w.task.task_id();
    w.input_object = w.dp.store_input(
        &w.client,
        w.task_id,
        w.task.input_commitment,
        w.input.clone(),
        3_600,
    );
    w.dp.register_task(&w.client, w.task_id, w.executor.address()).unwrap();
    w.dp.delegate_issuer(
        &w.client,
        w.task_id,
        LocalKey::from_seed(&[0xC0u8; 32]).address(),
    )
    .unwrap();
    w.commit_task().await;
    let mut worker = w.worker();
    // The control plane does not even offer it: the session claims no such profile.
    assert_eq!(w.run(&mut worker).await, AttemptOutcome::Idle);
}

// ── result before receipt ──────────────────────────────────────────────

#[tokio::test]
async fn result_persistence_failure_prevents_any_receipt() {
    let mut w = World::new();
    w.commit_task().await;
    w.dp.inject(DataPlaneFault::PutResultFails);
    let mut worker = w.worker();
    assert_eq!(
        w.run(&mut worker).await,
        AttemptOutcome::Failed {
            task_id: w.task_id,
            class: FailureClass::Persistence
        }
    );
    assert!(w.dp.result_ref(w.task_id, w.executor.address()).is_none());
    assert_eq!(w.chain.submissions(), 1, "only the task was ever submitted");
    assert!(w.chain.pending().is_empty(), "no receipt was submitted");
    // Recovery: a new attempt with a fresh capability succeeds and anchors.
    let mut worker2 = w.worker();
    assert!(matches!(
        w.run(&mut worker2).await,
        AttemptOutcome::Submitted { .. }
    ));
}

// ── crash recovery, by phase ───────────────────────────────────────────

#[tokio::test]
async fn crash_before_fetch_leaves_nothing_consumed_and_a_new_attempt_succeeds() {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    w1.inject(Fault::CrashBeforeFetch);
    assert_eq!(
        w.run(&mut w1).await,
        AttemptOutcome::Crashed {
            task_id: w.task_id,
            phase: "before-fetch"
        }
    );
    assert_eq!(
        w.dp.object_state(&w.input_object),
        Some(ObjectState::Authorized)
    );
    let first_attempt = w.cp.task(w.task_id).unwrap().attempt_count;

    // The lease is still live: a second instance is not offered the task.
    let mut w2 = w.worker();
    assert_eq!(w.run(&mut w2).await, AttemptOutcome::Idle);
    // Once it expires, the next instance gets a new attempt, and the old
    // capability (issued under the dead lease) is revoked.
    w.clock.advance(LEASE_SECS + 1);
    assert!(matches!(
        w.run(&mut w2).await,
        AttemptOutcome::Submitted { .. }
    ));
    assert_eq!(
        w.cp.task(w.task_id).unwrap().attempt_count,
        first_attempt + 1
    );
    let caps = &w.cp.task(w.task_id).unwrap().capabilities;
    assert!(caps.len() >= 3, "old fetch, new fetch, new put");
}

#[tokio::test]
async fn crash_after_fetch_requires_a_fresh_capability_and_never_reopens_the_old() {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    w1.inject(Fault::CrashAfterFetch);
    assert_eq!(
        w.run(&mut w1).await,
        AttemptOutcome::Crashed {
            task_id: w.task_id,
            phase: "after-fetch"
        }
    );
    assert_eq!(
        w.dp.object_state(&w.input_object),
        Some(ObjectState::Consumed)
    );
    let (old_cap, old_attempt) = w.cp.task(w.task_id).unwrap().capabilities[0];

    w.clock.advance(LEASE_SECS + 1);
    let mut w2 = w.worker();
    let outcome = w.run(&mut w2).await;
    assert!(
        matches!(outcome, AttemptOutcome::Submitted { .. }),
        "{outcome:?}"
    );
    let caps = &w.cp.task(w.task_id).unwrap().capabilities;
    let fresh = caps
        .iter()
        .find(|(id, a)| *id != old_cap && *a != old_attempt)
        .expect("a fresh capability under a new attempt");
    assert_ne!(fresh.1, old_attempt);
    // The old grant is dead: revoked at lease expiry, and consumed anyway.
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
    let _ = issuer;
}

#[tokio::test]
async fn crash_during_execution_is_retried_by_the_same_executor_not_reassigned() {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    w1.inject(Fault::CrashDuringExecution);
    assert_eq!(
        w.run(&mut w1).await,
        AttemptOutcome::Crashed {
            task_id: w.task_id,
            phase: "executing"
        }
    );
    assert!(w.dp.result_ref(w.task_id, w.executor.address()).is_none());
    w.clock.advance(LEASE_SECS + 1);
    let mut w2 = w.worker();
    assert!(matches!(
        w.run(&mut w2).await,
        AttemptOutcome::Submitted { .. }
    ));
    let pending = w.chain.pending();
    assert_eq!(
        pending[0].sender,
        w.executor.address(),
        "the same executor, a new instance"
    );
}

#[tokio::test]
async fn crash_after_persist_reuses_the_durable_result_and_the_receipt_is_identical() {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    w1.inject(Fault::CrashAfterPersist);
    assert_eq!(
        w.run(&mut w1).await,
        AttemptOutcome::Crashed {
            task_id: w.task_id,
            phase: "after-persist"
        }
    );
    let result = w.dp.result_ref(w.task_id, w.executor.address()).expect("result is durable");
    assert!(w.chain.pending().is_empty(), "no receipt yet");

    w.clock.advance(LEASE_SECS + 1);
    let mut w2 = w.worker();
    let outcome = w.run(&mut w2).await;
    assert!(
        matches!(outcome, AttemptOutcome::Submitted { .. }),
        "{outcome:?}"
    );
    // Not recomputed: the input object was consumed once and no second
    // fetch capability was issued under the new attempt.
    let caps = &w.cp.task(w.task_id).unwrap().capabilities;
    assert_eq!(
        caps.len(),
        2,
        "one fetch and one put, both from the first attempt"
    );
    let pending = w.chain.pending();
    let TransactionPayload::AnchorReceipt(receipt) = &pending[0].payload else {
        panic!()
    };
    assert_eq!(receipt.output_commitment, result.output_commitment);
    // Deterministic: the receipt a fresh instance rebuilds is byte-identical
    // to what the crashed one would have sent.
    let rebuilt = w2.bound_receipt(&w.task, result.output_commitment);
    assert_eq!(&rebuilt, receipt.as_ref());
}

#[tokio::test]
async fn crash_after_submit_resolves_by_lookup_and_never_signs_a_second_receipt() {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    w1.inject(Fault::CrashAfterSubmit);
    assert_eq!(
        w.run(&mut w1).await,
        AttemptOutcome::Crashed {
            task_id: w.task_id,
            phase: "after-submit"
        }
    );
    assert_eq!(w.chain.pending().len(), 1);
    let first = w.chain.pending()[0].clone();

    // Case A: the anchor lands before the retry. Lookup finds it.
    let height = w.chain.produce_block();
    w.clock.advance(LEASE_SECS + 1);
    let mut w2 = w.worker();
    assert_eq!(
        w.run(&mut w2).await,
        AttemptOutcome::Completed {
            task_id: w.task_id,
            height
        }
    );
    assert_eq!(
        w.chain.submissions(),
        2,
        "task + one anchor; nothing resubmitted"
    );

    // Case B: the anchor is still pending. The retry resubmits identical
    // bytes, which the chain treats as idempotent.
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    w1.inject(Fault::CrashAfterSubmit);
    w.run(&mut w1).await;
    w.clock.advance(LEASE_SECS + 1);
    let mut w2 = w.worker();
    let outcome = w.run(&mut w2).await;
    assert!(matches!(outcome, AttemptOutcome::Submitted { .. }));
    let pending = w.chain.pending();
    assert_eq!(pending.len(), 1, "still exactly one anchoring transaction");
    assert_eq!(
        transaction_hash(&pending[0]),
        transaction_hash(&first),
        "same bytes as the first attempt's"
    );
}

#[tokio::test]
async fn ambiguous_submission_is_looked_up_before_any_resubmission() {
    let mut w = World::new();
    w.commit_task().await;
    w.chain.lose_next_response();
    let mut worker = w.worker();
    // The chain accepted the anchor but the response was lost; the worker
    // looks up (not in a block yet), then resubmits the same bytes, which
    // the chain dedupes.
    let outcome = w.run(&mut worker).await;
    assert!(
        matches!(outcome, AttemptOutcome::Submitted { .. }),
        "{outcome:?}"
    );
    assert_eq!(w.chain.pending().len(), 1);
    assert_eq!(
        w.chain.submissions(),
        3,
        "task, lost anchor, identical resubmission"
    );

    // And if it had landed, lookup alone resolves it.
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    w.run(&mut worker).await;
    let height = w.chain.produce_block();
    let latest = w.chain.latest_height().await.unwrap();
    assert!(find_receipt(&w.chain, w.task_id, 1, latest).await.unwrap().is_some());
    assert_eq!(
        w.run(&mut worker).await,
        AttemptOutcome::Completed {
            task_id: w.task_id,
            height
        }
    );
}

// ── stale workers and duplicates ───────────────────────────────────────

#[tokio::test]
async fn stale_worker_is_fenced_and_duplicates_settle_on_one_result() {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    let s1 = w1.ensure_session(&mut w.cp).unwrap();
    let l1 = w.cp.offer(s1.session_id, &mut w.dp).unwrap().unwrap();
    let cap1 = w.cp.authorize_fetch(s1.session_id, l1.lease_id, &mut w.dp).unwrap();

    // Instance 1 goes quiet; its lease expires; instance 2 takes over.
    w.clock.advance(LEASE_SECS + 1);
    let mut w2 = w.worker();
    let outcome = w.run(&mut w2).await;
    assert!(matches!(outcome, AttemptOutcome::Submitted { .. }));

    // Instance 1 reappears: its reports are refused, its heartbeat is
    // refused, and its old capability was revoked with its lease.
    assert_eq!(
        w.cp.heartbeat(s1.session_id, l1.lease_id).unwrap_err(),
        ControlPlaneError::StaleLease
    );
    assert_eq!(
        w.cp.report(s1.session_id, l1.lease_id, AttemptEvent::Started, &mut w.dp)
            .unwrap_err(),
        ControlPlaneError::StaleLease
    );
    let ch = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.fetch_input(&Presentation::sign(cap1, ch, &w.executor)).unwrap_err(),
        DataPlaneError::Revoked
    );

    // Had both computed, the data plane keeps one result: a second put for
    // the same (task, executor) is refused with the existing reference.
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
    let put =
        w.dp.issue_capability(
            &issuer,
            &CapabilityRequest {
                task_id: w.task_id,
                operation: Operation::PutResult,
                resource: None,
                ttl_secs: 60,
                max_uses: 1,
            },
        )
        .unwrap();
    let ch = w.dp.issue_challenge(w.executor.address());
    let existing = w.dp.result_ref(w.task_id, w.executor.address()).unwrap();
    match w.dp.put_result(
        &Presentation::sign(put, ch, &w.executor),
        vec![9, 9, 9],
        [9u8; 32],
    ) {
        Err(DataPlaneError::ResultAlreadyExists(r)) => assert_eq!(*r, existing),
        other => panic!("second put must be refused: {other:?}"),
    }
    // No exactly-once claim: two attempts happened (attempt_count 2); the
    // chain would settle any second anchor by first-anchored-wins.
    assert_eq!(w.cp.task(w.task_id).unwrap().attempt_count, 2);
}

#[tokio::test]
async fn heartbeat_keeps_a_lease_alive_and_a_missed_one_lets_it_expire() {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    let s1 = w1.ensure_session(&mut w.cp).unwrap();
    let l1 = w.cp.offer(s1.session_id, &mut w.dp).unwrap().unwrap();
    assert!(ControlPlane::lease_signature_valid(&l1));
    w.clock.advance(LEASE_SECS - 5);
    let renewed = w.cp.heartbeat(s1.session_id, l1.lease_id).unwrap();
    assert!(renewed.not_after > l1.not_after);
    assert!(ControlPlane::lease_signature_valid(&renewed));
    w.clock.advance(LEASE_SECS - 5);
    assert!(w.cp.lease_is_live(l1.lease_id));
    w.clock.advance(6);
    assert!(!w.cp.lease_is_live(l1.lease_id));
    assert_eq!(
        w.cp.heartbeat(s1.session_id, l1.lease_id).unwrap_err(),
        ControlPlaneError::StaleLease
    );
}

// ── control-plane restart ──────────────────────────────────────────────

#[tokio::test]
async fn control_plane_restart_forgets_leases_keeps_attempts_and_reassigns_nothing() {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    w1.inject(Fault::CrashAfterPersist);
    w.run(&mut w1).await;
    let before = w.cp.task(w.task_id).unwrap().clone();
    let snapshot = w.cp.snapshot();
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(
        !json.contains("issuer_signature"),
        "no lease grant material is persisted"
    );

    let clock: Arc<dyn Clock> = Arc::new(w.clock.clone());
    let restored: mbongo_compute::control_plane::DurableState =
        serde_json::from_str(&json).unwrap();
    w.cp = ControlPlane::restore(
        restored,
        clock,
        IdSource::new([0x44u8; 32]),
        LocalKey::from_seed(&[0xC0u8; 32]),
        ControlPlaneConfig {
            lease_secs: LEASE_SECS,
            session_secs: 600,
            confirmation_depth: 1,
            capability_secs: 120,
        },
    );
    let after = w.cp.task(w.task_id).unwrap();
    assert_eq!(
        after.task.executor, before.task.executor,
        "executor survives unchanged"
    );
    assert_eq!(after.attempt_count, before.attempt_count);
    assert_eq!(after.lease, None, "unrecoverable leases are expired");
    assert_eq!(after.state, TaskState::Discovered);

    // The old instance is stale; a new attempt reuses the durable result.
    let mut w2 = w.worker();
    w.cp.observe(&w.chain).await.unwrap();
    let outcome = w.run(&mut w2).await;
    assert!(
        matches!(outcome, AttemptOutcome::Submitted { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        w.cp.task(w.task_id).unwrap().attempt_count,
        before.attempt_count + 1
    );
}

// ── private result authorization ───────────────────────────────────────

#[tokio::test]
async fn private_result_is_released_only_to_the_owner_under_a_grant() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    w.run(&mut worker).await;
    let result = w.dp.result_ref(w.task_id, w.executor.address()).unwrap();
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
    let req = CapabilityRequest {
        task_id: w.task_id,
        operation: Operation::GetResult,
        resource: Some(result.object),
        ttl_secs: 60,
        max_uses: 1,
    };
    // A get grant names the owner; the executor cannot present it, nor can
    // a stranger, nor can the owner without a valid challenge.
    let get = w.dp.issue_capability(&issuer, &req).unwrap();
    assert_eq!(get.presenter, w.client.address());
    let ch = w.dp.issue_challenge(w.executor.address());
    assert_eq!(
        w.dp.get_result(&Presentation::sign(get.clone(), ch, &w.executor)).unwrap_err(),
        DataPlaneError::BadChallenge
    );
    let stranger = LocalKey::from_seed(&[0xDDu8; 32]);
    let ch = w.dp.issue_challenge(w.client.address());
    assert_eq!(
        w.dp.get_result(&Presentation::sign(get.clone(), ch, &stranger)).unwrap_err(),
        DataPlaneError::BadPossessionProof
    );
    let ch = w.dp.issue_challenge(w.client.address());
    assert!(w.dp.get_result(&Presentation::sign(get.clone(), ch, &w.client)).is_ok());
    // Bounded: one use.
    let ch = w.dp.issue_challenge(w.client.address());
    assert_eq!(
        w.dp.get_result(&Presentation::sign(get, ch, &w.client)).unwrap_err(),
        DataPlaneError::Consumed
    );
    // Expired grant.
    let get =
        w.dp.issue_capability(
            &issuer,
            &CapabilityRequest {
                ttl_secs: 5,
                ..req.clone()
            },
        )
        .unwrap();
    w.clock.advance(6);
    let ch = w.dp.issue_challenge(w.client.address());
    assert_eq!(
        w.dp.get_result(&Presentation::sign(get, ch, &w.client)).unwrap_err(),
        DataPlaneError::Expired
    );
    // Deleted object: the receipt stays, the content is gone.
    w.dp.delete_object(&w.client, &result.object).unwrap();
    let get = w.dp.issue_capability(&issuer, &req).unwrap();
    let ch = w.dp.issue_challenge(w.client.address());
    assert_eq!(
        w.dp.get_result(&Presentation::sign(get, ch, &w.client)).unwrap_err(),
        DataPlaneError::Expired
    );
}

// ── logging and secrets ────────────────────────────────────────────────

#[tokio::test]
async fn debug_output_never_carries_payloads_keys_or_proofs() {
    let mut w = World::new();
    w.commit_task().await;
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
    let cap =
        w.dp.issue_capability(
            &issuer,
            &CapabilityRequest {
                task_id: w.task_id,
                operation: Operation::FetchInput,
                resource: Some(w.input_object),
                ttl_secs: 60,
                max_uses: 1,
            },
        )
        .unwrap();
    let ch = w.dp.issue_challenge(w.executor.address());
    let p = Presentation::sign(cap.clone(), ch, &w.executor);
    let worker = w.worker();
    let shown = format!("{p:?} {cap:?} {:?} {worker:?}", w.executor);
    assert!(!shown.contains(&hex::encode(p.proof)));
    assert!(!shown.contains(&hex::encode(cap.issuer_signature)));
    assert!(!shown.contains(&hex::encode([0xE1u8; 32])), "executor seed");
    let input_hex = hex::encode(&w.input);
    assert!(!shown.contains(&input_hex));
    let input = w.dp.fetch_input(&p).unwrap();
    assert!(!format!("{input:?}").contains(&input_hex));
    assert!(!format!("{input:?}").contains(std::str::from_utf8(&w.input).unwrap()));
}
