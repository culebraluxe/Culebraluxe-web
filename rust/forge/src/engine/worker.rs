//! Production Forge worker pass.
//!
//! The scheduler enters Rust directly. Persistence is owned by ForgeControlDao;
//! this module contains orchestration and policy only.

use crate::engine::learn::run_learn_pass;
use crate::engine::routing_brain::{parse_forge_routing_brain, ForgeRoutingBrain};
use crate::engine::vendor_session::with_shared;
use db::ForgeControlDao;
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
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            let rows = dao
                .stale_agent_work(stale_after_minutes)
                .await
                .map_err(|error| error.to_string())?;
            let mut recovered = 0u64;
            for row in rows {
                let reason = format!(
                    "stale worker: no heartbeat since {}; process/host presumed terminated",
                    row.updated_at
                );
                let hold =
                    assay_terminal_role(row.role.as_deref()) || row.attempts >= row.max_attempts;
                let failure_code = match row
                    .role
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "reviewer" | "verifier" => "ASSAY_RUNTIME_INTERRUPTED",
                    "builder" => "SMITH_RUNTIME_INTERRUPTED",
                    _ => "HUMAN_DECISION_REQUIRED",
                };

                if let Some(run_id) = row.story_run_id.as_deref() {
                    dao.interrupt_story_run(run_id, failure_code, &reason)
                        .await
                        .map_err(|error| error.to_string())?;
                }

                if hold {
                    dao.hold_stale_work(&row.id, &row.story_id, &reason)
                        .await
                        .map_err(|error| error.to_string())?;
                } else {
                    dao.requeue_stale_work(&row.id, &row.story_id)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                recovered += 1;
            }
            Ok::<u64, String>(recovered)
        })
    })?
}

pub fn fire_due_flights() -> Result<u64, String> {
    with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            let due = dao
                .due_flight_ids()
                .await
                .map_err(|error| error.to_string())?;
            for id in &due {
                let result = dao
                    .fire_flight(id)
                    .await
                    .map_err(|error| error.to_string())?;
                eprintln!(
                    "flight {}: queued={} stamped={} skipped={}",
                    result.batch_id, result.queued, result.stamped, result.skipped
                );
            }
            Ok::<u64, String>(due.len() as u64)
        })
    })?
}

pub fn next_ready_story() -> Result<Option<WorkerDispatch>, String> {
    with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            let row = dao
                .next_ready_work()
                .await
                .map_err(|error| error.to_string())?;
            Ok::<Option<WorkerDispatch>, String>(row.map(|row| WorkerDispatch {
                story_id: row.story_id,
                work_type: work_type_for_kind(row.kind.as_deref()).into(),
            }))
        })
    })?
}

pub fn run_worker_pass() -> Result<i32, String> {
    let brain = parse_forge_routing_brain(std::env::var("FORGE_ROUTING_BRAIN").ok().as_deref());
    if brain == ForgeRoutingBrain::Reducer {
        eprintln!(
            "forge-worker: FORGE_ROUTING_BRAIN=reducer is retired for unattended execution; Rust engine owns this pass"
        );
    }
    let stale = std::env::var("AGENT_WORKER_STALE_AFTER_MINUTES")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(10);

    let recovered = recover_stale_agent_work(stale)?;
    let flights = fire_due_flights()?;
    eprintln!("forge-worker: recovered={recovered} due_flights={flights}");

    // Learning is observational and fail-open: a learn-pass defect must not block dispatch.
    match run_learn_pass(std::path::Path::new("."), stale) {
        Ok(Some(story)) => eprintln!("learn: filed {story}"),
        Ok(None) => {}
        Err(error) => eprintln!("forge-learn-pass-failed: {error}"),
    }

    let Some(dispatch) = next_ready_story()? else {
        println!("no work");
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
        .env(
            "APP_ENV",
            std::env::var("APP_ENV").unwrap_or_else(|_| "production".into()),
        )
        .env(
            "EXECUTION_ENV",
            std::env::var("EXECUTION_ENV").unwrap_or_else(|_| "PROD".into()),
        )
        .status()
        .map_err(|error| format!("launch Rust Forge engine: {error}"))?;

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
