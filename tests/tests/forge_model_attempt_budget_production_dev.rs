//! Slice 5 smoke: the production registry/job composition and durable attempt authority on DEV.
//!
//! Run explicitly after migrations 282/283:
//!   cargo test -p test-harness --test forge_model_attempt_budget_production_dev -- --ignored

#[path = "support/forge_seam.rs"]
mod support;

use db::{Database, DbTarget};
use forge::engine::db_writer::DbForgeEvidenceReader;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DurableForgeExecution,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::WorkflowJobService;
use forge::engine::runner::ProductionRoleRunner;
use forge::engine::runtime::ForgeRuntime;
use forge::engine::turn_budget::{DbModelAttemptControl, MODEL_TURN_CAP_CODE};
use forge::engine::{durable_completion_ledger, forge_sdlc_definition, DbForgeStateWriter};
use forge::roles::ForgeLaneServices;
use std::sync::Arc;
use workflow::NeonStore;

#[test]
#[ignore = "needs DATABASE_URL_DEV and migrations 282/283"]
fn registered_durable_composition_enforces_the_shared_generation_cap() {
    let tokio = tokio::runtime::Runtime::new().expect("test runtime");
    let database = tokio
        .block_on(Database::connect_target(DbTarget::Dev))
        .expect("DATABASE_URL_DEV");
    assert!(
        db::shared::install(database.clone()),
        "the isolated integration-test process owns the shared DEV pool"
    );
    let pool = database.pool();
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-COMPOSITION-{tag}");

    tokio.block_on(async {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status, notes) \
             values ($1, 'PROOF', 'Durable Forge composition proof', 'High', 'Planned', '')",
        )
        .bind(&story)
        .execute(pool)
        .await
        .expect("insert proof story");
    });
    let run_id: String = tokio.block_on(async {
        sqlx::query_scalar(
            "insert into storyboard_story_run (story_id, started_at, execution_environment, run_type) \
             values ($1, now(), 'DEV', 'dispatch') returning id::text",
        )
        .bind(&story)
        .fetch_one(pool)
        .await
        .expect("insert proof generation")
    });

    let writer = Arc::new(DbForgeStateWriter);
    let runtime = ForgeRuntime::from_store(
        NeonStore::from_database(database.clone()).expect("Neon workflow store"),
        writer.clone(),
        None,
        Some(Arc::new(DbForgeEvidenceReader)),
        durable_completion_ledger(),
        forge_sdlc_definition(),
    )
    .expect("production-shaped durable Forge runtime");
    let attempt_control = Arc::new(
        DbModelAttemptControl::initialize(run_id.clone(), 1)
            .expect("durable attempt budget for this generation"),
    );
    let harness = Arc::new(support::SeamHarness::default());
    let runner = ProductionRoleRunner::new(harness.clone(), ForgeGateEvidence::default())
        .with_writer(writer)
        .with_story_run(Some(run_id.clone()))
        .with_model_attempt_control(attempt_control);
    let services = ForgeLaneServices::new(&runner);
    let registry = services.registry().expect("production role registry");
    let jobs = WorkflowJobService::new(runtime.engine());

    let options = || DriveForgeStoryOptions {
        work_type: "FEATURE",
        evidence: ForgeGateEvidence::default(),
        runner: None,
        max_steps: 1,
        worker_id: "forge-b2-slice5-smoke",
        within_story_concurrency: 1,
        stop_after: None,
        turn_cap: 1,
    };
    let durable = || DurableForgeExecution {
        jobs: &jobs,
        registry: &registry,
    };

    let first = drive_forge_story_with_jobs(&runtime, &story, options(), durable())
        .expect("the first registered durable role turn runs");
    assert_eq!(first.status, "Active");
    assert_eq!(harness.calls().len(), 1, "one fake model turn was launched");

    let budget: (i32, i32) = tokio.block_on(async {
        sqlx::query_as(
            "select used, cap from forge_model_attempt_budget where story_run_id = $1::uuid",
        )
        .bind(&run_id)
        .fetch_one(pool)
        .await
        .expect("durable generation budget")
    });
    assert_eq!(budget, (1, 1));

    let second = drive_forge_story_with_jobs(&runtime, &story, options(), durable())
        .expect("cap refusal is an observable story hold");
    assert!(second.needs_human);
    let stop = second
        .blocked_reason
        .as_deref()
        .expect("operator stop reason");
    assert!(stop.contains(MODEL_TURN_CAP_CODE), "{stop}");
    assert!(stop.contains("no role verdict was accepted"), "{stop}");
    assert_eq!(
        harness.calls().len(),
        1,
        "the refused task never reached the fake harness"
    );

    tokio.block_on(async {
        let after: (i32, i32) = sqlx::query_as(
            "select used, cap from forge_model_attempt_budget where story_run_id = $1::uuid",
        )
        .bind(&run_id)
        .fetch_one(pool)
        .await
        .expect("budget after refused launch");
        assert_eq!(after, (1, 1), "refusal does not consume another allowance");
        sqlx::query(
            "delete from process_instances where subject_type = 'story' and subject_id = $1",
        )
        .bind(&story)
        .execute(pool)
        .await
        .expect("remove proof workflow instance");
        sqlx::query("delete from storyboard_story where id = $1")
            .bind(&story)
            .execute(pool)
            .await
            .expect("remove proof story and generation");
    });
}
