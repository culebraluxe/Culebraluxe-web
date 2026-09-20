//! Cutover host. Engine + Forge + OpenCode.
//! NeonStore when APP_ENV / VERCEL_ENV is set. MemoryStore only for local dry-run.

use forge::engine::db_writer::DbForgeStateWriter;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{drive_forge_story, DriveForgeStoryOptions};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::git_publish::{GitReleaseOps, HostReleaseExecutor};
use forge::engine::opencode::OpenCodeHarness;
use forge::engine::packet::{ExecutionWorkspace, StoryPacket};
use forge::engine::worktree::{provision_worker_workspace, resolve_approved_base_ref};
use forge::engine::runner::ProductionRoleRunner;
use forge::engine::runtime::ForgeRuntime;
use forge::engine::vendor_session::database_url;
use forge::engine::writer::{ForgeReleaseExecutor, ForgeStateWriter, NullWriter};
use std::env;
use std::sync::Arc;
use workflow::{MemoryStore, NeonStore, TxStore};

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let story = flag(&args, "--story")
        .or_else(|| env::var("FORGE_STORY_ID").ok())
        .unwrap_or_default();
    if story.trim().is_empty() {
        eprintln!("usage: forge --story <id> [--work-type FEATURE|FAST|BUG|HOTFIX|RESEARCH|MIGRATION]");
        std::process::exit(2);
    }
    let work_type = flag(&args, "--work-type")
        .or_else(|| env::var("FORGE_WORK_TYPE").ok())
        .unwrap_or_else(|| "FEATURE".into());
    const ALLOWED: &[&str] = &["FEATURE", "FAST", "BUG", "HOTFIX", "RESEARCH", "MIGRATION"];
    if !ALLOWED.contains(&work_type.as_str()) {
        eprintln!("invalid --work-type {work_type}");
        std::process::exit(2);
    }
    if let Err(e) = forge::engine::execution_target::assert_forge_lane_may_start(
        &forge::engine::execution_target::env_pairs_from_process(),
    ) {
        eprintln!("{e}");
        std::process::exit(2);
    }
    let brain = forge::engine::routing_brain::parse_forge_routing_brain(
        env::var("FORGE_ROUTING_BRAIN").ok().as_deref(),
    );
    eprintln!("routing-brain={brain:?}");
    let mut harness = match OpenCodeHarness::from_env() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
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
                eprintln!("worktree {} branch {} base {}", ws.worktree_path.display(), ws.branch_name, ws.base_commit);
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
    let code = if use_neon {
        match NeonStore::connect_from_env() {
            Ok(store) => {
                eprintln!("workflow store=neon");
                drive(store, release, writer.clone(), &harness, &story, &work_type)
            }
            Err(e) => {
                eprintln!("neon store: {e}");
                1
            }
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
    std::process::exit(code);
}

fn drive<S: TxStore>(
    store: S,
    release: Arc<dyn ForgeReleaseExecutor>,
    writer: Arc<dyn ForgeStateWriter>,
    harness: &OpenCodeHarness,
    story: &str,
    work_type: &str,
) -> i32 {
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
            return 1;
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
        Ok(out) => {
            println!(
                "instance={} status={} steps={:?} human={} stopped={:?}",
                out.instance_id, out.status, out.steps, out.needs_human, out.stopped_after
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
