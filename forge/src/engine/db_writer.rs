//! Forge state writer. SQL/persistence lives in db::ForgeEngineDao.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::vendor_session::with_shared;
use crate::engine::writer::{ForgeEvidenceReader, ForgeStateWriter};
use db::ForgeEngineDao;

/// Read evidence for a story. Database errors propagate as `DbFailure` which
/// announces through `db::capture::notify` and lands in `app_error` if a sink
/// is installed. Returns `Err(DbFailure)` on database failure so callers can
/// distinguish "database down" (retryable engine fault) from "no evidence yet"
/// (empty evidence proceeds).
pub fn read_story_evidence(story_id: &str) -> Result<ForgeGateEvidence, db::DbFailure> {
    let result = with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            let row = dao.workflow_evidence_for_story(story_id).await?;
            let counts = dao.story_repair_counts(story_id).await?;
            Ok::<_, db::DbFailure>((row, counts))
        })
    });
    match result {
        Ok(Ok((row, counts))) => {
            let mut evidence = row.map(evidence_from_row).unwrap_or_default();
            // The budget the QA failure route spends: without it every route read 0 attempts and never stopped.
            if let Some((repairs, replans)) = counts {
                evidence.repair_attempts = Some(repairs.max(0) as u32);
                evidence.replan_attempts = Some(replans.max(0) as u32);
            }
            Ok(evidence)
        }
        Ok(Err(db_failure)) => {
            // The DbFailure has already announced itself through db::capture::notify
            // when it was constructed in the DAO. Propagate the error to the caller.
            Err(db_failure)
        }
        Err(string_error) => {
            // with_shared error (e.g., connection failed) — wrap as DbFailure.
            Err(db::DbFailure::configuration(
                "forge_evidence_read",
                format!("connection failed: {string_error}"),
            ))
        }
    }
}

fn evidence_from_row(row: db::ForgeEvidencePatch) -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: row.work_type,
        scout_required: row.scout_required,
        lead_decision: row.lead_decision,
        qa_review_required: row.qa_review_required,
        qa_review_passed: row.qa_review_passed,
        qa_passed: row.qa_passed,
        failure_class: row.failure_class,
        failed_release_stage: row.failed_release_stage,
        last_failure: row.last_failure,
        publish_succeeded: row.publish_succeeded,
        candidate_sha: row.candidate_sha,
        qa_verified_sha: row.qa_verified_sha,
        published_sha: row.published_sha,
        role_output_schema_version: row
            .role_output_schema_version
            .and_then(|version| u32::try_from(version).ok()),
        role_output_diagnostic: row.role_output_diagnostic,
        ..ForgeGateEvidence::default()
    }
}

#[derive(Debug, Default)]
pub struct DbForgeEvidenceReader;

impl ForgeEvidenceReader for DbForgeEvidenceReader {
    fn read(&self, story_id: &str) -> Result<ForgeGateEvidence, db::DbFailure> {
        read_story_evidence(story_id)
    }
}

pub struct DbForgeStateWriter;

impl DbForgeStateWriter {
    pub fn connect_env() -> Result<Self, String> {
        if crate::engine::vendor_session::database_url().is_none() {
            return Err(
                "DATABASE_URL_PROD is not set; Forge writes story state in production only".into(),
            );
        }
        Ok(Self)
    }

    fn run(
        &self,
        f: impl FnOnce(ForgeEngineDao, &tokio::runtime::Runtime) -> Result<(), String>,
    ) -> Result<(), String> {
        with_shared(|db, rt| f(ForgeEngineDao::new(db.clone()), rt))?
    }
}

impl ForgeStateWriter for DbForgeStateWriter {
    fn open_hold(&self, input: &crate::engine::hold::OpenHold) -> Result<String, String> {
        // One implementation of the row, in `hold`, reached through the same port as every other write.
        crate::engine::hold::open_forge_hold_record(input)
    }

    fn mark_story_in_progress(&self, story_id: &str) -> Result<(), String> {
        self.run(|dao, rt| {
            rt.block_on(async {
                dao.mark_story_in_progress(story_id)
                    .await
                    .map_err(|e| e.to_string())
            })
        })
    }

    fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> Result<(), String> {
        self.run(|dao, rt| {
            rt.block_on(async {
                dao.mark_story_human_hold(story_id, reason)
                    .await
                    .map_err(|e| e.to_string())
            })
        })
    }

    fn mark_story_complete(&self, story_id: &str) -> Result<(), String> {
        self.run(|dao, rt| {
            rt.block_on(async {
                dao.mark_story_complete(story_id)
                    .await
                    .map_err(|e| e.to_string())
            })
        })
    }

    fn stamp_run_candidate(&self, run_id: &str, candidate_sha: &str) -> Result<(), String> {
        self.run(|dao, rt| {
            rt.block_on(async {
                dao.stamp_run_candidate(run_id, candidate_sha)
                    .await
                    .map_err(|e| e.to_string())
            })
        })
    }

    fn record_run_usage(
        &self,
        run_id: &str,
        usage: &crate::engine::harness::HarnessUsage,
    ) -> Result<(), String> {
        self.run(|dao, rt| {
            rt.block_on(async {
                dao.add_run_usage(
                    run_id,
                    usage.tokens_input,
                    usage.tokens_output,
                    usage.cost_usd,
                )
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
            })
        })
    }

    fn settle_model_attempt_usage(
        &self,
        run_id: &str,
        generation_id: &str,
        attempt_key: &str,
        usage: Option<&crate::engine::harness::HarnessUsage>,
    ) -> Result<(), String> {
        self.run(|dao, rt| {
            rt.block_on(async {
                dao.settle_model_generation_attempt_usage(
                    generation_id,
                    attempt_key,
                    run_id,
                    usage.map(|reading| {
                        (
                            reading.session_id.as_str(),
                            reading.tokens_input,
                            reading.tokens_output,
                            reading.cost_usd,
                        )
                    }),
                )
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
            })
        })
    }

    fn append_run_detail(&self, run_id: &str, detail: &str) -> Result<(), String> {
        self.run(|dao, rt| {
            rt.block_on(async {
                dao.append_run_detail(run_id, detail)
                    .await
                    .map_err(|e| e.to_string())
            })
        })
    }

    fn record_tool_artifact(&self, input: &db::NewToolArtifact) -> Result<Option<String>, String> {
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.record_tool_artifact(input)
                    .await
                    .map(|row| Some(row.id))
                    .map_err(|e| e.to_string())
            })
        })?
    }

    fn read_assay_plan_snapshot(
        &self,
        run_id: &str,
    ) -> Result<Option<db::forge_assay::AssayPlanSnapshotRow>, String> {
        with_shared(|db, rt| {
            let dao = ::db::forge_assay::ForgeAssayDao::new(db.clone());
            rt.block_on(async {
                dao.plan_snapshot_for_run(run_id)
                    .await
                    .map_err(|error| error.to_string())
            })
        })?
    }

    fn read_assay_receipt(
        &self,
        key: &str,
    ) -> Result<Option<::db::forge_assay::AssayReceiptRow>, String> {
        with_shared(|db, rt| {
            let dao = ::db::forge_assay::ForgeAssayDao::new(db.clone());
            rt.block_on(async {
                dao.receipt_for_key(key)
                    .await
                    .map_err(|error| error.to_string())
            })
        })?
    }
}
