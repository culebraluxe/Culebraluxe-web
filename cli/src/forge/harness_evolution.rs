//! `forge harness-evolution` — the operator surface for the Harness Evolution subsystem.
//!
//! The status views are read-only: they answer which harness revision is active per domain, what
//! revisions exist, what experiments ran, and what lessons are retrievable. `probe` is the one
//! explicit writer this command grew (O2): it runs a historical task through the command sandbox
//! and records the probe/contrast/candidate/experiment rows through the durable store, with
//! `--dry-run` doing the read-only plan. Adoption and rollback stay in the subsystem; this command
//! never adopts, never rolls back, and never grants authority.
//!
//! Usage:
//!   cargo run -p cli -- forge harness-evolution status [--format json]
//!   cargo run -p cli -- forge harness-evolution revisions [--domain D] [--format json]
//!   cargo run -p cli -- forge harness-evolution experiments --domain D [--format json]
//!   cargo run -p cli -- forge harness-evolution experience --domain D [--executor E]
//!       [--mode parallel|sequential] [--seed N] [--limit N] [--format json]
//!   cargo run -p cli -- forge harness-evolution feed [--days N] [--limit N]
//!       [--input FILE] [--target dev|prod] [--format json]
//!   cargo run -p cli -- forge harness-evolution metrics [--limit N]
//!       [--input FILE] [--target dev|prod] [--format json]
//!   cargo run -p cli -- forge harness-evolution calibration [--input FILE] [--format json]
//!   cargo run -p cli -- forge harness-evolution probe --domain D --story S [--run R]
//!       --base-revision ID --candidate-revision ID [--candidate-kind invariant|domain]
//!       [--environment development|sandbox|ci|staging] [--command ABS] [--arg A]...
//!       [--model M] [--model-policy P] [--executor E] [--tool T]... [--dataset D]
//!       [--test-plan P] [--replay-id R] [--mode parallel|sequential] [--k N] [--seed N]
//!       [--target dev|prod] [--dry-run] [--format json]
//!   cargo run -p cli -- forge harness-evolution probe ... --authorize-model-run
//!       [--model-backend dsh|opencode]
//!       (model-turn sandbox instead of --command; mutually exclusive with --command, and a real
//!        run spends a paid model turn — explicit authorization is required)

use std::path::PathBuf;

use super::{connect, Failure};
use db::forge_read::{ScorecardHoldCountRow, ScorecardRunRow};
use db::{
    Database, DbTarget, ForgeCorrectionsDao, ForgeEngineDao, ForgeHarnessDao,
    ForgeHarnessRevisionRow, ForgeReadDao, TurnTrajectoryRow,
};
use forge::engine::harness_evolution::calibration::{
    calibration_report, CalibrationReport, CalibrationSample,
};
use forge::engine::harness_evolution::feed::{
    correction_feed_rows, targets_from_hypotheses, targets_from_learning_sources,
    CorrectionFeedRow, FeedOptions, HoldFeedRow, HypothesisFeedRow, ScorecardFeedRow,
};
use forge::engine::harness_evolution::metrics::{
    role_model_metrics, RoleModelMetric, RoleModelObservation, RunOutcome,
};
use forge::engine::harness_evolution::model::{
    AdoptionVerdict, EnvironmentClass, EvolutionBudget, EvolutionMode, HarnessDomain, ProposalKind,
};
use forge::engine::harness_evolution::model_turn_sandbox::{
    ModelTurnAuthorization, ModelTurnBackend, ModelTurnSandbox,
};
use forge::engine::harness_evolution::probe::ProbePins;
use forge::engine::harness_evolution::probe_replay::{
    environment_class_for, plan_recorded_replay, run_recorded_replay, HistoricalTask,
    HistoricalTaskLoader, HistoricalTaskRequest, MapRevisionSource, RecordedReplayRequest,
    ReplayError,
};
use forge::engine::harness_evolution::sandbox_command::{CommandSandbox, SandboxCommand};
use forge::engine::harness_evolution::store_db::{
    plan_replay_persistence, stored_revision, ReplayPersistenceMeta,
};
use forge::scorecard::{summarize_scorecard, ScorecardHold, ScorecardRun};
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    crate::apple_sync::load_env();
    if args.iter().any(|value| value == "--help") {
        usage();
        return Ok(0);
    }
    let command = args.first().map(String::as_str).unwrap_or("status");
    let json_out = wants_json(args);
    // O1 operator views: `feed`/`metrics` may render a supplied fixture without a database, and
    // `calibration` has no durable sample read path yet, so they dispatch before the pool opens.
    match command {
        "feed" => return run_feed(args).await,
        "metrics" => return run_metrics(args).await,
        "calibration" => return run_calibration(args).await,
        // `probe` needs a target-aware database (`--target`), so it dispatches before the default
        // pool opens and owns its own connection — same shape as feed/metrics.
        "probe" => return run_probe(args).await,
        _ => {}
    }
    let database = connect().await?;
    let target = database.declared_target().as_str().to_string();
    let dao = ForgeHarnessDao::new(database);

    match command {
        "status" => {
            let revisions = dao
                .list_all_revisions()
                .await
                .map_err(|error| Failure::failed(format!("cannot list revisions: {error}")))?;
            let mut domains: Vec<String> =
                revisions.iter().map(|row| row.domain.clone()).collect();
            domains.sort();
            domains.dedup();
            let mut active = Vec::new();
            for domain in &domains {
                let activation = dao
                    .current_activation(domain)
                    .await
                    .map_err(|error| Failure::failed(format!("cannot read activation: {error}")))?;
                active.push(json!({
                    "domain": domain,
                    "revision_id": activation.as_ref().map(|row| row.to_revision_id.clone()),
                    "activation_kind": activation.as_ref().map(|row| row.activation_kind.clone()),
                    "activated_at": activation.as_ref().map(|row| row.activated_at),
                    "activated_by": activation.as_ref().map(|row| row.activated_by.clone()),
                }));
            }
            if json_out {
                print_json(&json!({
                    "target": target,
                    "read_only": true,
                    "active": active,
                    "revision_count": revisions.len(),
                    "revisions": revisions.iter().map(revision_json).collect::<Vec<_>>(),
                }))?;
            } else {
                println!("forge harness-evolution status — target={target} (read-only)");
                println!("REVISIONS {}", revisions.len());
                if active.is_empty() {
                    println!("ACTIVE    none (no harness domain has an active revision)");
                } else {
                    for item in &active {
                        let revision = item.get("revision_id").and_then(Value::as_str);
                        println!(
                            "ACTIVE    {}  {}",
                            item.get("domain").and_then(Value::as_str).unwrap_or("?"),
                            revision.unwrap_or("none")
                        );
                    }
                }
            }
            Ok(0)
        }
        "revisions" => {
            let domain = flag(args, "--domain");
            let rows = match domain.as_deref() {
                Some(domain) => dao.list_revisions(domain).await,
                None => dao.list_all_revisions().await,
            }
            .map_err(|error| Failure::failed(format!("cannot list revisions: {error}")))?;
            if json_out {
                print_json(&json!({
                    "target": target,
                    "read_only": true,
                    "revisions": rows.iter().map(revision_json).collect::<Vec<_>>(),
                }))?;
            } else {
                println!("forge harness-evolution revisions — target={target} (read-only)");
                for row in &rows {
                    println!(
                        "  {}  {}  {}  {}  {}",
                        row.revision_id, row.domain, row.status, short(&row.manifest_hash, 12), row.created_by
                    );
                }
                if rows.is_empty() {
                    println!("  no revisions");
                }
            }
            Ok(0)
        }
        "experiments" => {
            let domain = required_flag(args, "--domain")?;
            let rows = dao
                .list_experiments(&domain)
                .await
                .map_err(|error| Failure::failed(format!("cannot list experiments: {error}")))?;
            if json_out {
                print_json(&json!({
                    "target": target,
                    "read_only": true,
                    "experiments": rows.iter().map(|row| json!({
                        "experiment_id": row.experiment_id,
                        "domain": row.domain,
                        "base_revision_id": row.base_revision_id,
                        "mode": row.mode,
                        "k": row.k,
                        "state": row.state,
                        "successor_revision_id": row.successor_revision_id,
                        "created_at": row.created_at,
                    })).collect::<Vec<_>>(),
                }))?;
            } else {
                println!("forge harness-evolution experiments — domain={domain} target={target} (read-only)");
                for row in &rows {
                    println!(
                        "  {}  {}  k={}  base={}  successor={}",
                        row.experiment_id,
                        row.state,
                        row.k,
                        row.base_revision_id,
                        row.successor_revision_id.as_deref().unwrap_or("none")
                    );
                }
                if rows.is_empty() {
                    println!("  no experiments");
                }
            }
            Ok(0)
        }
        "experience" => {
            let domain = required_flag(args, "--domain")?;
            let executor = flag(args, "--executor").unwrap_or_else(|| "opencode".to_string());
            let mode = flag(args, "--mode").unwrap_or_else(|| "parallel".to_string());
            let seed = flag(args, "--seed")
                .map(|value| value.parse::<i64>())
                .transpose()
                .map_err(|_| Failure::failed("--seed must be an integer"))?
                .unwrap_or(0);
            let limit = flag(args, "--limit")
                .map(|value| value.parse::<i64>())
                .transpose()
                .map_err(|_| Failure::failed("--limit must be an integer"))?
                .unwrap_or(20);
            let rows = dao
                .list_experiences(&domain, &executor, &mode, seed, limit)
                .await
                .map_err(|error| Failure::failed(format!("cannot list experience: {error}")))?;
            if json_out {
                print_json(&json!({
                    "target": target,
                    "read_only": true,
                    "scope": {"domain": domain, "executor": executor, "mode": mode, "seed": seed},
                    "experiences": rows.iter().map(|row| json!({
                        "experience_id": row.experience_id,
                        "lesson": row.lesson,
                        "confidence": row.confidence,
                        "manifest_hash": row.manifest_hash,
                        "created_at": row.created_at,
                    })).collect::<Vec<_>>(),
                }))?;
            } else {
                println!(
                    "forge harness-evolution experience — domain={domain} executor={executor} mode={mode} seed={seed} target={target} (read-only)"
                );
                for row in &rows {
                    println!(
                        "  {}  confidence {}  {}",
                        row.experience_id,
                        row.confidence,
                        short(&row.lesson, 100)
                    );
                }
                if rows.is_empty() {
                    println!("  no experience in this scope");
                }
            }
            Ok(0)
        }
        other => Err(Failure::failed(format!(
            "unknown harness-evolution command `{other}`; try status|revisions|experiments|experience|feed|metrics|calibration"
        ))),
    }
}

