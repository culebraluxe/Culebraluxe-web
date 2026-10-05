//! Cutover host. Engine + Forge + OpenCode.
//! NeonStore when APP_ENV / VERCEL_ENV is set. MemoryStore only for local dry-run.

mod test_support {
    /// A human GATE ends a turn with no blocked reason and settles `Done` (the pair accepts `Done` over `Hold`). A hold
    /// that carries a reason is the engine refusing to go on after a failure, and must settle `Error`, or its run is
    /// closed `Complete` at 100%.
    pub fn failure_hold_reason(needs_human: bool, blocked_reason: Option<String>) -> Option<String> {
        blocked_reason.filter(|reason| needs_human && !reason.trim().is_empty())
    }
}

use db::{AgentWorkOutcome, AgentWorkSettlement};
use forge::engine::agent_work;
use forge::engine::db_writer::{DbForgeEvidenceReader, DbForgeStateWriter};
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story_with_jobs, parse_forge_stop_after, DriveForgeStoryOptions,
    DurableForgeExecution, ForgeStopTarget,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::git_publish::{publish_switch_off, GitReleaseOps, HostReleaseExecutor};
use forge::engine::job::WorkflowJobService;
use forge::engine::runner::RoleHarness;
use forge::engine::spend_cap::{self, SPEND_CAP_ENV};
use forge::engine::packet::{ExecutionWorkspace, StoryPacket};
use forge::engine::re_runtime::shared_forge_runtime;
use forge::engine::runner::ProductionRoleRunner;
use forge::engine::runtime::ForgeRuntime;
use forge::engine::vendor_session::database_url;
use forge::engine::worktree::{
    provision_worker_workspace, resolve_approved_base_ref, resolve_base_commit, resolve_repo_root,
};
use forge::engine::writer::{
    ForgeEvidenceReader, ForgeReleaseExecutor, ForgeStateWriter, NullWriter,
};
use forge::roles::ForgeLaneServices;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use workflow::{MemoryStore, TxStore, WorkflowError};

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
            if let Some(status) = &settled.story_status {
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
                reject_configuration(work_item.as_deref(), &format!("invalid --stop-after {raw}"));
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
    // THE VENDOR IS SELECTED BEFORE IT IS VERIFIED — never the reverse. The backend decides which
    // vendor contract gets checked: OpenCode's for OpenCode, Maestro's for Maestro. Verifying OpenCode
    // unconditionally would make every Maestro turn depend on the direct OpenCode harness passing (§9).
    let harness_backend = match forge::engine::harness::HarnessBackend::from_env() {
        Ok(backend) => backend,
        Err(e) => {
            reject_configuration(work_item.as_deref(), &format!("{e}"));
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    eprintln!("harness_backend={:?}", harness_backend);
    // THE VENDOR BINARY IS RESOLVED AND VERIFIED BEFORE ANY CLAIM IS OPENED — not in the middle of a turn.
    // MEASURED 2026-10-03: `ENG-FORGE-C1-BUILD-INFO-01` opened its claim, dispatched `architect`, and died on the
    // vendor's own help text (exit 1) because `opencode` on this process's PATH was the npm `opencode-ai` 1.18.26 —
    // a different CLI from the v2 build Forge's argument list is written against (`jobs.last_error` for durable job
    // `b319bf40` is that help page, verbatim). A lane that cannot run the vendor it was written for has nothing to
    // dispatch, and learning that here costs one `--help` call instead of a claim, a run and a story's Hold.
    let vendor = match harness_backend {
        forge::engine::harness::HarnessBackend::OpenCode => {
            forge::engine::opencode_client::verify_vendor_contract(
                &forge::engine::opencode::default_cli_bin(),
            )
        }
        forge::engine::harness::HarnessBackend::Maestro => {
            forge::engine::maestro::verify_vendor_contract(
                &forge::engine::maestro::MaestroHarness::default_cli_bin()
                    .unwrap_or_else(|e| e.to_string()),
            )
        }
    };
    match vendor {
        Ok(vendor) => eprintln!("vendor={vendor}"),
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
    // THE STORY PACKET IS READ BEFORE THE CLAIM, NOT AFTER IT. Migration 024 §2 snapshots the specification into
    // `storyboard_story_run` **when execution begins**, and execution begins at the claim — so by the time the run
    // row exists, the packet has to be in hand. The run's specification is copied from `storyboard_story` by
    // `begin_agent_work_run` itself (nothing is passed in), but the packet is what the *model* reads, and a lane
    // that takes a queue row and only then learns it cannot read its packet has burned a claim to find out.
    //
    // An unreadable packet is the engine's plumbing failing, not a story verdict: no run is opened, no model turn
    // happens, and the claim goes back to the queue. The environment packet stays reachable for an attended,
    // deliberate run only, behind FORGE_PACKET_FROM_ENV=1.
    let packet_from_env = env::var("FORGE_PACKET_FROM_ENV").ok().as_deref() == Some("1");
    let packet = match StoryPacket::load_from_neon(&story) {
        Ok(packet) => {
            eprintln!("packet from storyboard_story {}", packet.id);
            Some(packet)
        }
        Err(e) if packet_from_env => {
            eprintln!(
                "story packet: {e} (FORGE_PACKET_FROM_ENV=1: running on the environment packet)"
            );
            None
        }
        Err(e) => {
            eprintln!(
                "story packet: {e}; refusing to run {story} without its authoritative packet"
            );
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
    // Test authoring is a Storyboard contract, not a story-id convention. Carry the declared mode into the
    // deterministic QA lane so RUST_CONTRACT can judge the authored test without pretending the application must
    // already satisfy the newly-written assertion.
    let test_mode = packet.as_ref().and_then(|packet| packet.test_mode.clone());
    let contract_assay_commands = packet
        .as_ref()
        .map(|packet| packet.assay_commands.clone())
        .unwrap_or_default();
    let contract_acceptance_mapped = packet
        .as_ref()
        .and_then(|packet| {
            packet.acceptance_criteria.as_deref().map(|criteria| {
                !criteria.trim().is_empty()
                    && !packet.assay_commands.is_empty()
                    && packet
                        .assay_commands
                        .iter()
                        .all(|command| criteria.contains(command))
            })
        })
        .unwrap_or(false);
    let mut story_run_id: Option<String> = None;
    // The row's dispatch envelope, carried on from the claim to the lane it configures.
    let mut run_model_policy: Option<String> = None;
    let mut run_launch_intent: Option<String> = None;
    if let Some(item) = work_item.as_deref() {
        match agent_work::begin_agent_work_run(item) {
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
    // The backend was selected and verified above, before the claim; the model the lane bills is decided
    // by the ROW (migration 179 `model_policy`), read at the claim together with
    // the execution policy. `OPENCODE_MODEL` still wins: that is an explicit, attended configuration. Before this,
    // the model was whatever the pin said and the policy column was decoration.
    // (harness_backend is bound above, next to the vendor preflight it selects.)
    eprintln!("harness_backend={:?}", harness_backend);
    let harness_context = forge::engine::harness::HarnessContext {
        story_id: story.clone(),
        packet: packet.clone().unwrap_or_else(|| forge::engine::packet::StoryPacket::default()),
        workspace: std::env::var("FORGE_WORKTREE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        execution_workspace: match (
            std::env::var("FORGE_WORKTREE").ok(),
            std::env::var("FORGE_BRANCH").ok(),
            std::env::var("FORGE_BASE_REF").ok(),
            std::env::var("FORGE_BASE_COMMIT").ok(),
        ) {
            (Some(path), Some(branch), Some(base_ref), Some(base_commit)) => {
                Some(forge::engine::packet::ExecutionWorkspace {
                    worktree_path: path,
                    branch_name: branch,
                    base_ref,
                    base_commit,
                })
            }
            _ => None,
        },
        model_policy: run_model_policy.clone(),
        model_override: std::env::var("OPENCODE_MODEL").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        spend_cap_usd: spend_cap::parse_forge_spend_cap_usd(
            std::env::var(SPEND_CAP_ENV).ok().as_deref(),
        ),
        assay_commands: contract_assay_commands.clone(),
        acceptance_mapped: contract_acceptance_mapped,
    };
    let mut harness = match forge::engine::harness::create_harness(harness_backend, harness_context) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
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
        "harness_backend={:?} model={} model_policy={}",
        harness_backend,
        harness.model(),
        run_model_policy.as_deref().unwrap_or("(default cheap)")
    );
    // The packet read before the claim becomes the packet the lane acts on. On the FORGE_PACKET_FROM_ENV fallback
    // there is no packet row, so the environment packet is what stands — an attended, deliberate act.
    if let Some(packet) = packet {
        harness.set_packet_and_story_id(packet, story.clone());
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
                harness.set_workspace(ws.worktree_path.clone());
                harness.set_execution_workspace(ExecutionWorkspace {
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
    } else if let Some(run_id) = story_run_id.as_deref() {
        // Default path: no worktree. Stamp the integration base so the run receipt is not
        // missing `base_commit_hash` just because FORGE_PROVISION was off.
        match resolve_repo_root(env::current_dir().ok().as_deref())
            .and_then(|root| resolve_base_commit(&root, &resolve_approved_base_ref()))
        {
            Ok(base_commit) => match agent_work::stamp_run_base_commit(run_id, &base_commit) {
                Ok(true) => eprintln!("run base_commit_hash={base_commit} (in-repo)"),
                Ok(false) => {
                    eprintln!("run base_commit_hash left unset (already stamped or empty)")
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
                        eprintln!("work_item could not be settled; the claim is left to recovery");
                    }
                    std::process::exit(2);
                }
            },
            Err(e) => {
                eprintln!("base_commit_hash: {e}");
                if settle_work_item(
                    work_item.as_deref(),
                    AgentWorkOutcome::Abandoned,
                    Some(&format!("base_commit_hash: {e}")),
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
        ops: GitReleaseOps {
            repo_root: repo,
            integration_proofs: contract_assay_commands.clone(),
        },
    });
    eprintln!(
        "harness={} model={} bin={} cwd={} neon={}",
        forge::engine::opencode::OPENCODE_HARNESS_ADAPTER_ID,
        harness.model(),
        harness.cli_bin(),
        harness.workspace().display(),
        database_url().is_some()
    );
    // Said before a token is spent, not discovered afterwards in an evidence row. Whether this run may publish
    // is the entire difference between a candidate that lands on `origin/main` and one that strands on a branch
    // nobody may push — House Rule 1 (`.githooks/pre-push`) refuses every ref but `main`, so there is no
    // fallback route off this machine for a candidate the composer refuses.
    eprintln!(
        "publish={} (FORGE_ALLOW_PUBLISH={})",
        if publish_switch_off(env::var("FORGE_ALLOW_PUBLISH").ok().as_deref()) {
            "off"
        } else {
            "on"
        },
        env::var("FORGE_ALLOW_PUBLISH").unwrap_or_else(|_| "(unset)".into())
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
            None,
            &*harness,
            &story,
            &work_type,
            stop_after.clone(),
            story_run_id.clone(),
            run_launch_intent.clone(),
            test_mode.clone(),
            contract_assay_commands.clone(),
            contract_acceptance_mapped,
        )
    } else {
        eprintln!("workflow store=neon (shared engine)");
        drive_with_shared_runtime(
            release,
            writer.clone(),
            Some(Arc::new(DbForgeEvidenceReader)),
            &*harness,
            &story,
            &work_type,
            stop_after.clone(),
            story_run_id.clone(),
            run_launch_intent.clone(),
            test_mode.clone(),
            contract_assay_commands.clone(),
            contract_acceptance_mapped,
        )
    };
    // One exit, one verdict. `Ok` means the story was driven through its turn (a story that stopped for a human comes
    // back `Ok` and the board's Hold says so); `Err` means the run failed. Either way the claim is settled here, not
    // left for stale recovery to guess about.
    match result {
        // A hold the engine took because the WORK failed (a role refused, its durable job is terminal) is that
        // failure's verdict, not a finished run. Settled `Done`, the pair accepted it over the `Hold` board and closed
        // the run `Complete` at 100% — every crashed role turn read as finished work. `Error` holds the board as it
        // is and closes the run `Failed` with the reason.
        Ok(DriveSummary {
            line,
            failure_hold: Some(reason),
        }) => {
            println!("{line}");
            eprintln!("work_item ended on a failure hold: {reason}");
            if settle_work_item(work_item.as_deref(), AgentWorkOutcome::Error, Some(&reason))
                .is_err()
            {
                eprintln!("work_item could not be settled; the claim is left to recovery");
            }
            std::process::exit(1);
        }
        Ok(DriveSummary { line: summary, .. }) => {
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
            let fault = forge::engine::engine_fault::is_engine_fault_error(&error);
            let error = error.to_string();
            // An engine fault is not the story's verdict. When the failure is the plumbing - the session was taken
            // away, the transport died, a statement was cut off - nothing about the story was decided, so the claim
            // is cleared back into the queue and the story keeps its turn (captain, 2026-09-29). Only a failure that
            // is about the work is recorded against it.
            // The verdict is read off the TYPED error (a connection failure needs no reading) before it is flattened
            // to the text the row records.
            let outcome = if fault {
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

/// What one drive reports to the exit path: the operator's summary line, and — when the engine held the story because
/// the work failed — the reason, which the exit settles as `Error` rather than `Done`.
struct DriveSummary {
    line: String,
    failure_hold: Option<String>,
}

fn drive<S: TxStore>(
    store: S,
    release: Arc<dyn ForgeReleaseExecutor>,
    writer: Arc<dyn ForgeStateWriter>,
    evidence_reader: Option<Arc<dyn ForgeEvidenceReader>>,
    harness: &dyn RoleHarness,
    story: &str,
    work_type: &str,
    stop_after: Option<ForgeStopTarget>,
    story_run_id: Option<String>,
    bench_intent: Option<String>,
    test_mode: Option<String>,
    contract_assay_commands: Vec<String>,
    contract_acceptance_mapped: bool,
) -> Result<DriveSummary, WorkflowError> {
    let mut rt = match ForgeRuntime::from_store(
        store,
        writer.clone(),
        Some(release.clone()),
        evidence_reader.clone(),
        // The receipt row, not a process-local set: this binary IS a child process per dispatch, so a
        // memory ledger would let every new process re-apply each completion in the instance history
        // (2026-09-29). The ledger is a required argument for exactly this reason.
        forge::engine::durable_completion_ledger(),
        forge_sdlc_definition(),
    ) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("{e}");
            return Err(e);
        }
    };
    drive_with_runtime(
        &mut rt,
        release,
        writer,
        evidence_reader,
        harness,
        story,
        work_type,
        stop_after,
        story_run_id,
        bench_intent,
        test_mode,
        contract_assay_commands,
        contract_acceptance_mapped,
    )
}

/// Drive a story using a pre-built ForgeRuntime (production path with shared engine).
fn drive_with_shared_runtime(
    release: Arc<dyn ForgeReleaseExecutor>,
    writer: Arc<dyn ForgeStateWriter>,
    evidence_reader: Option<Arc<dyn ForgeEvidenceReader>>,
    harness: &dyn RoleHarness,
    story: &str,
    work_type: &str,
    stop_after: Option<ForgeStopTarget>,
    story_run_id: Option<String>,
    bench_intent: Option<String>,
    test_mode: Option<String>,
    contract_assay_commands: Vec<String>,
    contract_acceptance_mapped: bool,
) -> Result<DriveSummary, WorkflowError> {
    let rt = shared_forge_runtime(
        writer.clone(),
        Some(release.clone()),
        evidence_reader.clone(),
        forge::engine::durable_completion_ledger(),
    )?;
    drive_with_runtime(
        &rt,
        release,
        writer,
        evidence_reader,
        harness,
        story,
        work_type,
        stop_after,
        story_run_id,
        bench_intent,
        test_mode,
        contract_assay_commands,
        contract_acceptance_mapped,
    )
}

/// Common drive logic shared by both memory and shared-engine paths.
fn drive_with_runtime<S: TxStore>(
    rt: &ForgeRuntime<S>,
    release: Arc<dyn ForgeReleaseExecutor>,
    writer: Arc<dyn ForgeStateWriter>,
    evidence_reader: Option<Arc<dyn ForgeEvidenceReader>>,
    harness: &dyn RoleHarness,
    story: &str,
    work_type: &str,
    stop_after: Option<ForgeStopTarget>,
    story_run_id: Option<String>,
    bench_intent: Option<String>,
    test_mode: Option<String>,
    contract_assay_commands: Vec<String>,
    contract_acceptance_mapped: bool,
) -> Result<DriveSummary, WorkflowError> {
    let evidence = ForgeGateEvidence {
        work_type: Some(work_type.to_string()),
        scout_required: Some(false),
        ..Default::default()
    };
    let durable_worker_id = story_run_id
        .as_deref()
        .map(|run_id| format!("forge:{run_id}"))
        .unwrap_or_else(|| format!("forge:{story}"));
    let runner = ProductionRoleRunner::new(harness, evidence.clone())
        .with_writer(writer.as_ref())
        .with_story_run(story_run_id)
        .with_bench_intent(bench_intent)
        .with_test_mode(test_mode)
        .with_contract_assay_commands(contract_assay_commands)
        .with_contract_acceptance_mapped(contract_acceptance_mapped)
        .with_evidence_reader(evidence_reader.clone());
    // Every canonical Forge lane is composed here, in one place: each service owns its lane's identity,
    // its authority and its own reading of a turn (roles/smith.rs, roles/architect.rs, roles/qa.rs, …),
    // and inherits the shared execution lifecycle. Workflow still owns sequencing, JobService still owns
    // execution reliability, and no role policy lives in this binary.
    let services = ForgeLaneServices::new(&runner);
    let registry = services.registry()?;
    let jobs = WorkflowJobService::new(rt.engine());
    match drive_forge_story_with_jobs(
        rt,
        story,
        DriveForgeStoryOptions {
            work_type,
            evidence,
            // Durable production resolves the concrete service through
            // ForgeServiceRegistry. The compatibility runner is not a second
            // production dispatch path.
            runner: None,
            max_steps: 40,
            worker_id: &durable_worker_id,
            split_concurrency: 1,
            stop_after,
            // The operator's ceiling, from the environment. Read here rather than in the loop so the cap a run is
            // held to is fixed for the whole generation.
            turn_cap: DriveForgeStoryOptions::turn_cap_from_env(),
        },
        DurableForgeExecution {
            jobs: &jobs,
            registry: &registry,
        },
    ) {
        Ok(out) => Ok(DriveSummary {
            line: format!(
                "instance={} status={} steps={:?} human={} stopped={:?} reconciled={}",
                out.instance_id,
                out.status,
                out.steps,
                out.needs_human,
                out.stopped_after,
                out.reconciled
            ),
            failure_hold: test_support::failure_hold_reason(out.needs_human, out.blocked_reason),
        }),
        Err(e) => {
            eprintln!("{e}");
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::failure_hold_reason;

    /// A human GATE (held, no reason) is a finished turn and settles `Done`; a hold that carries a reason is the
    /// engine refusing to go on after a failure and must settle `Error`, or its run is closed `Complete` at 100%.
    #[test]
    fn only_a_hold_with_a_reason_is_a_failure_hold() {
        assert_eq!(failure_hold_reason(true, None), None, "a human gate");
        assert_eq!(
            failure_hold_reason(
                true,
                Some("Forge role smith failed for task t: refused".into())
            ),
            Some("Forge role smith failed for task t: refused".into())
        );
        assert_eq!(
            failure_hold_reason(true, Some("  ".into())),
            None,
            "a blank reason says nothing"
        );
        assert_eq!(
            failure_hold_reason(false, Some("no ready task; active: smith=Reserved".into())),
            None,
            "an exhausted run is not a hold; the settlement pair already refuses its Done"
        );
    }
}
