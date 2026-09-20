//! Cutover host. Engine + Forge + OpenCode.

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{drive_forge_story, DriveForgeStoryOptions};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::git_publish::{GitReleaseOps, HostReleaseExecutor};
use forge::engine::opencode::OpenCodeHarness;
use forge::engine::packet::StoryPacket;
use forge::engine::runner::ProductionRoleRunner;
use forge::engine::runtime::ForgeRuntime;
use forge::engine::vendor_session::database_url;
use forge::engine::writer::NullWriter;
use std::env;
use std::sync::Arc;
use workflow::MemoryStore;

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
    let rt = match ForgeRuntime::from_store(
        MemoryStore::new(),
        Arc::new(NullWriter),
        Some(release),
        None,
        forge_sdlc_definition(),
    ) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let evidence = ForgeGateEvidence {
        work_type: Some(work_type.clone()),
        scout_required: Some(false),
        ..Default::default()
    };
    let runner = ProductionRoleRunner::new(&harness, evidence.clone());
    match drive_forge_story(
        &rt,
        &story,
        DriveForgeStoryOptions {
            work_type: &work_type,
            evidence,
            runner: Some(&runner),
            allow_synthetic_runner: false,
            max_steps: 40,
            worker_id: "forge",
            split_concurrency: 1,
            stop_after: None,
        },
    ) {
        Ok(out) => println!(
            "instance={} status={} steps={:?} human={} stopped={:?}",
            out.instance_id, out.status, out.steps, out.needs_human, out.stopped_after
        ),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