pub fn usage() {
    eprintln!("usage: cargo run -p cli -- forge harness-evolution <status|revisions|experiments|experience|feed|metrics|calibration|probe> [options]");
    eprintln!();
    eprintln!("  Read-only operator surface for Harness Evolution (migration 294 tables).");
    eprintln!("  status      active harness revision per domain + revision count");
    eprintln!("  revisions   [--domain D] immutable revisions");
    eprintln!("  experiments --domain D  propose/probe/compose experiments");
    eprintln!("  experience  --domain D [--executor E] [--mode parallel|sequential] [--seed N] [--limit N]");
    eprintln!("  feed        [--days N] [--limit N] [--input FILE] [--target dev|prod]");
    eprintln!("              Phase 3 improvement feed: scorecard/HOLD observations, repeated");
    eprintln!("              corrections and learn hypotheses, plus the evidence-linked targets a");
    eprintln!(
        "              proposer would consume. Rows without durable evidence are observations,"
    );
    eprintln!("              never targets.");
    eprintln!("  metrics     [--limit N] [--input FILE] [--target dev|prod]");
    eprintln!("              Per (role, model) attempt metrics from recorded trajectory turns;");
    eprintln!(
        "              an uncaptured rate/cost prints null in JSON and NOT CAPTURED in text."
    );
    eprintln!("  calibration [--input FILE]");
    eprintln!("              Blended decision-calibration (Brier) report from supplied samples;");
    eprintln!(
        "              no durable sample read path exists yet, so it says NOT CAPTURED rather"
    );
    eprintln!("              than inventing a score.");
    eprintln!(
        "  probe       --domain D --story S [--run R] --base-revision ID --candidate-revision ID"
    );
    eprintln!("              [--candidate-kind invariant|domain] [--environment development|sandbox|ci|staging]");
    eprintln!(
        "              [--command ABS] [--arg A]... [--model M] [--model-policy P] [--executor E]"
    );
    eprintln!("              [--tool T]... [--dataset D] [--test-plan P] [--replay-id R]");
    eprintln!("              [--mode parallel|sequential] [--k N] [--seed N] [--target dev|prod]");
    eprintln!("              [--dry-run]");
    eprintln!("              Records a real replay probe/contrast through the durable store:");
    eprintln!(
        "              loads one historical story/run, runs the command sandbox, and inserts"
    );
    eprintln!("              the experiment/candidate/probe/contrast rows. Production is refused;");
    eprintln!(
        "              --dry-run loads and plans only (no sandbox, no writes). --command must"
    );
    eprintln!(
        "              be an allowlisted absolute program; without it the probe fails closed."
    );
    eprintln!("  feed/metrics use the database named by --target, else APP_ENV; --input renders a");
    eprintln!("  supplied fixture without a database. calibration is fixture-only for now.");
    eprintln!("  All: [--format json].");
}

// ---------------------------------------------------------------------------------------------
// O1 operator views: feed / metrics / calibration
//
// All three are read-only. `feed` and `metrics` prefer a durable read path and accept `--input
// <file>` to render supplied rows without a database; `calibration` has no durable sample read
// path yet, so it renders the pure report from `--input` and says NOT CAPTURED otherwise.
// ---------------------------------------------------------------------------------------------

/// The output format every new operator view shares. The older subcommands keep their historical
/// `--format json` behaviour; the new ones refuse an unknown format instead of silently printing text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

fn parse_format(args: &[String]) -> Result<OutputFormat, Failure> {
    match flag(args, "--format").as_deref() {
        None | Some("text") => Ok(OutputFormat::Text),
        Some("json") => Ok(OutputFormat::Json),
        Some(other) => Err(Failure::usage(format!(
            "unknown --format `{other}`; expected text or json"
        ))),
    }
}

/// `--target dev|prod` (also `--target=…` when written as two arguments). Absent means the process
/// pool the rest of the CLI uses; an unknown value is a usage error, never a guessed database.
fn parse_target(args: &[String]) -> Result<Option<DbTarget>, Failure> {
    match flag(args, "--target").as_deref() {
        None => Ok(None),
        Some("dev") | Some("development") => Ok(Some(DbTarget::Dev)),
        Some("prod") | Some("production") => Ok(Some(DbTarget::Prod)),
        Some(other) => Err(Failure::usage(format!(
            "unknown --target `{other}`; expected dev or prod"
        ))),
    }
}

async fn database_for(args: &[String]) -> Result<Database, Failure> {
    match parse_target(args)? {
        Some(target) => Database::connect_target(target)
            .await
            .map_err(|error| Failure::failed(format!("cannot connect to {target:?}: {error}"))),
        None => connect().await,
    }
}

fn parse_limit(args: &[String], default: i64, max: i64) -> Result<i64, Failure> {
    match flag(args, "--limit") {
        None => Ok(default),
        Some(value) => value
            .parse::<i64>()
            .map(|limit| limit.clamp(1, max))
            .map_err(|_| Failure::usage("--limit must be an integer")),
    }
}

fn input_path(args: &[String]) -> Option<PathBuf> {
    flag(args, "--input").map(PathBuf::from)
}

fn read_input(path: &PathBuf) -> Result<String, Failure> {
    std::fs::read_to_string(path)
        .map_err(|error| Failure::failed(format!("cannot read {}: {error}", path.display())))
}

// ---- feed -----------------------------------------------------------------------------------

