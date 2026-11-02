//! The reference implementation must pass Mbongo Compute Conformance in
//! full, and the catalog must cover every mandatory case.

use mbongo_compute::conformance::reference::ReferenceSubject;
use mbongo_compute::conformance::{catalog, run_all, Group, Status, CONFORMANCE_VERSION};

#[tokio::test]
async fn reference_implementation_is_conformant() {
    let report = run_all(ReferenceSubject::new).await;
    let rendered = report.render();
    println!("{rendered}");
    for c in &report.cases {
        assert_eq!(
            c.status,
            Status::Pass,
            "{} {}: {:?}",
            c.case.id,
            c.case.name,
            c.status
        );
    }
    for g in Group::ALL {
        assert!(report.group_passed(g), "group {} failed", g.label());
    }
    assert!(report.passed());
    assert_eq!(report.version, CONFORMANCE_VERSION);
    assert!(rendered.starts_with("MBONGO_COMPUTE_CONFORMANCE\n"));
    assert!(rendered.contains("\nRESULT: PASS ("));
}

#[test]
fn catalog_is_complete_and_stable() {
    let cases = catalog();
    assert_eq!(cases.len(), 38, "expected 38 cases");
    // The 32 mandatory cases, in order, plus the six authority-driven additions.
    let ids: Vec<&str> = cases.iter().map(|c| c.id).collect();
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, format!("C{:02}", i + 1));
    }
    let names: Vec<&str> = cases.iter().map(|c| c.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "case names must be unique");
    for c in &cases {
        assert!(!c.invariants.is_empty(), "{} cites no invariant", c.id);
        let prefix = match c.group {
            Group::Core => "core_",
            Group::Privacy => "privacy_",
            Group::Authorization => "auth_",
            Group::Lifecycle => "lifecycle_",
            Group::FailureRecovery => "recovery_",
            Group::Observability => "observability_",
        };
        assert!(
            c.name.starts_with(prefix),
            "{} is named {} but grouped {:?}",
            c.id,
            c.name,
            c.group
        );
    }
    // The Workstream I invariants each map to at least one case.
    for inv in ["P4", "P5", "P15", "F19"] {
        assert!(
            cases.iter().any(|c| c.invariants.contains(&inv)),
            "{inv} is not mapped to any case"
        );
    }
    let mandatory = [
        "core_correct_executor_completes_flow",
        "auth_wrong_executor_cannot_execute",
        "core_control_plane_cannot_replace_executor",
        "auth_task_id_is_not_a_capability",
        "auth_locator_is_not_a_capability",
        "auth_capability_is_task_scoped",
        "auth_capability_is_resource_scoped",
        "auth_capability_is_executor_scoped",
        "auth_capability_requires_proof_of_possession",
        "auth_input_capability_is_single_use",
        "auth_consumed_capability_replay_fails",
        "auth_expired_capability_fails",
        "auth_revoked_capability_fails",
        "auth_cross_task_replay_fails",
        "auth_cross_executor_replay_fails",
        "auth_lease_alone_grants_no_data",
        "auth_capability_alone_grants_no_execution",
        "lifecycle_commitment_verified_before_execution",
        "lifecycle_corrupted_input_blocks_execution",
        "lifecycle_result_persisted_before_receipt",
        "lifecycle_result_persistence_failure_blocks_receipt",
        "recovery_crash_after_fetch_requires_fresh_capability",
        "recovery_stale_attempt_is_fenced",
        "recovery_duplicate_execution_does_not_overwrite_result",
        "core_exactly_once_is_not_claimed",
        "auth_task_id_alone_cannot_retrieve_result",
        "auth_unauthorized_result_retrieval_fails",
        "privacy_raw_input_absent_from_chain",
        "privacy_raw_output_absent_from_chain",
        "observability_diagnostics_omit_payloads_and_secrets",
        "core_control_plane_cannot_forge_executor_signature",
        "core_anchored_receipt_is_not_output_correctness",
    ];
    assert_eq!(mandatory.len(), 32);
    for m in mandatory {
        assert!(names.contains(&m), "mandatory case {m} missing");
    }
}
