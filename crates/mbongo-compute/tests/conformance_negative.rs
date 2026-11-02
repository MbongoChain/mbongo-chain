//! The suite is not vacuous: an adapter that misbehaves in a known way
//! fails the cases that guard that behaviour, and only those.
//!
//! Each misbehaviour wraps the reference subject and lies at the adapter
//! boundary, which is exactly where a careless future implementation
//! would: a task id honoured as a credential, an exactly-once claim, a
//! confidentiality claim, private bytes in the logs, a consumed grant
//! reopened.

use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::conformance::reference::ReferenceSubject;
use mbongo_compute::conformance::{
    run_all, Actor, Claims, FaultPoint, GrantOp, Outcome, Refusal, Retarget, Status, Subject,
};
use mbongo_core::{Address, ComputeTask, Receipt, Transaction};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Misbehaviour {
    TaskIdIsACredential,
    ClaimsExactlyOnce,
    ClaimsConfidential,
    LogsPrivateInput,
    ReopensConsumedGrant,
}

struct Misbehaving {
    inner: ReferenceSubject,
    how: Misbehaviour,
}

impl Subject for Misbehaving {
    type Chain = FakeChain;
    type Grant = <ReferenceSubject as Subject>::Grant;
    type Lease = <ReferenceSubject as Subject>::Lease;
    type Locator = <ReferenceSubject as Subject>::Locator;
    type Instance = <ReferenceSubject as Subject>::Instance;

    fn info(&self) -> mbongo_compute::conformance::SubjectInfo {
        let mut i = self.inner.info();
        i.name = format!("misbehaving({:?})", self.how);
        i.claims = Claims {
            exactly_once: self.how == Misbehaviour::ClaimsExactlyOnce,
            confidential_execution: self.how == Misbehaviour::ClaimsConfidential,
        };
        i
    }
    fn chain(&self) -> &FakeChain {
        self.inner.chain()
    }
    fn produce_block(&mut self) {
        self.inner.produce_block();
    }
    fn advance_time(&mut self, secs: u64) {
        self.inner.advance_time(secs);
    }
    fn lease_ttl_secs(&self) -> u64 {
        self.inner.lease_ttl_secs()
    }
    fn grant_ttl_secs(&self) -> u64 {
        self.inner.grant_ttl_secs()
    }
    fn private_input(&self) -> Vec<u8> {
        self.inner.private_input()
    }
    fn expected_output(&self) -> Vec<u8> {
        self.inner.expected_output()
    }
    fn task(&self) -> ComputeTask {
        self.inner.task()
    }
    fn input_locator(&self) -> Self::Locator {
        self.inner.input_locator()
    }
    fn address(&self, actor: Actor) -> Address {
        self.inner.address(actor)
    }
    async fn commit_task(&mut self) {
        self.inner.commit_task().await;
    }
    async fn commit_task_carrying_input_in_spec(&mut self) -> [u8; 32] {
        self.inner.commit_task_carrying_input_in_spec().await
    }
    fn new_instance(&mut self, actor: Actor) -> Self::Instance {
        self.inner.new_instance(actor)
    }
    fn inject(&mut self, instance: &mut Self::Instance, fault: FaultPoint) -> bool {
        self.inner.inject(instance, fault)
    }
    fn inject_result_store_failure(&mut self) -> bool {
        self.inner.inject_result_store_failure()
    }
    fn inject_lost_submission_response(&mut self) -> bool {
        self.inner.inject_lost_submission_response()
    }
    async fn run(&mut self, instance: &mut Self::Instance) -> Outcome {
        self.inner.run(instance).await
    }
    fn obtain_lease(&mut self, instance: &mut Self::Instance) -> Option<Self::Lease> {
        self.inner.obtain_lease(instance)
    }
    fn grant_under_lease(
        &mut self,
        instance: &Self::Instance,
        lease: &Self::Lease,
        op: GrantOp,
    ) -> Result<Self::Grant, Refusal> {
        self.inner.grant_under_lease(instance, lease, op)
    }
    fn report_started(&mut self, i: &Self::Instance, l: &Self::Lease) -> Result<(), Refusal> {
        self.inner.report_started(i, l)
    }
    fn heartbeat(&mut self, i: &Self::Instance, l: &Self::Lease) -> Result<(), Refusal> {
        self.inner.heartbeat(i, l)
    }
    fn attempt_count(&self, task_id: [u8; 32]) -> u32 {
        self.inner.attempt_count(task_id)
    }
    fn executor_of_record(&self, task_id: [u8; 32]) -> Option<Address> {
        self.inner.executor_of_record(task_id)
    }
    fn owner_side_grant(&mut self, presenter: Actor, op: GrantOp) -> Result<Self::Grant, Refusal> {
        self.inner.owner_side_grant(presenter, op)
    }
    fn present_fetch(&mut self, actor: Actor, grant: &Self::Grant) -> Result<Vec<u8>, Refusal> {
        match self.inner.present_fetch(actor, grant) {
            // A data plane with no consumption or revocation state: it serves
            // the right presenter again whenever the grant was once valid.
            Err(Refusal::Denied(m))
                if self.how == Misbehaviour::ReopensConsumedGrant
                    && (m.contains("Consumed") || m.contains("Revoked"))
                    && actor == Actor::Executor =>
            {
                Ok(self.inner.private_input())
            }
            r => r,
        }
    }
    fn replay_last_presentation(&mut self, grant: &Self::Grant) -> Result<Vec<u8>, Refusal> {
        self.inner.replay_last_presentation(grant)
    }
    fn present_fetch_unproven(&mut self, grant: &Self::Grant) -> Result<Vec<u8>, Refusal> {
        self.inner.present_fetch_unproven(grant)
    }
    fn present_put(&mut self, a: Actor, g: &Self::Grant, b: Vec<u8>) -> Result<(), Refusal> {
        self.inner.present_put(a, g, b)
    }
    fn present_get(&mut self, a: Actor, g: &Self::Grant) -> Result<Vec<u8>, Refusal> {
        self.inner.present_get(a, g)
    }
    fn fetch_with_task_id_only(
        &mut self,
        actor: Actor,
        task_id: [u8; 32],
    ) -> Result<Vec<u8>, Refusal> {
        if self.how == Misbehaviour::TaskIdIsACredential && task_id == self.inner.task().task_id() {
            return Ok(self.inner.private_input());
        }
        self.inner.fetch_with_task_id_only(actor, task_id)
    }
    fn fetch_with_locator_only(&mut self, a: Actor, l: &Self::Locator) -> Result<Vec<u8>, Refusal> {
        self.inner.fetch_with_locator_only(a, l)
    }
    fn fetch_with_lease_only(&mut self, a: Actor, l: &Self::Lease) -> Result<Vec<u8>, Refusal> {
        self.inner.fetch_with_lease_only(a, l)
    }
    fn result_with_task_id_only(&mut self, a: Actor, t: [u8; 32]) -> Result<Vec<u8>, Refusal> {
        self.inner.result_with_task_id_only(a, t)
    }
    fn revoke(&mut self, grant: &Self::Grant) -> Result<(), Refusal> {
        self.inner.revoke(grant)
    }
    fn retarget(&self, grant: &Self::Grant, how: Retarget) -> Self::Grant {
        self.inner.retarget(grant, how)
    }
    fn input_consumed(&self, locator: &Self::Locator) -> bool {
        self.inner.input_consumed(locator)
    }
    fn result_exists(&self, task_id: [u8; 32]) -> bool {
        self.inner.result_exists(task_id)
    }
    fn result_commitment(&self, task_id: [u8; 32]) -> Option<[u8; 32]> {
        self.inner.result_commitment(task_id)
    }
    fn owner_retrieve_result(&mut self, task_id: [u8; 32]) -> Result<Vec<u8>, Refusal> {
        self.inner.owner_retrieve_result(task_id)
    }
    fn control_plane_receipt(&mut self, c: [u8; 32]) -> Receipt {
        self.inner.control_plane_receipt(c)
    }
    fn control_plane_anchor_transaction(&mut self, r: Receipt) -> Transaction {
        self.inner.control_plane_anchor_transaction(r)
    }
    async fn executor_anchor_arbitrary(&mut self, c: [u8; 32]) -> Result<(), Refusal> {
        self.inner.executor_anchor_arbitrary(c).await
    }
    fn diagnostics(&self) -> String {
        let mut d = self.inner.diagnostics();
        if self.how == Misbehaviour::LogsPrivateInput {
            d.push_str(&format!(
                "DEBUG worker fetched input: {}\n",
                String::from_utf8_lossy(&self.inner.private_input())
            ));
        }
        d
    }
    fn secrets(&self) -> Vec<Vec<u8>> {
        self.inner.secrets()
    }
}

