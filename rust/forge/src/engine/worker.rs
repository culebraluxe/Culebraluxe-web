//! Production Forge worker pass.
//!
//! Replaces scripts/agent-work-entry.ts. The scheduler enters Rust directly:
//! stale runtime recovery -> due Flight fire -> oldest Ready story -> Rust Forge engine.
//! No legacy/workflow_app, agent-runtime, or TypeScript execution path is involved.

use crate::engine::vendor_session::with_shared;
use sqlx::Row;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct WorkerDispatch {
    pub story_id: String,
    pub work_type: String,
}

fn work_type_for_kind(kind: Option<&str>) -> &'static str {
    match kind.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "fix" => "BUG",
        "qa" | "learn" => "RESEARCH",
        _ => "FEATURE",
    }
}

fn assay_terminal_role(role: Option<&str>) -> bool {
    matches!(
        role.unwrap_or("").trim().to_ascii_lowercase().as_str(),
        "reviewer" | "verifier"
    )
}

pub fn recover_stale_agent_work(stale_after_minutes: i64) -> Result<u64, String> {
    with_shared(|db, rt| {
        rt.block_on(async {
            let rows = sqlx::query(
                "select id::text, story_id, role, attempts, max_attempts, story_run_id::text, updated_at::text
                 from agent_work_item
                 where state in ('Claimed','Running','Paused')
                   and updated_at < now() - ($1::text || ' minutes')::interval
                 order by updated_at asc",
            )
            .bind(stale_after_minutes.max(0).to_string())
            .fetch_all(db.pool())
            .await
            .map_err(|e| e.to_string())?;

            let mut recovered = 0u64;
            for row in rows {
                let id: String = row.try_get(0).map_err(|e| e.to_string())?;
                let story: String = row.try_get(1).map_err(|e| e.to_string())?;
                let role: Option<String> = row.try_get(2).ok();
                let attempts: i32 = row.try_get(3).unwrap_or(0);
                let max_attempts: i32 = row.try_get(4).unwrap_or(3);
                let run_id: Option<String> = row.try_get(5).ok();
                let updated_at: String = row.try_get(6).unwrap_or_else(|_| "unknown".into());
                let reason = format!("stale worker: no heartbeat since {updated_at}; process/host presumed terminated");
                let hold = assay_terminal_role(role.as_deref()) || attempts >= max_attempts;
                let failure_code = match role.as_deref().unwrap_or("").to_ascii_lowercase().as_str() {
                    "reviewer" | "verifier" => "ASSAY_RUNTIME_INTERRUPTED",
                    "builder" => "SMITH_RUNTIME_INTERRUPTED",
                    _ => "HUMAN_DECISION_REQUIRED",
                };

                if let Some(run) = run_id.as_deref() {
                    sqlx::query(
                        "update storyboard_story_run
                         set ended_at=now(), result_status='Interrupted', failure_code=$2,
                             notes=case when notes is null or notes='' then $3 else notes || E'\\n' || $3 end,
                             updated_at=now()
                         where id=$1::uuid and ended_at is null",
                    )
                    .bind(run)
                    .bind(failure_code)
                    .bind(&reason)
                    .execute(db.pool())
                    .await
                    .map_err(|e| e.to_string())?;
                }

                if hold {
                    sqlx::query(
                        "update agent_work_item set state='Error', error_text=$2, finished_at=now(), updated_at=now()
                         where id=$1::uuid and state in ('Claimed','Running','Paused')",
                    )
                    .bind(&id)
                    .bind(&reason)
                    .execute(db.pool())
                    .await
                    .map_err(|e| e.to_string())?;
                    sqlx::query(
                        "update storyboard_story set status='Hold', completed_at=null, updated_at=now() where id=$1",
                    )
                    .bind(&story)
                    .execute(db.pool())
                    .await
                    .map_err(|e| e.to_string())?;
                } else {
                    sqlx::query(
                        "update agent_work_item
                         set state='Ready', queued_at=now(), claimed_at=null, claimed_by=null,
                             started_at=null, finished_at=null, error_text=null, runtime_adapter=null,
                             external_run_id=null, updated_at=now()
                         where id=$1::uuid and state in ('Claimed','Running','Paused')",
                    )
                    .bind(&id)
                    .execute(db.pool())
                    .await
                    .map_err(|e| e.to_string())?;
                    sqlx::query(
                        "update storyboard_story set status='Ready', completed_at=null, updated_at=now() where id=$1",
                    )
                    .bind(&story)
                    .execute(db.pool())
                    .await
                    .map_err(|e| e.to_string())?;
                }
                recovered += 1;
            }
            Ok::<u64, String>(recovered)
        })
    })?
}

