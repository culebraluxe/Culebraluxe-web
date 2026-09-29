//! Cutover host. Engine + Forge + OpenCode.
//! NeonStore when APP_ENV / VERCEL_ENV is set. MemoryStore only for local dry-run.

use db::{AgentWorkOutcome, AgentWorkSettlement};
use forge::engine::agent_work;
use forge::engine::db_writer::DbForgeStateWriter;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story, parse_forge_stop_after, DriveForgeStoryOptions, ForgeStopTarget,
};
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

/// The run's own terminal write, on the way out of `main`: the item **and its story**, decided together in the
/// database (`db::settlement_pair`).
///
/// The result is returned, not only printed. A terminal write that fails has to change the exit status: the worker
/// settles a child only when it exits non-zero, and stale recovery then requeues whatever is left — which is how a
/// claim whose work had already landed could be handed to a second run. A verdict this process failed to write must
/// not look like a verdict it wrote.
fn settle_work_item(
    work_item: Option<&str>,
    outcome: AgentWorkOutcome,
    reason: Option<&str>,
) -> Result<Option<AgentWorkSettlement>, String> {
    let Some(item) = work_item else {
        return Ok(None);
    };
    match agent_work::finish_agent_work_run(item, outcome, reason) {
        Ok(Some(settled)) => {
            eprintln!("work_item={item} state={}", settled.item_state);
            if let Some(status) = settled.story_status {
                eprintln!("work_item={item} story set to {status} with its item");
            }
            if let Some(ref refusal) = settled.reason {
                eprintln!("work_item={item} {refusal}");
            }
            Ok(Some(settled))
        }
        Ok(None) => {
            eprintln!("work_item={item} already had a verdict; left as-is");
            Ok(None)
        }
        Err(error) => {
            eprintln!("work_item={item} could not be settled: {error}");
            Err(error)
        }
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
    // The dispatch cap the worker carried off the claimed row (migration 167). An unrecognised value is refused
    // rather than widened: a cap that cannot be read must not become "run the whole chain".
    let stop_after = match flag(&args, "--stop-after")
        .or_else(|| env::var("FORGE_STOP_AFTER").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        None => None,
        Some(raw) => match parse_forge_stop_after(&raw) {
            Some(target) => Some(target),
            None => {
                reject_configuration(
                    work_item.as_deref(),
                    &format!("invalid --stop-after {raw}"),
                );
                eprintln!("invalid --stop-after {raw}: expected scout|architect|lead");
                std::process::exit(2);
            }
        },
    };
    match forge::engine::execution_target::assert_forge_lane_may_start(
        &forge::engine::execution_target::env_pairs_from_process(),
    ) {
        Ok(target) => eprintln!("execution_target={target}"),
        Err(e) => {
            reject_configuration(work_item.as_deref(), &format!("{e}"));
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
    // Only now is this run real, so only now does it go `Running`. If the claim cannot be opened the run must not
    // start at all: a story driven without a claim is exactly the unowned dispatch this seam exists to remove.
    //
    // The run this claim opens is carried on: every artifact a lane produces is keyed to it (migration 130), so the
    // id has to reach the runner that writes them.
    // THE STORY PACKET IS READ BEFORE THE CLAIM, NOT AFTER IT. Migration 024 §2 has the specification snapshotted
    // into `storyboard_story_run` **when execution begins**, and execution begins at the claim — so the packet has
    // to be in hand by then. Loading it after the claim (which is what this did until 2026-09-29) meant every run
    // opened with a NULL snapshot: the run row existed and what it was executing was not in it.
    //
    // An unreadable packet is the engine's plumbing failing, not a story verdict: no run is opened, no model turn
    // happens, and the claim goes back to the queue. The environment packet stays reachable for an attended,
    // deliberate run only, behind FORGE_PACKET_FROM_ENV=1 — and a run on that fallback snapshots NO specification,
    // because there is no authoritative packet behind it to snapshot.
    let packet_from_env = env::var("FORGE_PACKET_FROM_ENV").ok().as_deref() == Some("1");
    let packet = match StoryPacket::load_from_neon(&story) {
        Ok(packet) => {
            eprintln!("packet from storyboard_story {}", packet.id);
            Some(packet)
        }
        Err(e) if packet_from_env => {
            eprintln!("story packet: {e} (FORGE_PACKET_FROM_ENV=1: running on the environment packet)");
            None
        }
        Err(e) => {
            eprintln!("story packet: {e}; refusing to run {story} without its authoritative packet");
            if settle_work_item(
                work_item.as_deref(),
                AgentWorkOutcome::Abandoned,
                Some(&format!("story packet: {e}")),
            )
            .is_err()
            {
                eprintln!("work_item could not be settled; the claim is left to recovery");
            }
            std::process::exit(2);
        }
    };
    // The specification the run opens with, read from the story row before the claim takes it — the one moment
    // the engine still holds it (migration 025: `agent_work_item` stores no specification).
    let snapshot = if packet.is_some() {
        match agent_work::story_run_snapshot(&story) {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                eprintln!("story specification: {e}; refusing to open a run with no snapshot");
                if settle_work_item(
                    work_item.as_deref(),
                    AgentWorkOutcome::Abandoned,
                    Some(&format!("story specification: {e}")),
                )
                .is_err()
                {
                    eprintln!("work_item could not be settled; the claim is left to recovery");
                }
                std::process::exit(2);
            }
        }
    } else {
        None
    };
    let mut story_run_id: Option<String> = None;
    // The row's dispatch envelope, carried on from the claim to the lane it configures.
    let mut run_model_policy: Option<String> = None;
    let mut run_launch_intent: Option<String> = None;
    if let Some(item) = work_item.as_deref() {
        match agent_work::begin_agent_work_run(item, snapshot.as_ref()) {
            Ok(Some(begin)) => {
                let policy = begin.execution_policy.clone();
                story_run_id = Some(begin.story_run_id.clone());
                run_model_policy = begin.model_policy.clone();
                run_launch_intent = begin.launch_intent.clone();
                // The Story Run this claim opened, with the envelope it was started under. Every durable artifact
                // the lane produces is keyed to the run, so a run whose id is never printed cannot be followed.
                eprintln!(
                    "story_run={} policy={policy} model_policy={} launch_intent={}",
                    begin.story_run_id,
                    run_model_policy.as_deref().unwrap_or("(default cheap)"),
                    run_launch_intent.as_deref().unwrap_or("(lead decides)")
                );
                // The durable envelope is read at the moment the run starts (migration 029: "only 'Unattended OK'
                // work may be claimed by the unattended poller"). A policy that names a human is a rail, not a
                // note: no model turn happens and the claim goes back to the queue.
                if !agent_work::execution_policy_allows_unattended(&policy)
                    && env::var("FORGE_ATTENDED").ok().as_deref() != Some("1")
                {
                    let reason = format!(
                        "execution_policy={policy} requires a human; refusing to run {item} unattended"
                    );
                    eprintln!("{reason} (set FORGE_ATTENDED=1 for a deliberate, attended run)");
                    if settle_work_item(
                        work_item.as_deref(),
                        AgentWorkOutcome::Abandoned,
                        Some(&reason),
                    )
                    .is_err()
                    {
                        eprintln!("work_item could not be settled; the claim is left to recovery");
                    }
                    std::process::exit(2);
                }
                eprintln!("work_item={item} state=Running policy={policy}");
            }
            // The row was not `Claimed`: it already settled, or was cancelled, or recovery requeued it. The claim is
            // not ours, so the story must not be driven — and the row must not be touched either, because settling a
            // claim we do not own is how a second writer gets on to a live story.
            Ok(None) => {
                eprintln!(
                    "work_item={item} is not Claimed; refusing to run a story whose claim this process does not own"
                );
                std::process::exit(2);
            }
            Err(error) => {
                eprintln!("cannot begin work item {item}: {error}");
                std::process::exit(2);
            }
        }
    }
    let brain = forge::engine::routing_brain::parse_forge_routing_brain(
        env::var("FORGE_ROUTING_BRAIN").ok().as_deref(),
    );
    eprintln!("routing-brain={brain:?}");
    // The model the lane bills is decided by the ROW (migration 179 `model_policy`), read at the claim together with
    // the execution policy. `OPENCODE_MODEL` still wins: that is an explicit, attended configuration. Before this,
    // the model was whatever the pin said and the policy column was decoration.
    let mut harness = match OpenCodeHarness::from_env_for_policy(run_model_policy.as_deref()) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
            // An unusable harness is the engine's own plumbing, not the story's verdict: nothing was attempted, so
            // the claim is cleared back into the queue (captain, 2026-09-29).
            if settle_work_item(
                work_item.as_deref(),
                AgentWorkOutcome::Abandoned,
                Some(&format!("{e}")),
            )
            .is_err()
            {
                eprintln!("work_item could not be settled; the claim is left to recovery");
            }
            std::process::exit(2);
        }
    };
    eprintln!(
        "model={} model_policy={}",
        harness.model,
        run_model_policy.as_deref().unwrap_or("(default cheap)")
    );
    // The packet read before the claim becomes the packet the lane acts on. On the FORGE_PACKET_FROM_ENV fallback
    // there is no packet row, so the environment packet is what stands — an attended, deliberate act.
    if let Some(packet) = packet {
        harness.packet = packet;
        harness.story_id = Some(story.clone());
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
                    base_commit: ws.base_commit.clone(),
                });
                // The base the run branched from is known only now, so it is stamped on to the run it belongs to the
                // moment provisioning answers (migration 106 `base_commit_hash`). A run whose base is unknown cannot
                // be read against the commit it produced, so a failure here is the engine's own plumbing failing:
                // the claim goes back rather than running on with a hole in the receipt.
                if let Some(run_id) = story_run_id.as_deref() {
                    match agent_work::stamp_run_base_commit(run_id, &ws.base_commit) {
                        Ok(true) => eprintln!("run base_commit_hash={}", ws.base_commit),
                        Ok(false) => {
                            eprintln!("run base_commit_hash left unset (provision reported no base commit)")
                        }
                        Err(e) => {
                            eprintln!("base_commit_hash: {e}");
                            if settle_work_item(
                                work_item.as_deref(),
                                AgentWorkOutcome::Abandoned,
                                Some(&format!("base_commit_hash: {e}")),
                            )
                            .is_err()
                            {
                                eprintln!(
                                    "work_item could not be settled; the claim is left to recovery"
                                );
                            }
                            std::process::exit(2);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("provision: {e}");
                // A workspace that could not be provisioned is the engine's own plumbing: the story never ran.
                if settle_work_item(
                    work_item.as_deref(),
                    AgentWorkOutcome::Abandoned,
                    Some(&format!("provision: {e}")),
                )
                .is_err()
                {
                    eprintln!("work_item could not be settled; the claim is left to recovery");
                }
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
    // THE STORE IS NEON UNLESS ASKED FOR BY NAME. The guard above has already refused anything that is not a
    // production run, so the store was never really optional — and keying it off `APP_ENV` being *set* was its own
    // hazard: an environment that declared `EXECUTION_ENV=PROD` while leaving `APP_ENV` unset would read a memory
    // store while the story writer below wrote the production row. Two stores, one run, and the one that answered
    // the engine's questions was not the one holding the record. `FORGE_STORE=memory` keeps the local dry run
    // reachable, deliberately, by name.
    let use_memory = env::var("FORGE_STORE")
        .map(|value| value.trim().eq_ignore_ascii_case("memory"))
        .unwrap_or(false);
    let result = if use_memory {
        eprintln!("workflow store=memory (FORGE_STORE=memory; local dry run only)");
        drive(
            MemoryStore::new(),
            release,
            writer.clone(),
            &harness,
            &story,
            &work_type,
            stop_after.clone(),
            story_run_id.clone(),
            run_launch_intent.clone(),
        )
    } else {
        match NeonStore::connect_from_env() {
            Ok(store) => {
                eprintln!("workflow store=neon");
                drive(
                    store,
                    release,
                    writer.clone(),
                    &harness,
                    &story,
                    &work_type,
                    stop_after.clone(),
                    story_run_id.clone(),
                    run_launch_intent.clone(),
                )
            }
            Err(e) => Err(format!("neon store: {e}")),
        }
    };
    // One exit, one verdict. `Ok` means the story was driven through its turn (a story that stopped for a human comes
    // back `Ok` and the board's Hold says so); `Err` means the run failed. Either way the claim is settled here, not
    // left for stale recovery to guess about.
    match result {
        Ok(summary) => {
            println!("{summary}");
            let settled = settle_work_item(work_item.as_deref(), AgentWorkOutcome::Done, None);
            match settled {
                // The engine returns `Ok` for runs the board does not call finished — `exhausted`, the step cap, a
                // wave blocked on a missing ready task. The pair refuses `Done` for those and records `Error` with
                // the reason, and the exit status has to say what the row says.
                Ok(Some(pair)) if pair.item_state != "Done" => {
                    eprintln!(
                        "work_item ended {}: the board did not confirm completion, so this is not a finished run",
                        pair.item_state
                    );
                    std::process::exit(1);
                }
                // `Done` written, a claim someone else already settled, or no claim at all to settle: over.
                Ok(_) => std::process::exit(0),
                // A verdict this process could not write is not a verdict: non-zero exit hands the row to the
                // worker's fallback and, after it, to stale recovery.
                Err(_) => std::process::exit(1),
            }
        }
        Err(error) => {
            eprintln!("{error}");
            // An engine fault is not the story's verdict. When the failure is the plumbing - the session was taken
            // away, the transport died, a statement was cut off - nothing about the story was decided, so the claim
            // is cleared back into the queue and the story keeps its turn (captain, 2026-09-29). Only a failure that
            // is about the work is recorded against it.
            let outcome = if forge::engine::engine_fault::is_engine_fault(&error) {
                eprintln!(
                    "work_item is cleared back into the queue: the engine failed, not the story"
                );
                AgentWorkOutcome::Abandoned
            } else {
                AgentWorkOutcome::Error
            };
            if settle_work_item(work_item.as_deref(), outcome, Some(&error)).is_err() {
                eprintln!("work_item could not be settled; the claim is left to recovery");
            }
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
    stop_after: Option<ForgeStopTarget>,
    story_run_id: Option<String>,
    bench_intent: Option<String>,
) -> Result<String, String> {
    let mut rt = match ForgeRuntime::from_store(
        store,
        writer.clone(),
        Some(release),
        None,
        // The receipt row, not a process-local set: this binary IS a child process per dispatch, so a
        // memory ledger would let every new process re-apply each completion in the instance history
        // (2026-09-29). The ledger is a required argument for exactly this reason.
        forge::engine::durable_completion_ledger(),
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
    let runner = ProductionRoleRunner::new(harness, evidence.clone())
        .with_story_run(story_run_id)
        .with_bench_intent(bench_intent);
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
            stop_after,
        },
    ) {
        Ok(out) => Ok(format!(
            "instance={} status={} steps={:?} human={} stopped={:?} reconciled={}",
            out.instance_id,
            out.status,
            out.steps,
            out.needs_human,
            out.stopped_after,
            out.reconciled
        )),
        Err(e) => {
            eprintln!("{e}");
            Err(format!("{e}"))
        }
    }
}
