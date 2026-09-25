//! Forge workflow evidence merge. Persistence lives in db::ForgeEngineDao.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::vendor_session::with_shared;
use db::{ForgeEngineDao, ForgeEvidencePatch};

pub fn merge_forge_workflow_evidence(
    process_instance_id: &str,
    story_id: &str,
    evidence: &ForgeGateEvidence,
    release_failure_resolved: bool,
) -> Result<(), String> {
    let patch = ForgeEvidencePatch {
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
    };

    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.merge_workflow_evidence(
                process_instance_id,
                story_id,
                &patch,
                release_failure_resolved,
            )
            .await
            .map_err(|error| error.to_string())
        })
    })?
}