/// The rows one feed view renders. Kept as the pure module types so the CLI adds no second
/// interpretation of a scorecard bucket, a HOLD group or a repeated correction.
struct FeedData {
    source: String,
    target: String,
    days: i64,
    scorecard: Vec<ScorecardFeedRow>,
    holds: Vec<HoldFeedRow>,
    corrections: Vec<CorrectionFeedRow>,
    hypotheses: Vec<HypothesisFeedRow>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedFixture {
    #[serde(default)]
    scorecard: Vec<ScorecardFixtureRow>,
    #[serde(default)]
    holds: Vec<HoldFixtureRow>,
    #[serde(default)]
    corrections: Vec<CorrectionFixtureRow>,
    #[serde(default)]
    hypotheses: Vec<HypothesisFixtureRow>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScorecardFixtureRow {
    role: String,
    agent: String,
    tasks: u64,
    completed: u64,
    held_or_escalated: u64,
    failed_or_interrupted: u64,
    rework: u64,
    #[serde(default)]
    mean_wall_minutes: Option<f64>,
    #[serde(default)]
    evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HoldFixtureRow {
    node: String,
    failure_class: String,
    holds: u32,
    #[serde(default)]
    evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorrectionFixtureRow {
    lane: String,
    correction: String,
    occurrences: u32,
    #[serde(default)]
    evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HypothesisFixtureRow {
    repository_key: String,
    finding_key: String,
    occurrence: u32,
    #[serde(default)]
    last_story_id: Option<String>,
    #[serde(default)]
    evidence_refs: Vec<String>,
}

async fn run_feed(args: &[String]) -> Result<u8, Failure> {
    let format = parse_format(args)?;
    let limit = parse_limit(args, 20, 200)?;
    let options = FeedOptions {
        max_targets: usize::try_from(limit).unwrap_or(usize::MAX),
        ..FeedOptions::default()
    };
    let data = match input_path(args) {
        Some(path) => load_feed_fixture(&path, args)?,
        None => load_feed_database(args, limit).await?,
    };
    match format {
        OutputFormat::Json => {
            print_json(&feed_json(&data, options))?;
            Ok(0)
        }
        OutputFormat::Text => {
            print!("{}", render_feed(&data, options));
            Ok(0)
        }
    }
}

fn load_feed_fixture(path: &PathBuf, args: &[String]) -> Result<FeedData, Failure> {
    let raw = read_input(path)?;
    let fixture: FeedFixture = serde_json::from_str(&raw).map_err(|error| {
        Failure::failed(format!(
            "cannot parse feed input {}: {error}",
            path.display()
        ))
    })?;
    Ok(feed_data_from_fixture(
        fixture,
        forge::scorecard::parse_window_days(args),
    ))
}

fn feed_data_from_fixture(fixture: FeedFixture, days: i64) -> FeedData {
    FeedData {
        source: "input".to_string(),
        target: "input".to_string(),
        days,
        scorecard: fixture
            .scorecard
            .into_iter()
            .map(|row| ScorecardFeedRow {
                role: row.role,
                agent: row.agent,
                tasks: row.tasks,
                completed: row.completed,
                held_or_escalated: row.held_or_escalated,
                failed_or_interrupted: row.failed_or_interrupted,
                rework: row.rework,
                mean_wall_minutes: row.mean_wall_minutes,
                evidence_refs: row.evidence_refs,
            })
            .collect(),
        holds: fixture
            .holds
            .into_iter()
            .map(|row| HoldFeedRow {
                node: row.node,
                failure_class: row.failure_class,
                holds: row.holds,
                evidence_refs: row.evidence_refs,
            })
            .collect(),
        corrections: fixture
            .corrections
            .into_iter()
            .map(|row| CorrectionFeedRow {
                lane: row.lane,
                correction: row.correction,
                occurrences: row.occurrences,
                evidence_refs: row.evidence_refs,
            })
            .collect(),
        hypotheses: fixture
            .hypotheses
            .into_iter()
            .map(|row| HypothesisFeedRow {
                repository_key: row.repository_key,
                finding_key: row.finding_key,
                occurrence: row.occurrence,
                last_story_id: row.last_story_id,
                evidence_refs: row.evidence_refs,
            })
            .collect(),
    }
}

async fn load_feed_database(args: &[String], limit: i64) -> Result<FeedData, Failure> {
    let days = forge::scorecard::parse_window_days(args);
    let database = database_for(args).await?;
    // Read the target BEFORE a DAO takes ownership: every number below is a statement about one
    // database and the header must name it.
    let target = database.declared_target().as_str().to_string();
    let read = ForgeReadDao::new(database.clone());
    let runs = read
        .scorecard_runs(days)
        .await
        .map_err(|error| Failure::failed(format!("cannot read scorecard runs: {error}")))?;
    let holds = read
        .scorecard_holds(days)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the HOLD ledger: {error}")))?;
    let corrections_dao = ForgeCorrectionsDao::new(database);
    let repeats = corrections_dao
        .repeated_corrections(2, limit)
        .await
        .map_err(|error| Failure::failed(format!("cannot read repeated corrections: {error}")))?;
    let findings = corrections_dao
        .list_open_learn_findings(limit)
        .await
        .map_err(|error| Failure::failed(format!("cannot read learn findings: {error}")))?;

    let reported_runs: Vec<ScorecardRun> = runs.into_iter().map(scorecard_run_from_row).collect();
    let reported_holds: Vec<ScorecardHold> = holds.iter().map(scorecard_hold_from_row).collect();
    let summary = summarize_scorecard(&reported_runs, &reported_holds, days);

    Ok(FeedData {
        source: "database".to_string(),
        target,
        days,
        scorecard: summary
            .rows
            .iter()
            .map(|row| ScorecardFeedRow {
                role: row.role.clone(),
                agent: row.agent.clone(),
                tasks: u64::try_from(row.tasks).unwrap_or(0),
                completed: u64::try_from(row.completed).unwrap_or(0),
                held_or_escalated: u64::try_from(row.held_or_escalated()).unwrap_or(0),
                failed_or_interrupted: u64::try_from(row.failed_or_interrupted()).unwrap_or(0),
                rework: u64::try_from(row.rework).unwrap_or(0),
                mean_wall_minutes: row.mean_wall_minutes,
                // The scorecard read path carries no durable run ids; feed targets require them, so
                // these rows stay observations and are reported as such rather than fabricated.
                evidence_refs: Vec::new(),
            })
            .collect(),
        holds: holds
            .iter()
            .map(|row| HoldFeedRow {
                node: forge::scorecard::normalized(row.node.as_deref()),
                failure_class: forge::scorecard::normalized(row.failure_class.as_deref()),
                holds: u32::try_from(row.holds).unwrap_or(u32::MAX),
                evidence_refs: Vec::new(),
            })
            .collect(),
        corrections: correction_feed_rows(&repeats),
        hypotheses: findings
            .iter()
            .map(|finding| HypothesisFeedRow {
                repository_key: finding.repository_key.clone(),
                finding_key: finding.finding_key.clone(),
                occurrence: u32::try_from(finding.occurrence).unwrap_or(0),
                last_story_id: finding.last_story_id.clone(),
                evidence_refs: Vec::new(),
            })
            .collect(),
    })
}

fn scorecard_run_from_row(row: ScorecardRunRow) -> ScorecardRun {
    ScorecardRun {
        role: row.role,
        agent: row.agent,
        result_status: row.result_status,
        retry_index: row.retry_index,
        wall_minutes: row.wall_minutes,
        cost_widgets: row.cost_widgets,
        cost_usd: row.cost_usd,
    }
}

fn scorecard_hold_from_row(row: &ScorecardHoldCountRow) -> ScorecardHold {
    ScorecardHold {
        node: row.node.clone(),
        failure_class: row.failure_class.clone(),
        holds: row.holds,
        open: row.open,
    }
}

fn feed_json(data: &FeedData, options: FeedOptions) -> Value {
    let targets =
        targets_from_learning_sources(&data.scorecard, &data.holds, &data.corrections, options);
    let hypotheses = targets_from_hypotheses(&data.hypotheses, options.max_targets);
    json!({
        "command": "feed",
        "read_only": true,
        "source": data.source,
        "target": data.target,
        "days": data.days,
        "options": {"max_targets": options.max_targets},
        "sources": {
            "scorecard": data.scorecard.len(),
            "holds": data.holds.len(),
            "corrections": data.corrections.len(),
            "hypotheses": data.hypotheses.len(),
        },
        "targets": targets.iter().map(target_json).collect::<Vec<_>>(),
        "hypotheses": hypotheses.iter().map(hypothesis_json).collect::<Vec<_>>(),
        "observations": {
            "scorecard": data.scorecard.iter().map(scorecard_feed_json).collect::<Vec<_>>(),
            "holds": data.holds.iter().map(hold_feed_json).collect::<Vec<_>>(),
            "corrections": data.corrections.iter().map(correction_feed_json).collect::<Vec<_>>(),
        },
        "not_captured": feed_not_captured(data),
    })
}

fn target_json(target: &forge::engine::harness_evolution::propose::ImprovementTarget) -> Value {
    serde_json::to_value(target).unwrap_or(Value::Null)
}

fn hypothesis_json(target: &forge::engine::harness_evolution::feed::HypothesisTarget) -> Value {
    json!({
        "id": target.id(),
        "domain": target.domain.id(),
        "repository_key": target.repository_key,
        "finding_key": target.finding_key,
        "hypothesis": target.hypothesis,
        "occurrence": target.occurrence,
        "evidence_refs": target.evidence_refs,
    })
}

fn scorecard_feed_json(row: &ScorecardFeedRow) -> Value {
    json!({
        "role": row.role,
        "agent": row.agent,
        "tasks": row.tasks,
        "completed": row.completed,
        "held_or_escalated": row.held_or_escalated,
        "failed_or_interrupted": row.failed_or_interrupted,
        "rework": row.rework,
        "mean_wall_minutes": row.mean_wall_minutes,
        "evidence_refs": row.evidence_refs,
        "targetable": !row.evidence_refs.is_empty(),
    })
}

fn hold_feed_json(row: &HoldFeedRow) -> Value {
    json!({
        "node": row.node,
        "failure_class": row.failure_class,
        "holds": row.holds,
        "evidence_refs": row.evidence_refs,
        "targetable": !row.evidence_refs.is_empty(),
    })
}

fn correction_feed_json(row: &CorrectionFeedRow) -> Value {
    json!({
        "lane": row.lane,
        "correction": row.correction,
        "occurrences": row.occurrences,
        "evidence_refs": row.evidence_refs,
        "targetable": !row.evidence_refs.is_empty(),
    })
}

fn feed_not_captured(data: &FeedData) -> Vec<String> {
    let mut notes = Vec::new();
    if data
        .scorecard
        .iter()
        .any(|row| row.evidence_refs.is_empty())
        || data.holds.iter().any(|row| row.evidence_refs.is_empty())
    {
        notes.push(
            "scorecard/HOLD observations carry no durable row ids on this read path; they are listed as observations, not targets (a target needs evidence)"
                .to_string(),
        );
    }
    if data
        .hypotheses
        .iter()
        .all(|row| row.evidence_refs.is_empty() && row.last_story_id.is_none())
        && !data.hypotheses.is_empty()
    {
        notes.push(
            "learn findings without a story or evidence ref cannot become hypothesis targets"
                .to_string(),
        );
    }
    notes
}

fn render_feed(data: &FeedData, options: FeedOptions) -> String {
    let targets =
        targets_from_learning_sources(&data.scorecard, &data.holds, &data.corrections, options);
    let hypotheses = targets_from_hypotheses(&data.hypotheses, options.max_targets);
    let mut lines = vec![format!(
        "forge harness-evolution feed — source={} target={} days={} (read-only)",
        data.source, data.target, data.days
    )];
    lines.push(format!(
        "  sources: scorecard={} holds={} corrections={} hypotheses={}",
        data.scorecard.len(),
        data.holds.len(),
        data.corrections.len(),
        data.hypotheses.len()
    ));
    for row in &data.scorecard {
        let wall = row
            .mean_wall_minutes
            .map(|minutes| format!("{minutes:.1}m"))
            .unwrap_or_else(|| forge::scorecard::SCORECARD_NOT_CAPTURED.to_string());
        lines.push(format!(
            "  scorecard {}/{} tasks={} completed={} held={} failed={} rework={} wall={} [{}]",
            row.role,
            row.agent,
            row.tasks,
            row.completed,
            row.held_or_escalated,
            row.failed_or_interrupted,
            row.rework,
            wall,
            if row.evidence_refs.is_empty() {
                "observation only"
            } else {
                "targetable"
            }
        ));
    }
    for row in &data.holds {
        lines.push(format!(
            "  hold {} {} holds={} [{}]",
            row.node,
            row.failure_class,
            row.holds,
            if row.evidence_refs.is_empty() {
                "observation only"
            } else {
                "targetable"
            }
        ));
    }
    for row in &data.corrections {
        lines.push(format!(
            "  correction {} x{} evidence={}",
            row.lane,
            row.occurrences,
            row.evidence_refs.join(",")
        ));
    }
    for hypothesis in &hypotheses {
        lines.push(format!(
            "  hypothesis [{}] {} (evidence {})",
            hypothesis.domain.id(),
            hypothesis.hypothesis,
            hypothesis.evidence_refs.join(",")
        ));
    }
    lines.push(format!(
        "TARGETS {}  HYPOTHESES {}",
        targets.len(),
        hypotheses.len()
    ));
    for target in &targets {
        lines.push(format!(
            "  target [{}] {} evidence={}",
            target.domain.id(),
            target.signal.id(),
            target.evidence_refs.join(",")
        ));
    }
    for note in feed_not_captured(data) {
        lines.push(format!("NOT CAPTURED: {note}"));
    }
    lines.join("\n") + "\n"
}

// ---- metrics --------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct MetricsData {
    source: String,
    target: String,
    limit: i64,
    observations: Vec<RoleModelObservation>,
    unmapped_outcomes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetricsFixture {
    #[serde(default)]
    observations: Vec<ObservationFixture>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationFixture {
    role: String,
    model: String,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(default = "default_attempt")]
    attempt: u32,
    #[serde(default)]
    route_matched: Option<bool>,
    #[serde(default)]
    cost_micros: Option<u64>,
    #[serde(default)]
    evidence_ref: String,
}

fn default_attempt() -> u32 {
    1
}

async fn run_metrics(args: &[String]) -> Result<u8, Failure> {
    let format = parse_format(args)?;
    let limit = parse_limit(args, 500, 5_000)?;
    let data = match input_path(args) {
        Some(path) => load_metrics_fixture(&path)?,
        None => load_metrics_database(args, limit).await?,
    };
    let metrics = role_model_metrics(&data.observations);
    match format {
        OutputFormat::Json => {
            print_json(&metrics_json(&data, &metrics))?;
            Ok(0)
        }
        OutputFormat::Text => {
            print!("{}", render_metrics(&data, &metrics));
            Ok(0)
        }
    }
}

fn load_metrics_fixture(path: &PathBuf) -> Result<MetricsData, Failure> {
    let raw = read_input(path)?;
    let fixture: MetricsFixture = serde_json::from_str(&raw).map_err(|error| {
        Failure::failed(format!(
            "cannot parse metrics input {}: {error}",
            path.display()
        ))
    })?;
    Ok(metrics_data_from_fixture(fixture))
}

fn metrics_data_from_fixture(fixture: MetricsFixture) -> MetricsData {
    let mut observations = Vec::with_capacity(fixture.observations.len());
    let mut unmapped_outcomes = 0u64;
    for row in fixture.observations {
        let mapped = row.outcome.as_deref().and_then(parse_run_outcome);
        if row.outcome.is_some() && mapped.is_none() {
            unmapped_outcomes += 1;
        }
        observations.push(RoleModelObservation {
            role: row.role,
            model: row.model,
            outcome: mapped,
            attempt: row.attempt.max(1),
            route_matched: row.route_matched,
            cost_micros: row.cost_micros,
            evidence_ref: row.evidence_ref,
        });
    }
    MetricsData {
        source: "input".to_string(),
        target: "input".to_string(),
        limit: 0,
        observations,
        unmapped_outcomes,
    }
}

async fn load_metrics_database(args: &[String], limit: i64) -> Result<MetricsData, Failure> {
    let database = database_for(args).await?;
    let target = database.declared_target().as_str().to_string();
    let dao = ForgeEngineDao::new(database);
    let turns = dao
        .recent_trajectory_turns(limit)
        .await
        .map_err(|error| Failure::failed(format!("cannot read trajectory turns: {error}")))?;
    let mut observations = Vec::with_capacity(turns.len());
    let mut unmapped_outcomes = 0u64;
    for turn in &turns {
        let (observation, unmapped) = observation_from_turn(turn);
        if unmapped {
            unmapped_outcomes += 1;
        }
        observations.push(observation);
    }
    Ok(MetricsData {
        source: "trajectory_turns".to_string(),
        target,
        limit,
        observations,
        unmapped_outcomes,
    })
}

fn observation_from_turn(row: &TurnTrajectoryRow) -> (RoleModelObservation, bool) {
    let outcome = row.outcome.as_deref().and_then(parse_run_outcome);
    let unmapped = row.outcome.is_some() && outcome.is_none();
    let attempt = row
        .role_attempt
        .and_then(|index| u32::try_from(index).ok())
        .map(|index| index.saturating_add(1))
        .filter(|attempt| *attempt > 0)
        .unwrap_or(1);
    // A turn recorded with `usage_known = false` asked for a reading and could not take one; it is
    // unpriced, never free. `cost_usd` is a vendor reading in dollars (the metric wants micro-units).
    let cost_micros = match (row.usage_known, row.cost_usd) {
        (Some(false), _) => None,
        (_, Some(usd)) if usd.is_finite() && usd >= 0.0 => Some((usd * 1_000_000.0).round() as u64),
        _ => None,
    };
    (
        RoleModelObservation {
            role: row
                .role
                .clone()
                .unwrap_or_else(|| forge::scorecard::SCORECARD_UNRECORDED.to_string()),
            model: row
                .model
                .clone()
                .unwrap_or_else(|| forge::scorecard::SCORECARD_UNRECORDED.to_string()),
            outcome,
            attempt,
            // No durable route comparison exists on the turn row: NOT CAPTURED, not "matched".
            route_matched: None,
            cost_micros,
            evidence_ref: format!("trajectory_turn:{}", row.attempt_key),
        },
        unmapped,
    )
}

fn parse_run_outcome(value: &str) -> Option<RunOutcome> {
    match value.trim().to_ascii_lowercase().as_str() {
        "completed" | "complete" | "success" | "succeeded" | "passed" => {
            Some(RunOutcome::Completed)
        }
        "held" | "hold" | "escalated" => Some(RunOutcome::Held),
        "failed" | "failure" | "error" | "errored" => Some(RunOutcome::Failed),
        "interrupted" | "cancelled" | "canceled" | "aborted" => Some(RunOutcome::Interrupted),
        _ => None,
    }
}

fn metrics_json(data: &MetricsData, metrics: &[RoleModelMetric]) -> Value {
    json!({
        "command": "metrics",
        "read_only": true,
        "source": data.source,
        "target": data.target,
        "limit": data.limit,
        "observations": data.observations.len(),
        "unmapped_outcomes": data.unmapped_outcomes,
        "metrics": metrics.iter().map(metric_json).collect::<Vec<_>>(),
        "not_captured": [
            "route_matched is not recorded on a trajectory turn",
            "a metric with a null value was not captured; null is not zero",
        ],
    })
}

fn metric_json(metric: &RoleModelMetric) -> Value {
    json!({
        "role": metric.role,
        "model": metric.model,
        "attempts": metric.attempts,
        "completed": metric.completed,
        "held": metric.held,
        "failed": metric.failed,
        "interrupted": metric.interrupted,
        "uncaptured_outcomes": metric.uncaptured_outcomes,
        "rework_attempts": metric.rework_attempts,
        "completion_rate": metric.completion_rate,
        "rework_rate": metric.rework_rate,
        "route_observed": metric.route_observed,
        "route_mismatches": metric.route_mismatches,
        "routing_accuracy": metric.routing_accuracy,
        "priced_attempts": metric.priced_attempts,
        "unpriced_attempts": metric.unpriced_attempts,
        "total_cost_micros": metric.total_cost_micros,
        "mean_cost_micros": metric.mean_cost_micros,
        "cost_coverage": metric.cost_coverage,
        "evidence_refs": metric.evidence_refs,
    })
}

fn render_metrics(data: &MetricsData, metrics: &[RoleModelMetric]) -> String {
    let mut lines = vec![format!(
        "forge harness-evolution metrics — source={} target={} observations={} (read-only)",
        data.source,
        data.target,
        data.observations.len()
    )];
    if data.unmapped_outcomes > 0 {
        lines.push(format!(
            "  {} turn(s) had an outcome outside the metric vocabulary; counted as uncaptured",
            data.unmapped_outcomes
        ));
    }
    if metrics.is_empty() {
        lines.push("  no trajectory observations".to_string());
    }
    for metric in metrics {
        lines.push(format!(
            "  {}/{} attempts={} completion={} rework={} routing={} cost={}",
            metric.role,
            metric.model,
            metric.attempts,
            rate(metric.completion_rate),
            rate(metric.rework_rate),
            rate(metric.routing_accuracy),
            cost(metric.total_cost_micros),
        ));
    }
    lines.join("\n") + "\n"
}

fn rate(value: Option<f64>) -> String {
    value.map_or_else(
        || forge::scorecard::SCORECARD_NOT_CAPTURED.to_string(),
        |rate| format!("{:.1}%", rate * 100.0),
    )
}

fn cost(value: Option<u64>) -> String {
    value.map_or_else(
        || forge::scorecard::SCORECARD_NOT_CAPTURED.to_string(),
        |micros| format!("{micros}µ"),
    )
}

// ---- calibration ----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalibrationFixture {
    #[serde(default)]
    samples: Vec<CalibrationFixtureSample>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalibrationFixtureSample {
    decision_id: String,
    verdict: String,
    #[serde(default)]
    stated_probability: Option<f64>,
    #[serde(default)]
    outcome: Option<bool>,
    #[serde(default)]
    declared_by: Option<String>,
    #[serde(default)]
    probability_source: Option<String>,
    #[serde(default)]
    evidence_refs: Vec<String>,
}

async fn run_calibration(args: &[String]) -> Result<u8, Failure> {
    let format = parse_format(args)?;
    let Some(path) = input_path(args) else {
        let message =
            "no durable calibration sample read path exists yet; pass --input <samples.json>";
        return match format {
            OutputFormat::Json => {
                print_json(&json!({
                    "command": "calibration",
                    "read_only": true,
                    "source": "none",
                    "samples": 0,
                    "scored": 0,
                    "brier": null,
                    "label": CalibrationReport::LABEL,
                    "not_captured": message,
                }))?;
                Ok(0)
            }
            OutputFormat::Text => {
                println!("forge harness-evolution calibration (read-only)");
                println!("NOT CAPTURED: {message}");
                Ok(0)
            }
        };
    };
    let samples = load_calibration_fixture(&path)?;
    let report = calibration_report(&samples)
        .map_err(|error| Failure::failed(format!("cannot calibrate: {error}")))?;
    match format {
        OutputFormat::Json => {
            print_json(&calibration_json(&report))?;
            Ok(0)
        }
        OutputFormat::Text => {
            println!("forge harness-evolution calibration (read-only)");
            println!("{}", report.summary_line());
            for bucket in &report.buckets {
                println!(
                    "  {} samples={} scored={} brier={}",
                    bucket.verdict.id(),
                    bucket.samples,
                    bucket.scored,
                    bucket
                        .brier
                        .map_or_else(|| "NOT SCORED".to_string(), |brier| format!("{brier:.4}"))
                );
            }
            Ok(0)
        }
    }
}

fn load_calibration_fixture(path: &PathBuf) -> Result<Vec<CalibrationSample>, Failure> {
    let raw = read_input(path)?;
    let fixture: CalibrationFixture = serde_json::from_str(&raw).map_err(|error| {
        Failure::failed(format!(
            "cannot parse calibration input {}: {error}",
            path.display()
        ))
    })?;
    calibration_samples_from_fixture(fixture)
}

fn calibration_samples_from_fixture(
    fixture: CalibrationFixture,
) -> Result<Vec<CalibrationSample>, Failure> {
    let mut samples = Vec::with_capacity(fixture.samples.len());
    for row in fixture.samples {
        let verdict = parse_adoption_verdict(&row.verdict).ok_or_else(|| {
            Failure::failed(format!(
                "calibration sample `{}` has unknown verdict `{}`; expected adopt|reject|more_evidence|council|chris",
                row.decision_id, row.verdict
            ))
        })?;
        let mut sample =
            CalibrationSample::new(row.decision_id, verdict).with_evidence_refs(row.evidence_refs);
        if let Some(probability) = row.stated_probability {
            sample = sample.with_probability(probability);
        }
        if let Some(outcome) = row.outcome {
            sample = sample.with_outcome(outcome);
        }
        if let Some(declared_by) = row.declared_by {
            sample = sample.with_declared_by(declared_by);
        }
        if let Some(probability_source) = row.probability_source {
            sample = sample.with_probability_source(probability_source);
        }
        samples.push(sample);
    }
    Ok(samples)
}

fn parse_adoption_verdict(value: &str) -> Option<AdoptionVerdict> {
    let value = value.trim();
    AdoptionVerdict::ALL
        .into_iter()
        .find(|verdict| verdict.id() == value)
}

fn calibration_json(report: &CalibrationReport) -> Value {
    json!({
        "command": "calibration",
        "read_only": true,
        "source": "input",
        "label": CalibrationReport::LABEL,
        "samples": report.samples,
        "scored": report.scored,
        "missing_outcomes": report.missing_outcomes,
        "missing_probabilities": report.missing_probabilities,
        "missing_both": report.missing_both,
        "brier": report.brier,
        "buckets": report.buckets.iter().map(|bucket| json!({
            "verdict": bucket.verdict.id(),
            "samples": bucket.samples,
            "scored": bucket.scored,
            "missing_outcomes": bucket.missing_outcomes,
            "missing_probabilities": bucket.missing_probabilities,
            "missing_both": bucket.missing_both,
            "brier": bucket.brier,
        })).collect::<Vec<_>>(),
    })
}

fn revision_json(row: &ForgeHarnessRevisionRow) -> Value {
    json!({
        "revision_id": row.revision_id,
        "parent_revision_id": row.parent_revision_id,
        "domain": row.domain,
        "status": row.status,
        "manifest_hash": row.manifest_hash,
        "created_at": row.created_at,
        "created_by": row.created_by,
    })
}

// ---------------------------------------------------------------------------------------------
// O2: probe — run a historical replay in the command sandbox and record durable probe rows
// ---------------------------------------------------------------------------------------------

/// A historical task already read through the target-aware DAO. `HistoricalTaskLoader` is a
/// synchronous seam; handing it the row the CLI read keeps probe on the operator's `--target`
/// (never the engine's process pool) and avoids opening a second connection.
struct StaticTaskLoader(HistoricalTask);

impl HistoricalTaskLoader for StaticTaskLoader {
    fn load(&self, _request: &HistoricalTaskRequest) -> Result<HistoricalTask, ReplayError> {
        Ok(self.0.clone())
    }
}

/// `--environment`: any probe class except Production. Production is a refusal, not a fallback;
/// an unknown word is a usage error (never silently mapped to development).
fn parse_probe_environment(args: &[String]) -> Result<EnvironmentClass, Failure> {
    match flag(args, "--environment").as_deref() {
        None | Some("development") | Some("dev") => Ok(EnvironmentClass::Development),
        Some("sandbox") | Some("local") => Ok(EnvironmentClass::Sandbox),
        Some("ci") | Some("test") => Ok(EnvironmentClass::Ci),
        Some("staging") => Ok(EnvironmentClass::Staging),
        Some("prod") | Some("production") => Err(Failure::failed(
            "probe refuses the production class: a live production environment is never a probe target",
        )),
        Some(other) => Err(Failure::usage(format!(
            "unknown --environment `{other}`; expected development, sandbox, ci, staging"
        ))),
    }
}

fn parse_proposal_kind(args: &[String]) -> Result<ProposalKind, Failure> {
    match flag(args, "--candidate-kind").as_deref() {
        None | Some("invariant") => Ok(ProposalKind::Invariant),
        Some("domain") => Ok(ProposalKind::Domain),
        Some(other) => Err(Failure::usage(format!(
            "unknown --candidate-kind `{other}`; expected invariant or domain"
        ))),
    }
}

fn parse_probe_mode(args: &[String]) -> Result<EvolutionMode, Failure> {
    match flag(args, "--mode").as_deref() {
        None | Some("parallel") => Ok(EvolutionMode::Parallel),
        Some("sequential") => Ok(EvolutionMode::Sequential),
        Some(other) => Err(Failure::usage(format!(
            "unknown --mode `{other}`; expected parallel or sequential"
        ))),
    }
}

fn parse_u32_flag(args: &[String], name: &str, default: u32, max: u32) -> Result<u32, Failure> {
    match flag(args, name) {
        None => Ok(default),
        Some(value) => value
            .parse::<u32>()
            .map(|value| value.clamp(1, max))
            .map_err(|_| Failure::usage(format!("{name} must be an integer"))),
    }
}

/// The k=1 budget by default; k>1 is expressible and validated by the model before any execution.
fn probe_budget(args: &[String]) -> Result<EvolutionBudget, Failure> {
    let k = parse_u32_flag(args, "--k", 1, 16)?;
    let budget = EvolutionBudget {
        k,
        max_steps: 1,
        max_probes: k.saturating_mul(2).max(3),
    };
    budget
        .validate()
        .map_err(|error| Failure::usage(format!("invalid probe budget: {error}")))?;
    Ok(budget)
}

fn parse_seed(args: &[String]) -> Result<u64, Failure> {
    match flag(args, "--seed") {
        None => Ok(0),
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| Failure::usage("--seed must be an unsigned integer")),
    }
}

/// Every `--flag VALUE` occurrence, in order (repeatable flags such as `--arg` and `--tool`).
fn repeated_flag(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .collect()
}

fn sanitize_identifier(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "story".to_string()
    } else {
        trimmed.chars().take(40).collect()
    }
}

/// A new experiment id per probe run: story-scoped and time-stamped, so two runs never collide and
/// the durable rows say which story they replayed.
fn probe_experiment_id(story_id: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!("probe-{}-{nanos}", sanitize_identifier(story_id))
}

/// Which sandbox the probe will use. Derived from the flags only (no backend availability check),
/// so argument errors are deterministic and testable without a model or a database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeSandboxChoice {
    /// The command sandbox (`--command`, the default and the only no-spend sandbox).
    Command,
    /// The model-turn sandbox (`--authorize-model-run`), which spends one paid turn per probe.
    ModelTurn,
}

/// Argument contract for the two sandboxes:
/// * `--command` and `--authorize-model-run` are mutually exclusive;
/// * `--model-backend` is only meaningful with explicit model-run authorization;
/// * absence of both is the command sandbox (a real run then fails closed on the missing command,
///   while `--dry-run` still works).
fn probe_sandbox_choice(args: &[String]) -> Result<ProbeSandboxChoice, Failure> {
    let authorized_model_run = args
        .iter()
        .any(|value| value == ModelTurnAuthorization::FLAG);
    let has_command = flag(args, "--command").is_some();
    let has_model_backend = flag(args, "--model-backend").is_some();
    if authorized_model_run && has_command {
        return Err(Failure::usage(format!(
            "{} and --command are mutually exclusive: one probe runs one sandbox",
            ModelTurnAuthorization::FLAG
        )));
    }
    if has_model_backend && !authorized_model_run {
        return Err(Failure::usage(format!(
            "--model-backend requires {} (a model turn is refused without explicit authorization)",
            ModelTurnAuthorization::FLAG
        )));
    }
    Ok(if authorized_model_run {
        ProbeSandboxChoice::ModelTurn
    } else {
        ProbeSandboxChoice::Command
    })
}

/// `--model-backend dsh|opencode`, else the `FORGE_HARNESS` backend, else the dsh default. An unknown
/// word is a typed refusal (the model-turn backend is never guessed).
fn parse_model_backend(args: &[String]) -> Result<ModelTurnBackend, Failure> {
    match flag(args, "--model-backend") {
        Some(name) => ModelTurnBackend::from_name(Some(&name))
            .map_err(|error| Failure::failed(format!("model-run backend refused: {error}"))),
        None => ModelTurnBackend::from_env()
            .map_err(|error| Failure::failed(format!("model-run backend refused: {error}"))),
    }
}

/// Build the command sandbox for a real run. Missing `--command` fails closed: the probe will not
/// invent a command, and the sandbox still enforces its exact-invocation allowlist.
fn probe_command_sandbox(args: &[String]) -> Result<CommandSandbox, Failure> {
    let program = flag(args, "--command").ok_or_else(|| {
        Failure::usage(
            "missing --command; the probe runs the command sandbox and fails closed without one",
        )
    })?;
    let command = SandboxCommand::new(program, repeated_flag(args, "--arg"));
    let sandbox = CommandSandbox::new(command);
    sandbox
        .validate()
        .map_err(|error| Failure::failed(format!("sandbox refused the probe command: {error}")))?;
    Ok(sandbox)
}

async fn run_probe(args: &[String]) -> Result<u8, Failure> {
    let format = parse_format(args)?;
    let database = database_for(args).await?;
    let target = database.declared_target().as_str().to_string();
    let dry_run = args.iter().any(|value| value == "--dry-run");
    // Sandbox-flag contract first: a mutually exclusive combination is a usage error before any
    // database read, never a run that gets halfway.
    let sandbox_choice = probe_sandbox_choice(args)?;

    let domain_id = required_flag(args, "--domain")?;
    let domain = HarnessDomain::from_id(&domain_id).ok_or_else(|| {
        Failure::usage(format!(
            "unknown --domain `{domain_id}`; expected a harness domain id"
        ))
    })?;
    let story_id = required_flag(args, "--story")?;
    let run_id = flag(args, "--run");
    let base_revision_id = required_flag(args, "--base-revision")?;
    let candidate_revision_id = required_flag(args, "--candidate-revision")?;
    let environment = parse_probe_environment(args)?;
    let candidate_kind = parse_proposal_kind(args)?;
    let mode = parse_probe_mode(args)?;
    let budget = probe_budget(args)?;
    let seed = parse_seed(args)?;
    let experiment_id = flag(args, "--experiment-id")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| probe_experiment_id(&story_id));

    // The historical task comes from the target the operator named, not a process-wide pool.
    let reader = ForgeReadDao::new(database.clone());
    let row = reader
        .historical_harness_task(&story_id, run_id.as_deref())
        .await
        .map_err(|error| Failure::failed(format!("cannot load historical task: {error}")))?
        .ok_or_else(|| {
            Failure::failed(format!(
                "no historical Forge run for story {story_id}{}; a replay never invents a task",
                run_id
                    .as_deref()
                    .map(|run| format!(" run {run}"))
                    .unwrap_or_default()
            ))
        })?;
    let task = HistoricalTask::new(row.story_id, row.run_id, row.title, row.packet)
        .with_source_environment(environment_class_for(row.execution_environment.as_deref()))
        .with_run_metadata(row.run_type, row.agent_runtime, row.result_status)
        .with_packet_sha(row.packet_sha);
    task.validate()
        .map_err(|error| Failure::failed(format!("historical task is unusable: {error}")))?;

    // Revisions are read from the same target database and verified before any execution.
    let harness_dao = ForgeHarnessDao::new(database.clone());
    let rows = harness_dao
        .list_all_revisions()
        .await
        .map_err(|error| Failure::failed(format!("cannot read harness revisions: {error}")))?;
    let find_revision = |revision_id: &str| -> Result<
        forge::engine::harness_evolution::model::HarnessRevision,
        Failure,
    > {
        let row = rows
            .iter()
            .find(|row| row.revision_id == revision_id)
            .cloned()
            .ok_or_else(|| {
                Failure::failed(format!(
                    "harness revision {revision_id} is not in the {target} catalog"
                ))
            })?;
        stored_revision(row).map_err(|error| {
            Failure::failed(format!(
                "revision {revision_id} failed verification: {error}"
            ))
        })
    };
    let base = find_revision(&base_revision_id)?;
    let candidate = find_revision(&candidate_revision_id)?;
    if base.domain != domain || candidate.domain != domain {
        return Err(Failure::failed(format!(
            "revisions belong to {} / {}, not {domain}; a probe is scoped to one harness domain",
            base.domain, candidate.domain
        )));
    }
    let revisions = MapRevisionSource::from_revisions([base.clone(), candidate.clone()])
        .map_err(|error| Failure::failed(format!("cannot build revision source: {error}")))?;

    let pins = ProbePins::new(
        format!("forge-story:{story_id}"),
        flag(args, "--model").unwrap_or_else(|| "deepseek-chat".to_string()),
        flag(args, "--model-policy").unwrap_or_else(|| "default".to_string()),
        flag(args, "--executor").unwrap_or_else(|| "opencode".to_string()),
        repeated_flag(args, "--tool"),
        environment,
        flag(args, "--dataset").unwrap_or_else(|| "forge-harness-evolution".to_string()),
        flag(args, "--test-plan").unwrap_or_else(|| "cargo test -p forge".to_string()),
        flag(args, "--replay-id").unwrap_or_else(|| "replay-harness-evolution".to_string()),
    );
    if pins.environment_class.is_production() {
        return Err(Failure::failed(
            "probe refuses production: a live production environment is never a probe target",
        ));
    }
    let request = RecordedReplayRequest {
        experiment_id: experiment_id.clone(),
        pins,
        mode,
        budget,
        seed,
        base_revision: base.id.clone(),
        candidate_revision: candidate.id.clone(),
        task_request: HistoricalTaskRequest::new(story_id.clone(), run_id.clone()),
    };

    let loader = StaticTaskLoader(task.clone());
    if dry_run {
        let plan = plan_recorded_replay(&loader, &revisions, &request)
            .map_err(|error| Failure::failed(format!("probe dry run refused: {error}")))?;
        let dry_run_sandbox = match sandbox_choice {
            ProbeSandboxChoice::Command => "command".to_string(),
            ProbeSandboxChoice::ModelTurn => {
                let backend = parse_model_backend(args)?;
                format!("model-turn:{}", backend.id())
            }
        };
        if format == OutputFormat::Json {
            print_json(&json!({
                "command": "probe",
                "dry_run": true,
                "read_only": true,
                "target": target,
                "experiment_id": experiment_id,
                "domain": domain.id(),
                "story_id": plan.task.story_id,
                "run_id": plan.task.run_id,
                "base_revision": plan.base.id,
                "candidate_revision": plan.candidate.id,
                "contract_digest": plan.contract_digest(),
                "sandbox": dry_run_sandbox,
                "model_run_authorized": sandbox_choice == ProbeSandboxChoice::ModelTurn,
                "evidence_refs": plan.task.evidence_refs,
                "persisted": Value::Null,
            }))?;
        } else {
            println!("PROBE DRY RUN (target={target}, read-only)");
            println!("  experiment  {experiment_id}  domain={}", domain.id());
            println!(
                "  story       {}  run={}",
                plan.task.story_id, plan.task.run_id
            );
            println!(
                "  revisions   {} -> {}  (contract {})",
                plan.base.id,
                plan.candidate.id,
                &plan.contract_digest()[..12.min(plan.contract_digest().len())]
            );
            println!(
                "  sandbox     {dry_run_sandbox}{}",
                if sandbox_choice == ProbeSandboxChoice::ModelTurn {
                    "  (explicit model-run authorization; a real run would spend one paid turn)"
                } else {
                    ""
                }
            );
            println!("  not run     the sandbox is not executed in a dry run; nothing was written");
        }
        return Ok(0);
    }

    // Real run: pick the sandbox the flags authorized. The model-turn path mints its authorization
    // here, which checks backend availability and refuses production BEFORE any spend.
    let (sandbox_label, outcome) = match sandbox_choice {
        ProbeSandboxChoice::Command => {
            let sandbox = probe_command_sandbox(args)?;
            let outcome = run_recorded_replay(&loader, &sandbox, &revisions, &request)
                .map_err(|error| Failure::failed(format!("probe refused or failed ({error})")))?;
            ("command".to_string(), outcome)
        }
        ProbeSandboxChoice::ModelTurn => {
            let backend = parse_model_backend(args)?;
            let authorization =
                ModelTurnAuthorization::authorize_model_run(backend.clone(), environment).map_err(
                    |error| Failure::failed(format!("model-run authorization refused: {error}")),
                )?;
            let sandbox = ModelTurnSandbox::new(authorization);
            sandbox.validate().map_err(|error| {
                Failure::failed(format!("model-turn sandbox refused the probe: {error}"))
            })?;
            let label = format!("model-turn:{}", backend.id());
            let outcome = run_recorded_replay(&loader, &sandbox, &revisions, &request)
                .map_err(|error| Failure::failed(format!("probe refused or failed ({error})")))?;
            (label, outcome)
        }
    };
    let meta = ReplayPersistenceMeta {
        experiment_id: &experiment_id,
        domain,
        candidate_kind,
        mode,
        budget,
        seed,
    };
    let rows = plan_replay_persistence(&meta, &outcome)
        .map_err(|error| Failure::failed(format!("probe records are unbuildable: {error}")))?;

    // Insert order follows the foreign keys: experiment -> candidate -> probes -> contrast.
    harness_dao
        .insert_experiment(&rows.experiment)
        .await
        .map_err(|error| {
            Failure::failed(format!(
                "probe record failed at experiment {experiment_id}: {error}"
            ))
        })?;
    harness_dao
        .insert_candidate(&rows.candidate)
        .await
        .map_err(|error| {
            Failure::failed(format!(
                "probe record failed at candidate (experiment {experiment_id}): {error}"
            ))
        })?;
    for probe in &rows.probes {
        harness_dao.insert_probe(probe).await.map_err(|error| {
            Failure::failed(format!(
                "probe record failed at probe {}: {error}",
                probe.probe_id
            ))
        })?;
    }
    harness_dao
        .insert_contrast(&rows.contrast)
        .await
        .map_err(|error| {
            Failure::failed(format!(
                "probe record failed at contrast {} (experiment {experiment_id}): {error}",
                rows.contrast.contrast_id
            ))
        })?;

    let probe_views: Vec<Value> = [&outcome.reference, &outcome.candidate_reading]
        .into_iter()
        .map(|reading| {
            json!({
                "probe_id": reading.outcome.probe_id,
                "side": reading.outcome.side.id(),
                "revision_id": reading.outcome.revision,
                "evidence_id": reading.outcome.evidence_id,
                "evidence_refs": reading.outcome.evidence_refs,
            })
        })
        .collect();
    if format == OutputFormat::Json {
        print_json(&json!({
            "command": "probe",
            "dry_run": false,
            "read_only": false,
            "target": target,
            "experiment_id": experiment_id,
            "domain": domain.id(),
            "story_id": outcome.task.story_id,
            "run_id": outcome.task.run_id,
            "base_revision": outcome.base.id,
            "candidate_revision": outcome.candidate.id,
            "contract_digest": outcome.contract_digest,
            "sandbox": sandbox_label,
            "outcome": if outcome.probe_failed() { "failed" } else { "compared" },
            "probes": probe_views,
            "contrast": {
                "contrast_id": outcome.contrast.id,
                "summary": outcome.contrast.summary,
                "evidence_refs": outcome.contrast.evidence_refs,
            },
            "persisted": {
                "experiment": rows.experiment.experiment_id,
                "candidate": rows.candidate.candidate_id,
                "probes": rows.probes.len(),
                "contrast": rows.contrast.contrast_id,
            },
        }))?;
    } else {
        println!("PROBE RECORDED (target={target})");
        println!(
            "  experiment  {}  domain={}  state=compared",
            experiment_id,
            domain.id()
        );
        println!(
            "  story       {}  run={}",
            outcome.task.story_id, outcome.task.run_id
        );
        println!(
            "  revisions   {} -> {}  (contract {})",
            outcome.base.id,
            outcome.candidate.id,
            &outcome.contract_digest[..12.min(outcome.contract_digest.len())]
        );
        println!("  sandbox     {sandbox_label}");
        println!(
            "  outcome     {}",
            if outcome.probe_failed() {
                "FAILED probe execution(s) — recorded as a failed experiment for review (nothing adopted)"
            } else {
                "compared"
            }
        );
        for probe in &probe_views {
            println!(
                "  probe       {}  {}  revision={}  evidence={}",
                probe.get("probe_id").and_then(Value::as_str).unwrap_or("-"),
                probe.get("side").and_then(Value::as_str).unwrap_or("-"),
                probe
                    .get("revision_id")
                    .and_then(Value::as_str)
                    .unwrap_or("-"),
                probe
                    .get("evidence_id")
                    .and_then(Value::as_str)
                    .unwrap_or("-"),
            );
        }
        println!(
            "  contrast    {}  {}",
            outcome.contrast.id,
            short(&outcome.contrast.summary, 80)
        );
        println!(
            "  persisted   experiment/candidate/{} probe(s)/contrast",
            rows.probes.len()
        );
    }
    // The rows are persisted for review, but a failed probe pair must not look like a clean run.
    if outcome.probe_failed() {
        return Err(Failure::failed(format!(
            "probe recorded FAILED execution(s); experiment {experiment_id} is persisted as failed for review"
        )));
    }
    Ok(0)
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|value| value == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn required_flag(args: &[String], name: &str) -> Result<String, Failure> {
    flag(args, name).ok_or_else(|| Failure::failed(format!("missing required {name}")))
}

fn wants_json(args: &[String]) -> bool {
    args.iter().any(|value| value == "--format")
        && args
            .iter()
            .position(|value| value == "--format")
            .and_then(|index| args.get(index + 1))
            .map(|value| value == "json")
            .unwrap_or(false)
}

fn short(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_string()
    } else {
        text.chars()
            .take(limit.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

fn print_json(value: &Value) -> Result<(), Failure> {
    println!(
        "{}",
        serde_json::to_string_pretty(value)
            .map_err(|error| Failure::failed(format!("cannot serialize JSON: {error}")))?
    );
    Ok(())
}

#[cfg(test)]
mod operator_tests {
    //! O1 argument parsing and JSON shape. These tests use the pure view builders and fixture
    //! conversion; no test opens a database or a model seat.

    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn format_parsing_accepts_text_and_json_and_rejects_unknown() {
        assert_eq!(
            parse_format(&args(&[])).expect("default"),
            OutputFormat::Text
        );
        assert_eq!(
            parse_format(&args(&["--format", "json"])).expect("json"),
            OutputFormat::Json
        );
        assert_eq!(
            parse_format(&args(&["--format", "text"])).expect("text"),
            OutputFormat::Text
        );
        let error = parse_format(&args(&["--format", "yaml"])).expect_err("unknown format");
        assert!(error.to_string().contains("expected text or json"));
    }

    #[test]
    fn target_parsing_names_dev_and_prod_and_refuses_guesswork() {
        assert!(parse_target(&args(&[])).expect("absent").is_none());
        assert_eq!(
            parse_target(&args(&["--target", "dev"])).expect("dev"),
            Some(DbTarget::Dev)
        );
        assert_eq!(
            parse_target(&args(&["--target", "prod"])).expect("prod"),
            Some(DbTarget::Prod)
        );
        assert!(parse_target(&args(&["--target", "staging"])).is_err());
    }

    #[test]
    fn limit_parsing_clamps_and_rejects_non_integers() {
        assert_eq!(parse_limit(&args(&[]), 20, 200).expect("default"), 20);
        assert_eq!(
            parse_limit(&args(&["--limit", "999"]), 20, 200).expect("clamp"),
            200
        );
        assert_eq!(
            parse_limit(&args(&["--limit", "0"]), 20, 200).expect("floor"),
            1
        );
        assert!(parse_limit(&args(&["--limit", "many"]), 20, 200).is_err());
    }

    fn feed_fixture_json() -> &'static str {
        r#"{
          "scorecard":[{"role":"smith","agent":"opencode-harness","tasks":10,"completed":6,
            "held_or_escalated":1,"failed_or_interrupted":2,"rework":4,
            "mean_wall_minutes":12.5}],
          "holds":[{"node":"validate","failure_class":"missing_test","holds":3}],
          "corrections":[{"lane":"smith","correction":"name the failing migration",
            "occurrences":3,"evidence_refs":["correction:1","correction:2"]}],
          "hypotheses":[{"repository_key":"forge","finding_key":"silent-catch",
            "occurrence":2,"last_story_id":"S-1"}]
        }"#
    }

    #[test]
    fn feed_fixture_emits_only_evidence_linked_targets() {
        let fixture: FeedFixture =
            serde_json::from_str(feed_fixture_json()).expect("fixture parses");
        let data = feed_data_from_fixture(fixture, 30);
        assert_eq!(data.scorecard.len(), 1);
        assert_eq!(data.corrections.len(), 1);

        let view = feed_json(&data, FeedOptions::default());
        assert_eq!(view["read_only"], json!(true));
        assert_eq!(view["source"], "input");
        assert_eq!(view["sources"]["corrections"], json!(1));
        let targets = view["targets"].as_array().expect("targets array");
        assert_eq!(
            targets.len(),
            1,
            "only the correction row has durable evidence"
        );
        assert_eq!(targets[0]["signal"]["signal"], "repeated_correction");
        assert_eq!(targets[0]["evidence_refs"][0], "correction:1");
        let hypotheses = view["hypotheses"].as_array().expect("hypotheses array");
        assert_eq!(hypotheses.len(), 1);
        assert_eq!(hypotheses[0]["evidence_refs"][0], "storyboard_story:S-1");
        assert!(
            !view["not_captured"].as_array().expect("notes").is_empty(),
            "unreferenced observations must be explained"
        );
    }

    #[test]
    fn feed_render_never_prints_a_zero_for_unmeasured_wall_time() {
        let fixture: FeedFixture = serde_json::from_str(
            r#"{"scorecard":[{"role":"smith","agent":"opencode","tasks":10,"completed":6,
                "held_or_escalated":1,"failed_or_interrupted":2,"rework":1}]}"#,
        )
        .expect("fixture");
        let data = feed_data_from_fixture(fixture, 30);
        let text = render_feed(&data, FeedOptions::default());
        assert!(
            text.contains("NOT CAPTURED"),
            "unmeasured wall time is not 0m: {text}"
        );
    }

    #[test]
    fn metrics_fixture_maps_outcomes_and_reports_uncaptured_as_null() {
        let fixture: MetricsFixture = serde_json::from_str(
            r#"{"observations":[
                {"role":"smith","model":"deepseek-flash","outcome":"completed","attempt":1,
                 "cost_micros":1200,"evidence_ref":"trajectory_turn:a"},
                {"role":"smith","model":"deepseek-flash","outcome":"uncertain","attempt":2,
                 "evidence_ref":"trajectory_turn:b"}
            ]}"#,
        )
        .expect("fixture");
        let data = metrics_data_from_fixture(fixture);
        assert_eq!(data.observations.len(), 2);
        assert_eq!(
            data.unmapped_outcomes, 1,
            "an outcome outside the vocabulary is counted, not silently called failed"
        );

        let metrics = role_model_metrics(&data.observations);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].completed, 1);
        assert_eq!(metrics[0].uncaptured_outcomes, 1);
        assert_eq!(metrics[0].total_cost_micros, Some(1200));

        let view = metrics_json(&data, &metrics);
        assert_eq!(view["read_only"], json!(true));
        assert_eq!(view["metrics"][0]["routing_accuracy"], Value::Null);
        assert_eq!(
            view["metrics"][0]["completion_rate"],
            json!(1.0),
            "completion rate is over CAPTURED outcomes; the uncaptured attempt is not a failure"
        );
        assert!(!view["not_captured"].as_array().expect("notes").is_empty());
    }

    fn calibration_fixture_json() -> &'static str {
        r#"{"samples":[
           {"decision_id":"decision-1","verdict":"adopt","stated_probability":0.9,
            "outcome":true,"declared_by":"policy","probability_source":"decision_row",
            "evidence_refs":["activation:1"]},
           {"decision_id":"decision-2","verdict":"council","outcome":false,
            "evidence_refs":["activation:2"]}
        ]}"#
    }

    #[test]
    fn calibration_scores_brier_and_counts_unstated_probabilities() {
        let fixture: CalibrationFixture =
            serde_json::from_str(calibration_fixture_json()).expect("fixture");
        let samples = calibration_samples_from_fixture(fixture).expect("samples");
        let report = calibration_report(&samples).expect("report");
        assert_eq!(report.samples, 2);
        assert_eq!(report.scored, 1);
        assert_eq!(report.missing_probabilities, 1);
        assert!(
            report.brier.expect("one scored sample") < 0.02,
            "0.9 against a right outcome is a small Brier score"
        );

        let view = calibration_json(&report);
        assert_eq!(view["read_only"], json!(true));
        assert_eq!(view["label"], CalibrationReport::LABEL);
        assert_eq!(view["buckets"].as_array().expect("buckets").len(), 2);
    }

    #[test]
    fn calibration_refuses_an_out_of_range_probability_and_unknown_verdict() {
        let out_of_range: CalibrationFixture = serde_json::from_str(
            r#"{"samples":[{"decision_id":"d","verdict":"adopt",
                "stated_probability":1.4,"outcome":true}]}"#,
        )
        .expect("fixture");
        let samples = calibration_samples_from_fixture(out_of_range).expect("samples");
        assert!(
            calibration_report(&samples).is_err(),
            "no clamping: 1.4 is a refusal"
        );

        let unknown: CalibrationFixture =
            serde_json::from_str(r#"{"samples":[{"decision_id":"d","verdict":"vibes"}]}"#)
                .expect("fixture");
        let error = calibration_samples_from_fixture(unknown).expect_err("unknown verdict");
        assert!(error.to_string().contains("unknown verdict"));
    }

    #[test]
    fn calibration_with_nothing_to_score_is_none_not_zero() {
        let report = calibration_report(&[]).expect("empty report");
        assert_eq!(report.brier, None, "nothing scored is None, never 0.0");
        let view = calibration_json(&report);
        assert_eq!(view["brier"], Value::Null);
        assert_eq!(view["samples"], json!(0));
    }
}