async fn failing_cases(how: Misbehaviour) -> Vec<&'static str> {
    let report = run_all(|| Misbehaving {
        inner: ReferenceSubject::new(),
        how,
    })
    .await;
    assert!(!report.passed(), "{how:?} passed the suite");
    report
        .cases
        .iter()
        .filter(|c| c.status != Status::Pass)
        .map(|c| c.case.id)
        .collect()
}

#[tokio::test]
async fn a_task_id_honoured_as_a_credential_fails_the_authorization_cases() {
    assert_eq!(
        failing_cases(Misbehaviour::TaskIdIsACredential).await,
        ["C04"]
    );
}

#[tokio::test]
async fn an_exactly_once_claim_fails() {
    assert_eq!(
        failing_cases(Misbehaviour::ClaimsExactlyOnce).await,
        ["C25"]
    );
}

#[tokio::test]
async fn a_confidentiality_claim_fails() {
    assert_eq!(
        failing_cases(Misbehaviour::ClaimsConfidential).await,
        ["C37"]
    );
}

#[tokio::test]
async fn private_input_in_diagnostics_fails_the_observability_case() {
    assert_eq!(failing_cases(Misbehaviour::LogsPrivateInput).await, ["C30"]);
}

#[tokio::test]
async fn reopening_a_spent_grant_fails_the_single_use_revocation_and_recovery_cases() {
    // Single use (C10), revocation (C13), the crash-after-fetch retry (C22)
    // and the stale attempt (C23) all rest on the data plane remembering
    // that a grant is spent; nothing else does.
    assert_eq!(
        failing_cases(Misbehaviour::ReopensConsumedGrant).await,
        ["C10", "C13", "C22", "C23"]
    );
}
