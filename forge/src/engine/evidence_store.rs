//! The one mapping from gate evidence to the `forge_workflow_evidence` row.
//!
//! The merge itself lives in `db::ForgeEngineDao::merge_workflow_evidence` (one writer, one fact) and is
//! driven by the completion ledger (`engine/db_ledger.rs`), which applies it inside the unit it finalizes.
//! The `merge_forge_workflow_evidence` wrapper that used to sit above this mapping had no caller: a second
//! door onto the same row. It was deleted on 2026-09-29 rather than wired, because wiring it would give
//! `forge_workflow_evidence` two writers — the audit pins that table at exactly one.

use crate::engine::facts::ForgeGateEvidence;
use db::ForgeEvidencePatch;

/// The one mapping from gate evidence to the `forge_workflow_evidence` row. Shared with the completion
/// ledger so the port and the ledger cannot disagree about which column a field lands in.
pub fn evidence_patch(evidence: &ForgeGateEvidence) -> ForgeEvidencePatch {
    ForgeEvidencePatch {
        work_type: evidence.work_type.clone(),
        scout_required: evidence.scout_required,
        lead_decision: evidence.lead_decision.clone(),
        qa_review_required: evidence.qa_review_required,
        qa_review_passed: evidence.qa_review_passed,
        qa_passed: evidence.qa_passed,
        failure_class: evidence.failure_class.clone(),
        failed_release_stage: evidence.failed_release_stage.clone(),
        last_failure: evidence.last_failure.clone(),
        publish_succeeded: evidence.publish_succeeded,
        candidate_sha: evidence.candidate_sha.clone(),
        qa_verified_sha: evidence.qa_verified_sha.clone(),
        published_sha: evidence.published_sha.clone(),
    }
}