#[cfg(test)]
mod probe_tests {
    //! O2 probe argument rules: no test opens a database, a sandbox, or a model seat.

    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn probe_environment_never_maps_to_production_and_unknown_words_are_usage_errors() {
        assert_eq!(
            parse_probe_environment(&args(&[])).unwrap(),
            EnvironmentClass::Development
        );
        assert_eq!(
            parse_probe_environment(&args(&["--environment", "sandbox"])).unwrap(),
            EnvironmentClass::Sandbox
        );
        assert_eq!(
            parse_probe_environment(&args(&["--environment", "ci"])).unwrap(),
            EnvironmentClass::Ci
        );
        assert_eq!(
            parse_probe_environment(&args(&["--environment", "staging"])).unwrap(),
            EnvironmentClass::Staging
        );
        let refused = parse_probe_environment(&args(&["--environment", "prod"]))
            .expect_err("production is refused");
        assert!(refused.to_string().contains("production"), "{refused}");
        let typo = parse_probe_environment(&args(&["--environment", "prodcution"]))
            .expect_err("unknown is not a silent default");
        assert!(typo.to_string().contains("unknown --environment"), "{typo}");
    }

    #[test]
    fn probe_kind_and_mode_are_closed_vocabularies() {
        assert_eq!(
            parse_proposal_kind(&args(&[])).unwrap(),
            ProposalKind::Invariant
        );
        assert_eq!(
            parse_proposal_kind(&args(&["--candidate-kind", "domain"])).unwrap(),
            ProposalKind::Domain
        );
        assert!(parse_proposal_kind(&args(&["--candidate-kind", "guess"])).is_err());

        assert_eq!(
            parse_probe_mode(&args(&[])).unwrap(),
            EvolutionMode::Parallel
        );
        assert_eq!(
            parse_probe_mode(&args(&["--mode", "sequential"])).unwrap(),
            EvolutionMode::Sequential
        );
        assert!(parse_probe_mode(&args(&["--mode", "recursive"])).is_err());
    }

