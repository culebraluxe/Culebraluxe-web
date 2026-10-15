//! The claimed-dispatch tail, moved out of `worker.rs` (2026-10-14).
//!
//! `worker.rs` had grown past the boundedness ceiling. What moved is the part that only runs once a claim is in
//! hand: the child's settlement reporting, the child's shutdown, and the dispatch itself. The move is
//! text-exact (`pub(super)` is the only edit) and the pass still calls `run_claimed_dispatch` by that name.

use super::*;

/// Report what a fenced settle answered.
///
/// Only `Settled` is a verdict this call wrote. Every other answer is a fact ABOUT the claim — a successor holds it,
/// this execution already settled it, a different outcome is already stored, the item is gone — and printing those
/// as settlements is how a lost claim came to look like a settled one.
pub(super) fn report_settlement(
    item: &str,
    when: &str,
    answer: Result<db::SettlementResult, String>,
) {
    match answer {
        Ok(answer) => {
            let pair = answer
                .settlement()
                .map(|pair| {
                    format!(
                        " item={} story={}",
                        pair.item_state,
                        pair.story_status.as_deref().unwrap_or("unchanged")
                    )
                })
                .unwrap_or_default();
            eprintln!("forge-worker: {item} {when}: {}{pair}", answer.name());
        }
        Err(error) => eprintln!("forge-worker: {item} {when} could not be written: {error}"),
    }
}

/// Stop a running child and everything it started.
///
/// The child is spawned in its own process group (`command.process_group(0)`), so signalling the GROUP reaches the
/// `cargo run` wrapper and the engine it launched. Killing only the wrapper would leave the engine running — holding
/// a claim it no longer has and writing to the same story — which is the state a lease-loss stop exists to prevent.
pub(super) fn stop_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        match std::process::Command::new("kill")
            .args(["-9", &format!("-{}", child.id())])
            .status()
        {
            Ok(status) if status.success() => {}
            // The group may already be gone (the child exited between the poll and here), which is not a failure.
            Ok(status) => eprintln!("forge-worker: signalling child group returned {status}"),
            Err(error) => eprintln!("forge-worker: could not signal child group: {error}"),
        }
    }
    if let Err(error) = child.kill() {
        // ESRCH here is the ordinary race: the child died on its own between the poll and the signal.
        eprintln!("forge-worker: child kill returned {error}");
    }
    if let Err(error) = child.wait() {
        eprintln!("forge-worker: child could not be reaped: {error}");
    }
}

