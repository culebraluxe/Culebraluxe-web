//! Cutover host. Engine + Forge + OpenCode.
//! NeonStore when APP_ENV / VERCEL_ENV is set. MemoryStore only for local dry-run.

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{drive_forge_story, DriveForgeStoryOptions};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::git_publish::{GitReleaseOps, HostReleaseExecutor};
use forge::engine::opencode::OpenCodeHarness;
use forge::engine::packet::StoryPacket;
use forge::engine::runner::ProductionRoleRunner;
use forge::engine::runtime::ForgeRuntime;
use forge::engine::vendor_session::database_url;
use forge::engine::writer::{ForgeReleaseExecutor, NullWriter};
use std::env;
use std::sync::Arc;
use workflow::{MemoryStore, NeonStore, TxStore};

fn main() {
    let story = env::var("FORGE_STORY_ID").unwrap_or_default();
    if story.trim().is_empty() {
        eprintln!("FORGE_STORY_ID is required");
        std::process::exit(2);
    }
    let work_type = env::var("FORGE_WORK_TYPE").unwrap_or_else(|_| "FEATURE".into());
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

    let use_neon = env::var("APP_ENV").is_ok() || env::var("VERCEL_ENV").is_ok();
    let code = if use_neon {
        match NeonStore::connect_from_env() {
            Ok(store) => {
                eprintln!("workflow store=neon");
                drive(store, release, &harness, &story, &work_type)
            }
            Err(e) => {
                eprintln!("neon store: {e}");
                1
            }
        }
    } else {
        eprintln!("workflow store=memory (APP_ENV unset)");
        drive(MemoryStore::new(), release, &harness, &story, &work_type)
    };
    std::process::exit(code);
}

fn drive<S: TxStore>(
    store: S,
    release: Arc<dyn ForgeReleaseExecutor>,
    harness: &OpenCodeHarness,
    story: &str,
    work_type: &str,
) -> i32 {
    let rt = match ForgeRuntime::from_store(
        store,
        Arc::new(NullWriter),
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
        &rt,
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