    #[test]
    fn the_probe_budget_defaults_to_k1_and_is_always_valid() {
        let k1 = probe_budget(&args(&[])).expect("k1 budget");
        assert_eq!(k1.k, 1);
        assert_eq!(k1.max_steps, 1);
        assert_eq!(k1.max_probes, 3, "k1 keeps the model's probe ceiling");
        k1.validate().expect("k1 valid");

        let k4 = probe_budget(&args(&["--k", "4"])).expect("k4 budget");
        assert_eq!(k4.k, 4);
        assert_eq!(
            k4.max_probes, 8,
            "max_probes must cover k candidates per scope"
        );
        k4.validate().expect("k4 valid");

        // clamps, never silently zero: k=0 becomes 1, k=999 becomes 16.
        assert_eq!(probe_budget(&args(&["--k", "0"])).unwrap().k, 1);
        assert_eq!(probe_budget(&args(&["--k", "999"])).unwrap().k, 16);
        assert!(probe_budget(&args(&["--k", "many"])).is_err());
    }

    #[test]
    fn repeated_flags_preserve_argument_order() {
        let parsed = args(&[
            "--arg",
            "--release",
            "--arg",
            "probe",
            "--tool",
            "ripgrep",
            "--tool",
            "cargo",
        ]);
        assert_eq!(repeated_flag(&parsed, "--arg"), vec!["--release", "probe"]);
        assert_eq!(repeated_flag(&parsed, "--tool"), vec!["ripgrep", "cargo"]);
        assert!(repeated_flag(&parsed, "--missing").is_empty());
        assert_eq!(parse_seed(&args(&["--seed", "42"])).unwrap(), 42);
        assert!(parse_seed(&args(&["--seed", "-1"])).is_err());
    }