/// Run ONE claimed story to its end, in its own thread.
///
/// Everything below the claim lives here rather than in `run_worker_pass`, so the pass can hold several stories at
/// once: the policy fence, the heartbeat that keeps the claim out of stale recovery, the child process itself, and
/// the settlement of a child that left no verdict of its own. The claim is passed in rather than looked up, so this
/// function owns exactly the dispatch it was given — which is what makes it safe to run beside its peers.
pub(super) fn run_claimed_dispatch(
    dispatch: WorkerDispatch,
    worker_id: String,
    _stale_after_minutes: i64,
) -> Result<i32, String> {
    let _in_flight = InFlight::enter();
    let config = ForgeConfig::from_env();

    eprintln!(
        "forge-worker: claimed={} worker={} story={} work_type={} policy={} model_policy={} stop_after={} launch_intent={}",
        dispatch.work_item_id,
        worker_id,
        dispatch.story_id,
        dispatch.work_type,
        dispatch.execution_policy,
        dispatch.model_policy.as_deref().unwrap_or("(none)"),
        dispatch.stop_after.as_deref().unwrap_or("(full chain)"),
        dispatch
            .launch_intent
            .as_deref()
            .unwrap_or("(lead decides)")
    );

    // The durable envelope is read here, before the child exists, so a policy that names a human never reaches a
    // model. The claim goes back to the queue rather than being held against the story: no run happened.
    if !agent_work::execution_policy_allows_unattended(&dispatch.execution_policy)
        && !config.attended_override
    {
        let reason = format!(
            "execution_policy={} requires a human; refusing to dispatch {} unattended",
            dispatch.execution_policy, dispatch.work_item_id
        );
        eprintln!("forge-worker: {reason} (set FORGE_ATTENDED=1 for a deliberate, attended run)");
        report_settlement(
            &dispatch.work_item_id,
            "refused an unattended dispatch",
            agent_work::finish_agent_work_run(
                &dispatch.work_item_id,
                &dispatch.claim,
                AgentWorkOutcome::Abandoned,
                Some(&reason),
            ),
        );
        return Ok(0);
    }

    // The claim is only worth holding if it stays fresh for as long as the run lasts — and only while it is still
    // ours. The beat is fenced by (owner, generation), so it reports the moment a reclaim or a settle ends this
    // dispatch's authority, and the child is stopped rather than left writing under a claim it no longer has.
    let heartbeat = spawn_heartbeat(
        dispatch.work_item_id.clone(),
        dispatch.claim.clone(),
        config.heartbeat_interval,
    );
    let claim_generation = dispatch.claim.generation.to_string();

    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--manifest-path",
        "Cargo.toml",
        "-p",
        "forge",
        "--bin",
        "forge",
        "--",
        "--story",
        &dispatch.story_id,
        "--work-type",
        &dispatch.work_type,
        "--work-item",
        &dispatch.work_item_id,
        // The child is a different process: it must be told which authority it is executing under, or its begin and
        // its settlement would have to guess — and guessing is what the fence removes.
        "--claim-owner",
        &dispatch.claim.owner,
        "--claim-generation",
        &claim_generation,
    ]);
    // The dispatch cap travels with the dispatch (migration 167: "read by the engine worker when it claims the
    // item"). It is omitted when the column is NULL, which is the full chain — the child's own default.
    if let Some(stop_after) = dispatch.stop_after.as_deref() {
        command.args(["--stop-after", stop_after]);
    }
    command
        .env(
            "APP_ENV",
            std::env::var("APP_ENV").unwrap_or_else(|_| "production".into()),
        )
        .env(
            "EXECUTION_ENV",
            std::env::var("EXECUTION_ENV").unwrap_or_else(|_| "PROD".into()),
        )
        // Parallel stories MUST NOT share the control-plane checkout. The child provisions one disposable
        // /tmp worktree from origin/main, keyed by this durable work-item id.
        .env("FORGE_PROVISION", "1")
        .env("FORGE_RUN_ID", &dispatch.work_item_id)
        .env(
            "FORGE_ALLOW_PUBLISH",
            if config.allow_publish { "1" } else { "0" },
        )
        // THE FLOOR BELONGS TO THE COORDINATOR, NOT TO EVERY CHILD. Each story runs in its own process with its own
        // pool, so four concurrent stories must not each hold the engine's warm floor open against one Neon branch
        // (`FORGE_DB_POOL_MIN`, default 20 in `db/src/pool.rs:193`). The children are short-lived and
        // single-story: MIN=0 means no warm floor, connections open on first use up to MAX=6 per child.
        // With 4 concurrent stories this is at most 24 connections against one Neon branch.
        .env("FORGE_DB_POOL_MIN", config.db_pool_min.to_string())
        .env("FORGE_DB_POOL_MAX", config.db_pool_max.to_string());
    // THE WALL CLOCK IS A HARD LIMIT (S4). A story that hangs must not hold its claim and its worker
    // slot forever; a timeout settles the run as a timeout and frees both. The whole process group is
    // signalled so the grandchild engine dies with the cargo wrapper.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let run_timeout = Duration::from_secs(
        std::env::var("FORGE_RUN_TIMEOUT_SECS")
            .ok()
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .unwrap_or(3600),
    );
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let reason = format!("launch Rust Forge engine: {error}");
            report_settlement(
                &dispatch.work_item_id,
                "could not launch the child",
                agent_work::finish_agent_work_run(
                    &dispatch.work_item_id,
                    &dispatch.claim,
                    AgentWorkOutcome::Abandoned,
                    Some(&reason),
                ),
            );
            heartbeat.stop();
            eprintln!("{}", heartbeat.report(&dispatch.work_item_id));
            return Err(reason);
        }
    };
    let started = std::time::Instant::now();
    let status = loop {
        // LOST AUTHORITY STOPS THE CHILD. A beat that came back "no longer claimable" means somebody else owns this
        // item now: the successor may already be provisioning a worktree for it, and every write this child makes
        // under the old authority is a second writer on one story. The child is signalled and reaped here, and this
        // thread settles nothing — the claim is not its to end.
        if let Some(reason) = lease_loss_reason(&heartbeat.snapshot(), &dispatch.work_item_id) {
            eprintln!(
                "forge-worker: claim for {} is no longer held by {} generation {}; stopping its child",
                dispatch.work_item_id, dispatch.claim.owner, dispatch.claim.generation
            );
            stop_child(&mut child);
            heartbeat.stop();
            eprintln!("{}", heartbeat.report(&dispatch.work_item_id));
            return Err(reason);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= run_timeout {
                    eprintln!(
                        "forge-worker: run for {} exceeded {}s; killing process group",
                        dispatch.work_item_id,
                        run_timeout.as_secs()
                    );
                    stop_child(&mut child);
                    let reason =
                        format!("FORGE_RUN_TIMEOUT_SECS={} elapsed", run_timeout.as_secs());
                    report_settlement(
                        &dispatch.work_item_id,
                        "timed out",
                        agent_work::finish_agent_work_run(
                            &dispatch.work_item_id,
                            &dispatch.claim,
                            AgentWorkOutcome::Abandoned,
                            Some(&reason),
                        ),
                    );
                    heartbeat.stop();
                    eprintln!("{}", heartbeat.report(&dispatch.work_item_id));
                    return Err(reason);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(error) => {
                heartbeat.stop();
                eprintln!("{}", heartbeat.report(&dispatch.work_item_id));
                return Err(format!("wait on forge child: {error}"));
            }
        }
    };
    heartbeat.stop();
    // The heartbeat's own report, on the ordinary path too: beats, a refused beat, a failed beat and a panicked
    // thread are supervision outcomes, and a pass that never prints them is a pass nobody can audit.
    eprintln!("{}", heartbeat.report(&dispatch.work_item_id));

    // A worktree is an execution sandbox, not workflow state. Remove it after every child run. The cleanup helper
    // keeps the branch only when its candidate is not yet contained in origin/main, so a Hold cannot erase paid code.
    if let Err(error) = cleanup_worker_workspace(
        std::env::current_dir().ok().as_deref(),
        &dispatch.story_id,
        &dispatch.work_item_id,
        std::env::var("FORGE_WORKTREES_ROOT")
            .ok()
            .as_deref()
            .map(Path::new),
    ) {
        eprintln!(
            "forge-worker: worktree cleanup failed story={} item={}: {error}",
            dispatch.story_id, dispatch.work_item_id
        );
    }

    // The child settles its own run. This is the net under a child that died before it could (crash, kill, OOM):
    // the guard inside `finish_agent_work_run` makes it a no-op if the child already has a verdict, so it can
    // only ever replace a hung `Running` row, never a real one.
    //
    // Reaching here therefore means exactly one thing: **the child left no verdict**. That is never a story's
    // failure - a story that failed says so in the row it writes - so the claim is cleared back into the queue
    // (`Abandoned`) rather than held. This is the case that cost hours on 2026-09-29: a failed engine ended the
    // story in a human `Hold` nobody had decided.
    if !status.success() {
        let reason = format!("forge run exited with {}", status.code().unwrap_or(1));
        report_settlement(
            &dispatch.work_item_id,
            "left no verdict",
            agent_work::finish_agent_work_run(
                &dispatch.work_item_id,
                &dispatch.claim,
                AgentWorkOutcome::Abandoned,
                Some(&reason),
            ),
        );
    }

    Ok(status.code().unwrap_or(1))
}
