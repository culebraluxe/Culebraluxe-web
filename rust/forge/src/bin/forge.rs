//! Cutover host. Engine + Forge + OpenCode.
//! NeonStore when APP_ENV / VERCEL_ENV is set. MemoryStore only for local dry-run.

use db::AgentWorkOutcome;
use forge::engine::agent_work;
use forge::engine::db_writer::DbForgeStateWriter;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{drive_forge_story, DriveForgeStoryOptions};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::git_publish::{GitReleaseOps, HostReleaseExecutor};
use forge::engine::opencode::OpenCodeHarness;
use forge::engine::packet::{ExecutionWorkspace, StoryPacket};
use forge::engine::runner::ProductionRoleRunner;
use forge::engine::runtime::ForgeRuntime;
use forge::engine::vendor_session::database_url;
use forge::engine::worktree::{provision_worker_workspace, resolve_approved_base_ref};
use forge::engine::writer::{ForgeReleaseExecutor, ForgeStateWriter, NullWriter};
use std::env;
use std::sync::Arc;
use workflow::{MemoryStore, NeonStore, TxStore};

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

/// The engine's default per-statement ceiling: 5 minutes, against the pool's 30-second request-path default.
/// See the comment in `main`. Long enough for a cold branch's first write; still a ceiling, so a genuinely stuck
/// query ends the run instead of holding it forever.
const ENGINE_STATEMENT_TIMEOUT_MS: &str = "300000";

/// The engine's default wait for an open connection: 60 seconds, against the pool's 10-second request-path
/// default. A cold Neon branch has to complete the pool's floor of handshakes before the first statement can run.
const ENGINE_CONNECT_TIMEOUT_MS: &str = "60000";

/// A claim whose launch configuration the engine refuses: terminalize it now, as `Error`.
///
/// Leaving it `Claimed` would hand the row to stale recovery ten minutes later, which reports a dead worker rather
/// than a configuration that was never runnable. This is `reject_agent_work_configuration`'s first caller — the DAO
/// that could not run until 2026-09-29, because it wrote the illegal state `Failed`.
fn reject_configuration(work_item: Option<&str>, reason: &str) {
    let Some(item) = work_item else {
        return;
    };
    match agent_work::reject_agent_work_configuration(item, reason) {
        Ok(()) => eprintln!("work_item={item} state=Error ({reason})"),
        Err(error) => eprintln!("work_item={item} could not be rejected: {error}"),
    }
}

/// The run's own terminal write, on the way out of `main`. A second settle is a no-op by construction, so this
/// cannot overwrite a verdict the worker or recovery already wrote.
fn settle_work_item(work_item: Option<&str>, outcome: AgentWorkOutcome, reason: Option<&str>) {
    let Some(item) = work_item else {
        return;
    };
    match agent_work::finish_agent_work_run(item, outcome, reason) {
        Ok(true) => eprintln!("work_item={item} state={}", outcome.as_state()),
        Ok(false) => eprintln!("work_item={item} already had a verdict; left as-is"),
        Err(error) => eprintln!("work_item={item} could not be settled: {error}"),
    }
}