    #[test]
    fn a_probe_experiment_id_is_story_scoped() {
        let id = probe_experiment_id("story/with spaces");
        assert!(id.starts_with("probe-story-with-spaces-"), "{id}");
        let other = probe_experiment_id("story-2");
        assert!(other.starts_with("probe-story-2-"), "{other}");
        assert_eq!(sanitize_identifier("---"), "story");
    }

    #[test]
    fn model_run_authorization_and_command_are_mutually_exclusive() {
        let both = probe_sandbox_choice(&args(&[
            "--authorize-model-run",
            "--command",
            "/usr/bin/true",
        ]))
        .expect_err("both sandboxes cannot run");
        assert!(both.to_string().contains("mutually exclusive"), "{both}");

        let backend_without_authorization =
            probe_sandbox_choice(&args(&["--model-backend", "dsh"])).expect_err("no token");
        assert!(
            backend_without_authorization
                .to_string()
                .contains("requires --authorize-model-run"),
            "{backend_without_authorization}"
        );

        assert_eq!(
            probe_sandbox_choice(&args(&[])).unwrap(),
            ProbeSandboxChoice::Command
        );
        assert_eq!(
            probe_sandbox_choice(&args(&["--command", "/usr/bin/true"])).unwrap(),
            ProbeSandboxChoice::Command
        );
        assert_eq!(
            probe_sandbox_choice(&args(&["--authorize-model-run"])).unwrap(),
            ProbeSandboxChoice::ModelTurn
        );
        assert_eq!(
            probe_sandbox_choice(&args(&[
                "--authorize-model-run",
                "--model-backend",
                "opencode"
            ]))
            .unwrap(),
            ProbeSandboxChoice::ModelTurn
        );
    }