async fn fire_batch(db: &db::Database, batch_id: &str) -> Result<(u64, u64), String> {
    let policy: Option<String> = sqlx::query_scalar(
        "select model_policy from forge_batch where id=$1::uuid",
    )
    .bind(batch_id)
    .fetch_optional(db.pool())
    .await
    .map_err(|e| e.to_string())?
    .flatten();
    let policy = policy.unwrap_or_else(|| "cheap".into());

    let rows = sqlx::query(
        "select story_id, coalesce(kind,'normal') as kind
         from forge_batch_item where batch_id=$1::uuid and state='Staged' order by story_id",
    )
    .bind(batch_id)
    .fetch_all(db.pool())
    .await
    .map_err(|e| e.to_string())?;

    let mut queued = 0u64;
    let mut stamped = 0u64;
    for row in rows {
        let story: String = row.try_get("story_id").map_err(|e| e.to_string())?;
        let kind: String = row.try_get("kind").unwrap_or_else(|_| "normal".into());

        sqlx::query(
            "update storyboard_story set status='Ready', updated_at=now() where id=$1",
        )
        .bind(&story)
        .execute(db.pool())
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "update forge_batch_item set state='Queued', queued_at=now(), error_text=null
             where batch_id=$1::uuid and story_id=$2",
        )
        .bind(batch_id)
        .bind(&story)
        .execute(db.pool())
        .await
        .map_err(|e| e.to_string())?;

        stamped += sqlx::query(
            "update agent_work_item set kind=$2, model_policy=$3, updated_at=now()
             where story_id=$1 and state='Ready'",
        )
        .bind(&story)
        .bind(&kind)
        .bind(&policy)
        .execute(db.pool())
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
        queued += 1;
    }

    sqlx::query(
        "update forge_batch set status='Fired', fired_at=now() where id=$1::uuid and status<>'Fired'",
    )
    .bind(batch_id)
    .execute(db.pool())
    .await
    .map_err(|e| e.to_string())?;
    Ok((queued, stamped))
}

pub fn fire_due_flights() -> Result<u64, String> {
    with_shared(|db, rt| {
        rt.block_on(async {
            let due: Vec<String> = sqlx::query_scalar(
                "select id::text from forge_batch
                 where status='Scheduled' and scheduled_for is not null and scheduled_for <= now()
                 order by scheduled_for asc",
            )
            .fetch_all(db.pool())
            .await
            .map_err(|e| e.to_string())?;
            for id in &due {
                let (queued, stamped) = fire_batch(db, id).await?;
                eprintln!("flight {id}: queued={queued} stamped={stamped}");
            }
            Ok::<u64, String>(due.len() as u64)
        })
    })?
}

pub fn next_ready_story() -> Result<Option<WorkerDispatch>, String> {
    with_shared(|db, rt| {
        rt.block_on(async {
            let row = sqlx::query(
                "select story_id, kind
                 from agent_work_item
                 where state='Ready'
                 order by queued_at asc nulls last, story_id asc
                 limit 1",
            )
            .fetch_optional(db.pool())
            .await
            .map_err(|e| e.to_string())?;
            let Some(row) = row else { return Ok(None) };
            let story_id: String = row.try_get("story_id").map_err(|e| e.to_string())?;
            let kind: Option<String> = row.try_get("kind").ok();
            Ok::<Option<WorkerDispatch>, String>(Some(WorkerDispatch {
                story_id,
                work_type: work_type_for_kind(kind.as_deref()).into(),
            }))
        })
    })?
}

pub fn run_worker_pass() -> Result<i32, String> {
    let stale = std::env::var("AGENT_WORKER_STALE_AFTER_MINUTES")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(10);
    let recovered = recover_stale_agent_work(stale)?;
    let flights = fire_due_flights()?;
    eprintln!("forge-worker: recovered={recovered} due_flights={flights}");

    let Some(dispatch) = next_ready_story()? else {
        eprintln!("forge-worker: idle (no Ready work item)");
        return Ok(0);
    };
    eprintln!(
        "forge-worker: story={} work_type={}",
        dispatch.story_id, dispatch.work_type
    );

    let status = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "rust/Cargo.toml",
            "-p",
            "forge",
            "--bin",
            "forge",
            "--",
            "--story",
            &dispatch.story_id,
            "--work-type",
            &dispatch.work_type,
        ])
        .env("APP_ENV", std::env::var("APP_ENV").unwrap_or_else(|_| "production".into()))
        .env(
            "EXECUTION_ENV",
            std::env::var("EXECUTION_ENV").unwrap_or_else(|_| "PROD".into()),
        )
        .status()
        .map_err(|e| format!("launch Rust Forge engine: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_batch_kind_to_engine_work_type() {
        assert_eq!(work_type_for_kind(Some("fix")), "BUG");
        assert_eq!(work_type_for_kind(Some("qa")), "RESEARCH");
        assert_eq!(work_type_for_kind(Some("learn")), "RESEARCH");
        assert_eq!(work_type_for_kind(Some("normal")), "FEATURE");
    }

    #[test]
    fn assay_roles_are_terminal_recovery_roles() {
        assert!(assay_terminal_role(Some("reviewer")));
        assert!(assay_terminal_role(Some("verifier")));
        assert!(!assay_terminal_role(Some("builder")));
    }
}
