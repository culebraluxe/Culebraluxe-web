//! Port of db/forge-workflow-evidence.ts merge + read. Same table, same coalesce rules.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::vendor_session::{psql_query, sql_literal};

pub fn merge_forge_workflow_evidence(
    process_instance_id: &str,
    story_id: &str,
    evidence: &ForgeGateEvidence,
    release_failure_resolved: bool,
) -> Result<(), String> {
    let pid = sql_literal(process_instance_id);
    let sid = sql_literal(story_id);
    let clear = if release_failure_resolved {
        "true"
    } else {
        "false"
    };
    let opt = |v: &Option<String>| {
        v.as_deref()
            .map(sql_literal)
            .unwrap_or_else(|| "NULL".into())
    };
    let opt_bool = |v: Option<bool>| match v {
        Some(true) => String::from("true"),
        Some(false) => String::from("false"),
        None => String::from("NULL"),
    };
    let sql = format!(
        "INSERT INTO forge_workflow_evidence (
            process_instance_id, story_id, work_type, scout_required, lead_decision,
            qa_review_required, qa_review_passed, qa_passed, failure_class, failed_release_stage,
            last_failure, publish_succeeded, candidate_sha, qa_verified_sha, published_sha
         ) VALUES (
            {pid}::uuid, {sid}, {work}, {scout}, {lead},
            {qrev_req}, {qrev_ok}, {qpass}, {fclass}, {fstage},
            {last}, {pubok}, {cand}, {qasha}, {pubsha}
         )
         ON CONFLICT (process_instance_id) DO UPDATE SET
            work_type = COALESCE(EXCLUDED.work_type, forge_workflow_evidence.work_type),
            scout_required = COALESCE(EXCLUDED.scout_required, forge_workflow_evidence.scout_required),
            lead_decision = COALESCE(EXCLUDED.lead_decision, forge_workflow_evidence.lead_decision),
            qa_review_required = COALESCE(EXCLUDED.qa_review_required, forge_workflow_evidence.qa_review_required),
            qa_review_passed = COALESCE(EXCLUDED.qa_review_passed, forge_workflow_evidence.qa_review_passed),
            qa_passed = COALESCE(EXCLUDED.qa_passed, forge_workflow_evidence.qa_passed),
            failure_class = CASE WHEN {clear} THEN NULL
              ELSE COALESCE(EXCLUDED.failure_class, forge_workflow_evidence.failure_class) END,
            failed_release_stage = CASE WHEN {clear} THEN NULL
              ELSE COALESCE(EXCLUDED.failed_release_stage, forge_workflow_evidence.failed_release_stage) END,
            last_failure = CASE WHEN {clear} THEN NULL
              ELSE COALESCE(EXCLUDED.last_failure, forge_workflow_evidence.last_failure) END,
            publish_succeeded = COALESCE(EXCLUDED.publish_succeeded, forge_workflow_evidence.publish_succeeded),
            candidate_sha = COALESCE(EXCLUDED.candidate_sha, forge_workflow_evidence.candidate_sha),
            qa_verified_sha = COALESCE(EXCLUDED.qa_verified_sha, forge_workflow_evidence.qa_verified_sha),
            published_sha = COALESCE(EXCLUDED.published_sha, forge_workflow_evidence.published_sha),
            updated_at = now()",
        work = opt(&evidence.work_type),
        scout = opt_bool(evidence.scout_required),
        lead = opt(&evidence.lead_decision),
        qrev_req = opt_bool(evidence.qa_review_required),
        qrev_ok = opt_bool(evidence.qa_review_passed),
        qpass = opt_bool(evidence.qa_passed),
        fclass = opt(&evidence.failure_class),
        fstage = opt(&evidence.failed_release_stage),
        last = opt(&evidence.last_failure),
        pubok = opt_bool(evidence.publish_succeeded),
        cand = opt(&evidence.candidate_sha),
        qasha = opt(&evidence.qa_verified_sha),
        pubsha = opt(&evidence.published_sha),
    );
    psql_query(&sql).map(|_| ())
}