    #[test]
    fn the_model_backend_selector_is_a_closed_vocabulary() {
        assert_eq!(
            parse_model_backend(&args(&["--model-backend", "dsh"])).unwrap(),
            ModelTurnBackend::Dsh
        );
        assert_eq!(
            parse_model_backend(&args(&["--model-backend", "opencode"])).unwrap(),
            ModelTurnBackend::OpenCode
        );
        let unknown = parse_model_backend(&args(&["--model-backend", "anthropic"]))
            .expect_err("unknown backend is refused");
        assert!(unknown.to_string().contains("unknown"), "{unknown}");
    }

    #[test]
    fn the_command_sandbox_fails_closed_without_a_command() {
        let missing = probe_command_sandbox(&args(&[])).expect_err("missing --command");
        assert!(
            missing.to_string().contains("missing --command"),
            "{missing}"
        );
        let relative = probe_command_sandbox(&args(&["--command", "cargo"]))
            .expect_err("relative program is refused");
        assert!(
            relative.to_string().contains("sandbox refused"),
            "{relative}"
        );
        let not_allowed = probe_command_sandbox(&args(&["--command", "/bin/ls"]))
            .expect_err("non-allowlisted invocation is refused");
        assert!(
            not_allowed.to_string().contains("sandbox refused"),
            "{not_allowed}"
        );
        // The default allowlist contains /usr/bin/true with no arguments.
        probe_command_sandbox(&args(&["--command", "/usr/bin/true"])).expect("allowlisted");
    }
}