fn main() {
    // The engine's own database budgets, and the rule behind them: `forge::engine::db_budget`. Both binaries that
    // talk to the control plane install these, because the process that was dying was the worker.
    let budget = forge::engine::db_budget::install_engine_db_budget();
    let args: Vec<String> = env::args().collect();
    // The claim this run was launched against, if any. A claimed run **owns** its queue row: it opens it as
    // `Running` before the first role turn and settles it on the way out. An operator invocation without the flag
    // stays legal and owns nothing — but nothing that dispatches may launch without it (`engine::worker` claims
    // first and passes this id).
    let work_item = flag(&args, "--work-item")
        .or_else(|| env::var("FORGE_WORK_ITEM_ID").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let story = flag(&args, "--story")
        .or_else(|| env::var("FORGE_STORY_ID").ok())
        .unwrap_or_default();
    if story.trim().is_empty() {
        reject_configuration(work_item.as_deref(), "no --story");
        eprintln!(
            "usage: forge --story <id> [--work-type FEATURE|FAST|BUG|HOTFIX|RESEARCH|MIGRATION] [--work-item <uuid>]"
        );
        std::process::exit(2);
    }
    let work_type = flag(&args, "--work-type")
        .or_else(|| env::var("FORGE_WORK_TYPE").ok())
        .unwrap_or_else(|| "FEATURE".into());
    const ALLOWED: &[&str] = &["FEATURE", "FAST", "BUG", "HOTFIX", "RESEARCH", "MIGRATION"];
    if !ALLOWED.contains(&work_type.as_str()) {
        reject_configuration(
            work_item.as_deref(),
            &format!("invalid --work-type {work_type}"),
        );
        eprintln!("invalid --work-type {work_type}");
        std::process::exit(2);
    }
    if let Err(e) = forge::engine::execution_target::assert_forge_lane_may_start(
        &forge::engine::execution_target::env_pairs_from_process(),
    ) {
        reject_configuration(work_item.as_deref(), &format!("{e}"));
        eprintln!("{e}");
        std::process::exit(2);
    }
    // Only now is this run real, so only now does it go `Running`. If the claim cannot be opened the run must not
    // start at all: a story driven without a claim is exactly the unowned dispatch this seam exists to remove.
    if let Some(item) = work_item.as_deref() {
        if let Err(error) = agent_work::begin_agent_work_run(item) {
            eprintln!("cannot begin work item {item}: {error}");
            std::process::exit(2);
        }
        eprintln!("work_item={item} state=Running");
    }
    let brain = forge::engine::routing_brain::parse_forge_routing_brain(
        env::var("FORGE_ROUTING_BRAIN").ok().as_deref(),
    );
    eprintln!("routing-brain={brain:?}");
    let mut harness = match OpenCodeHarness::from_env() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
            settle_work_item(work_item.as_deref(), AgentWorkOutcome::Error, Some(&format!("{e}")));
            std::process::exit(2);
        }
    };
    match StoryPacket::load_from_neon(&story) {
        Ok(packet) => {
            eprintln!("packet from storyboard_story {}", packet.id);
            harness.packet = packet;
            harness.story_id = Some(story.clone());
        }
        Err(e) => eprintln!("story packet: {e} (using env packet)"),
    }
    if env::var("FORGE_PROVISION").ok().as_deref() == Some("1") {
        match provision_worker_workspace(
            env::current_dir().ok().as_deref(),
            &story,
            env::var("FORGE_RUN_ID").ok().as_deref(),
            Some(&resolve_approved_base_ref()),
            None,
        ) {
            Ok(ws) => {
                eprintln!(
                    "worktree {} branch {} base {}",
                    ws.worktree_path.display(),
                    ws.branch_name,
                    ws.base_commit
                );
                harness.workspace = ws.worktree_path.clone();
                harness.execution_workspace = Some(ExecutionWorkspace {
                    worktree_path: ws.worktree_path.display().to_string(),
                    branch_name: ws.branch_name,
                    base_ref: ws.base_ref,
                    base_commit: ws.base_commit,
                });
            }
            Err(e) => {
                eprintln!("provision: {e}");
                settle_work_item(
                    work_item.as_deref(),
                    AgentWorkOutcome::Error,
                    Some(&format!("provision: {e}")),
                );
                std::process::exit(2);
            }
        }
    }
    let repo = env::current_dir().unwrap_or_else(|_| ".".into());
    let release = Arc::new(HostReleaseExecutor {
        ops: GitReleaseOps { repo_root: repo },
    });
    eprintln!(
        "harness={} model={} bin={} cwd={} neon={}",
        forge::engine::opencode::OPENCODE_HARNESS_ADAPTER_ID,
        harness.model,
        harness.cli_bin,
        harness.workspace.display(),
        database_url().is_some()
    );
    // Declared, not inferred: the budgets a run is actually using, said out loud. The run that died at the
    // statement ceiling and the tick that died on the connect budget both left no trace of which they had.
    eprintln!(
        "statement_ceiling_ms={} connect_budget_ms={}",
        budget.statement_timeout_ms, budget.connect_timeout_ms
    );

    let writer: Arc<dyn ForgeStateWriter> = match DbForgeStateWriter::connect_env() {
        Ok(w) => {
            eprintln!("story writer=neon");
            Arc::new(w)
        }
        Err(e) => {
            eprintln!("story writer=null ({e})");
            Arc::new(NullWriter)
        }
    };
    let use_neon = env::var("APP_ENV").is_ok() || env::var("VERCEL_ENV").is_ok();
    let result = if use_neon {
        match NeonStore::connect_from_env() {
            Ok(store) => {
                eprintln!("workflow store=neon");
                drive(store, release, writer.clone(), &harness, &story, &work_type)
            }
            Err(e) => Err(format!("neon store: {e}")),
        }
    } else {
        eprintln!("workflow store=memory (APP_ENV unset)");
        drive(
            MemoryStore::new(),
            release,
            writer.clone(),
            &harness,
            &story,
            &work_type,
        )
    };
    // One exit, one verdict. `Ok` means the story was driven through its turn (a story that stopped for a human comes
    // back `Ok` and the board's Hold says so); `Err` means the run failed. Either way the claim is settled here, not
    // left for stale recovery to guess about.
    match result {
        Ok(summary) => {
            println!("{summary}");
            settle_work_item(work_item.as_deref(), AgentWorkOutcome::Done, None);
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}");
            settle_work_item(
                work_item.as_deref(),
                AgentWorkOutcome::Error,
                Some(&error),
            );
            std::process::exit(1);
        }
    }
}

fn drive<S: TxStore>(
    store: S,
    release: Arc<dyn ForgeReleaseExecutor>,
    writer: Arc<dyn ForgeStateWriter>,
    harness: &OpenCodeHarness,
    story: &str,
    work_type: &str,
) -> Result<String, String> {
    let mut rt = match ForgeRuntime::from_store(
        store,
        writer.clone(),
        Some(release),
        None,
        forge_sdlc_definition(),
    ) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("{e}");
            return Err(format!("{e}"));
        }
    };
    let evidence = ForgeGateEvidence {
        work_type: Some(work_type.to_string()),
        scout_required: Some(false),
        ..Default::default()
    };
    let runner = ProductionRoleRunner::new(harness, evidence.clone());
    match drive_forge_story(
        &mut rt,
        story,
        DriveForgeStoryOptions {
            work_type,
            evidence,
            runner: Some(&runner),
            allow_synthetic_runner: false,
            max_steps: 40,
            worker_id: "forge",
            split_concurrency: 1,
            stop_after: None,
        },
    ) {
        Ok(out) => Ok(format!(
            "instance={} status={} steps={:?} human={} stopped={:?}",
            out.instance_id, out.status, out.steps, out.needs_human, out.stopped_after
        )),
        Err(e) => {
            eprintln!("{e}");
            Err(format!("{e}"))
        }
    }
}
