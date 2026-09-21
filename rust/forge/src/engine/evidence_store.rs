//! Port of db/forge-workflow-evidence.ts merge + read. Same table, same coalesce rules.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::vendor_session::with_shared;

pub fn merge_forge_workflow_evidence(
    process_instance_id: &str,
    story_id: &str,
    evidence: &ForgeGateEvidence,
    release_failure_resolved: bool,
) -> Result<(), String> {
    // Binds, on the workspace pool. The statement text no longer varies with the data: the fifteen values are
    // parameters, and the "clear the failure columns" flag is one more, instead of `true`/`false` written into three
    // CASE expressions by string formatting. The coalesce rules below are unchanged.
    with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query(
                "insert into forge_workflow_evidence (
                    process_instance_id, story_id, work_type, scout_required, lead_decision,
                    qa_review_required, qa_review_passed, qa_passed, failure_class, failed_release_stage,
                    last_failure, publish_succeeded, candidate_sha, qa_verified_sha, published_sha
                 ) values (
                    $1::uuid, $2, $3, $4, $5,
                    $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15
                 )
                 on conflict (process_instance_id) do update set
                    work_type = coalesce(excluded.work_type, forge_workflow_evidence.work_type),
                    scout_required = coalesce(excluded.scout_required, forge_workflow_evidence.scout_required),
                    lead_decision = coalesce(excluded.lead_decision, forge_workflow_evidence.lead_decision),
                    qa_review_required = coalesce(excluded.qa_review_required, forge_workflow_evidence.qa_review_required),
                    qa_review_passed = coalesce(excluded.qa_review_passed, forge_workflow_evidence.qa_review_passed),
                    qa_passed = coalesce(excluded.qa_passed, forge_workflow_evidence.qa_passed),
                    failure_class = case when $16 then null
                      else coalesce(excluded.failure_class, forge_workflow_evidence.failure_class) end,
                    failed_release_stage = case when $16 then null
                      else coalesce(excluded.failed_release_stage, forge_workflow_evidence.failed_release_stage) end,
                    last_failure = case when $16 then null
                      else coalesce(excluded.last_failure, forge_workflow_evidence.last_failure) end,
                    publish_succeeded = coalesce(excluded.publish_succeeded, forge_workflow_evidence.publish_succeeded),
                    candidate_sha = coalesce(excluded.candidate_sha, forge_workflow_evidence.candidate_sha),
                    qa_verified_sha = coalesce(excluded.qa_verified_sha, forge_workflow_evidence.qa_verified_sha),
                    published_sha = coalesce(excluded.published_sha, forge_workflow_evidence.published_sha),
                    updated_at = now()",
            )
            .bind(process_instance_id)
            .bind(story_id)
            .bind(evidence.work_type.as_deref())
            .bind(evidence.scout_required)
            .bind(evidence.lead_decision.as_deref())
            .bind(evidence.qa_review_required)
            .bind(evidence.qa_review_passed)
            .bind(evidence.qa_passed)
            .bind(evidence.failure_class.as_deref())
            .bind(evidence.failed_release_stage.as_deref())
            .bind(evidence.last_failure.as_deref())
            .bind(evidence.publish_succeeded)
            .bind(evidence.candidate_sha.as_deref())
            .bind(evidence.qa_verified_sha.as_deref())
            .bind(evidence.published_sha.as_deref())
            .bind(release_failure_resolved)
            .execute(db.pool())
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
        })
    })??;
    Ok(())
}
