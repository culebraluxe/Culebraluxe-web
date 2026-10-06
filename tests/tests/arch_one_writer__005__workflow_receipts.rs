//! ARCH.ONE_WRITER — workflow receipts (TST-ARCH-ONE-WRITER-005).
//!
//! Contract: a workflow receipt is judged by exactly one production function,
//! `assess_release_receipt` in `forge/src/engine/release_receipt.rs`. A clean
//! agent result alone is never a receipt (migration 112 stores receipts as
//! `deployment_receipt` / `production_verification_receipt` columns on
//! `forge_workflow_evidence`); the receipt counts only when its artifact sha
//! is a real commit sha and its id is not a placeholder.
//!
//! Level: L0 Pure — pure function calls and file reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__005__workflow_receipts

use forge::engine::release_receipt::{
    assess_release_receipt, deployment_receipt_failure_reason, is_commit_sha,
    is_placeholder_receipt_id, ReleaseEvidence, ReleaseReceiptKind,
};
use test_harness::source;

fn evidence(sha: &str, id: &str, success: bool) -> ReleaseEvidence {
    ReleaseEvidence {
        kind: ReleaseReceiptKind::Deployment,
        artifact_sha: sha.into(),
        receipt_id: id.into(),
        success,
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-005); the file and the assay use it.
fn arch_one_writer_005__workflow_receipts() {
    // The placeholder detector refuses empties and stand-ins, and accepts a real receipt id.
    assert!(is_placeholder_receipt_id(None));
    assert!(is_placeholder_receipt_id(Some("")));
    assert!(is_placeholder_receipt_id(Some("n/a")));
    assert!(is_placeholder_receipt_id(Some("tbd - see memo")));
    assert!(!is_placeholder_receipt_id(Some("deploy:abc1234")));

    // The sha detector accepts short and full hex shas, and refuses everything else.
    assert!(is_commit_sha(Some("abc1234")));
    assert!(is_commit_sha(Some(
        "9f3a1c2e4b5d6f708192a3b4c5d6e7f8091a2b3c"
    )));
    assert!(!is_commit_sha(None));
    assert!(!is_commit_sha(Some("not-a-sha")));
    assert!(!is_commit_sha(Some("abc"))); // too short to be a sha

    // The assessment is the single judge: no receipt, bad sha, or placeholder id each fail.
    let missing = assess_release_receipt(None);
    assert!(!missing.ok && missing.reason.is_some());
    let bad_sha = assess_release_receipt(Some(&evidence("nope", "deploy:abc1234", true)));
    assert!(!bad_sha.ok);
    let placeholder = assess_release_receipt(Some(&evidence("abc1234", "tbd", true)));
    assert!(!placeholder.ok);
    let good = assess_release_receipt(Some(&evidence("abc1234", "deploy:abc1234", true)));
    assert!(good.ok && good.reason.is_none());

    // The deployment reason compares against the published sha and the success flag.
    assert!(deployment_receipt_failure_reason(None, Some("abc1234")).is_some());
    let mismatch = deployment_receipt_failure_reason(
        Some(&evidence("abc1234", "deploy:abc1234", true)),
        Some("deadbee"),
    );
    assert!(mismatch.is_some());
    let failed = deployment_receipt_failure_reason(
        Some(&evidence("abc1234", "deploy:abc1234", false)),
        Some("abc1234"),
    );
    assert!(failed.is_some());
    let clean = deployment_receipt_failure_reason(
        Some(&evidence("abc1234", "deploy:abc1234", true)),
        Some("abc1234"),
    );
    assert_eq!(clean, None);

    // One judge, not two: no other non-test file in the workspace defines the assessment.
    let workspace = source::sources_under(&source::workspace_root());
    let definers: Vec<String> = workspace
        .iter()
        .filter(|path| !source::relative(path).contains("/tests/"))
        .filter(|path| source::read(path).contains("fn assess_release_receipt"))
        .map(|path| source::relative(path))
        .collect();
    assert_eq!(
        definers,
        vec!["forge/src/engine/release_receipt.rs"],
        "the workflow receipt has one assessing function; a second definition is a second judge"
    );

    // The receipt columns live on the evidence table, not beside it.
    let migration = source::read(
        &source::workspace_root().join("db/migrations/112_forge_v10_release_receipts.sql"),
    );
    assert!(
        migration.contains("forge_workflow_evidence")
            && migration.contains("deployment_receipt")
            && migration.contains("production_verification_receipt"),
        "migration 112 stores both receipt columns on forge_workflow_evidence"
    );

    // Negative controls: the detectors fire on junk and stay quiet on real values.
    assert!(is_placeholder_receipt_id(Some(" waived ")));
    assert!(!is_commit_sha(Some("zzzzzzz")));
}
