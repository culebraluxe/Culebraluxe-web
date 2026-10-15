//! The story turn loop: the long inner driver that plans waves, runs role turns and decides when a story
//! needs a human. Moved out of `drive.rs` on 2026-10-14 to keep that file under the boundedness ceiling —
//! move-only apart from `pub(super)` on the function the two public entry points still call.
//!
//! It reads the same state the parent exposes and calls this crate's own batch, planning and gate helpers
//! through the parent's glob imports.

use super::*;

pub(super) fn drive_forge_story_inner<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    opts: DriveForgeStoryOptions<'_>,
    durable: Option<DurableForgeExecution<'_, '_>>,
) -> Result<DriveForgeStoryResult> {
    validate_story_concurrency(opts.within_story_concurrency, durable.is_some())?;
    if durable.is_none() && opts.runner.is_none() {
        return Err(WorkflowError::generic(
            "Direct Forge execution requires an explicit real role runner; there is no synthetic runner. Durable production execution resolves concrete services through JobService + ForgeServiceRegistry.",
        ));
    }
    let stop_target = resolve_forge_stop_target(opts.stop_after.as_ref());
    let mut stopped_after = None;
    let mut steps = Vec::new();
    let mut wave_index = 0usize;

    let wake = rt.wake_story(story_id, opts.work_type, opts.evidence.clone())?;
    let instance_id = wake.instance_id;
    let reconciled = wake.reconciled;

    if let Some(durable) = durable {
        durable.jobs.recover_stale(256)?;
    }

    for _ in 0..opts.max_steps {
        let tasks = rt.list_role_tasks(story_id)?;
        if tasks.is_empty() {
            break;
        }
        if note_human_gate(rt, story_id, &tasks)? {
            return Ok(ending(&instance_id, &steps, &stopped_after, reconciled)
                .needs_human(instance_status(rt, &instance_id)?, None));
        }
        if !tasks.iter().any(|t| t.status == TaskStatus::Ready) {
            return Ok(
                ending(&instance_id, &steps, &stopped_after, reconciled).exhausted(
                    instance_status(rt, &instance_id)?,
                    Some(no_ready_task_reason(&tasks)),
                ),
            );
        }

        let cap = opts
            .within_story_concurrency
            .clamp(1, turn_budget::MAX_WITHIN_STORY_CONCURRENCY);
        wave_index += 1;
        let plan = plan_next_wave(&tasks, cap, story_id, &instance_id, wave_index);
        for batch in plan.batches {
            if cap > 1 && batch.len() > 1 {
                let Some(durable) = durable else {
                    return Err(WorkflowError::conflict(
                        "FORGE_PARALLEL_LANES_UNAVAILABLE",
                        "parallel batches require durable job ownership",
                    ));
                };
                validate_parallel_execution(durable.registry)?;
                let prepared =
                    prepare_durable_batch(rt, story_id, &batch, durable, opts.worker_id)?;
                if prepared.is_empty() {
                    continue;
                }
                eprintln!(
                    "forge-wave story={} instance={} wave={} prepared_lanes={} configured_cap={}",
                    story_id,
                    instance_id,
                    wave_index,
                    prepared.len(),
                    cap
                );
                let turn_ceiling = turn_ceiling_from_env();
                let mut executions = execute_durable_batch(
                    prepared,
                    durable.jobs,
                    opts.worker_id,
                    turn_ceiling,
                    story_id,
                    wave_index,
                );
                executions.sort_by(|a, b| a.claimed.task.task_id.cmp(&b.claimed.task.task_id));
                let mut hold_reason: Option<String> = None;
                let mut engine_fault: Option<WorkflowError> = None;
                for mut execution in executions {
                    // The verdict channels are decided by one function (`WO-302` Part A) so a business
                    // Hold and a sibling engine fault cannot be weighed two ways. Classified before the
                    // outcome moves: the channels are owned, the error object stays the lane's.
                    let channels = classify_lane_failure(&execution);
                    let outcome = match execution.outcome {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            let (stop_kind, role_verdict) = lane_stop_diagnostic(&error);
                            eprintln!(
                                "forge-execution-stop story={} instance={} wave={} task={} job={} attempt={} stop={} error_code={} role_verdict={} detail={}",
                                story_id,
                                instance_id,
                                wave_index,
                                execution.claimed.task.task_id,
                                execution.claimed.lease.job_id,
                                execution.claimed.lease.attempts,
                                stop_kind,
                                error.code(),
                                role_verdict,
                                error
                            );
                            if channels.is_engine_fault {
                                if engine_fault.is_none() {
                                    engine_fault = Some(error);
                                }
                            } else if hold_reason.is_none() {
                                hold_reason = Some(channels.reason.clone());
                            }
                            // The tree is RETAINED, and the guard is what says so: a lane that failed may
                            // hold work no branch has yet (`WO-121`), so the disposition is decided and
                            // named — path and branch — rather than left to a `let _ =`.
                            execution.worktrees.retain(
                                &execution.claimed.task.task_id,
                                &format!("execution_failed={}", channels.reason),
                            );
                            continue;
                        }
                    };

                    let candidate = outcome.evidence.candidate_sha.clone();
                    if let Err(error) = complete_role_task_with_transient_retry(
                        rt,
                        &execution.claimed.task.task_id,
                        &execution.claimed.actor,
                        &outcome,
                    ) {
                        if error.code() == "TASK_ALREADY_COMPLETED" {
                            // Settle through the shared helper (`WO-302` Part B): a transient complete
                            // failure here must hold with the failure named, never orphan the job.
                            if let Some(reason) = settle_completed_lane(
                                durable.jobs,
                                opts.worker_id,
                                &execution.claimed.lease.job_id,
                                &execution.claimed.task.task_id,
                                true,
                            ) {
                                if hold_reason.is_none() {
                                    hold_reason = Some(reason);
                                }
                            }
                            // This release used to be `let _ = remove_lane_worktree(..)`: a tree Forge could
                            // not remove was not worth a line. It is reported now (`WO-121`). The task is
                            // still complete, so the loop continues — but the story is held when nothing
                            // else held it, because a tree left behind is Forge's to answer for.
                            if let Some(cleanup) =
                                execution.worktrees.release(&execution.claimed.task.task_id)
                            {
                                if hold_reason.is_none() {
                                    hold_reason = Some(format!(
                                        "task {} was already completed, but its lane worktree could not be released: {cleanup}",
                                        execution.claimed.task.task_id
                                    ));
                                }
                            }
                            continue;
                        }
                        let reason = completion_failure_reason(&error);
                        if let Err(settle_error) = durable.jobs.fail(
                            &execution.claimed.lease.job_id,
                            opts.worker_id,
                            &reason,
                            true,
                        ) {
                            eprintln!(
                                "forge-wave task={} durable_settlement_failed={settle_error}",
                                execution.claimed.task.task_id
                            );
                        }
                        if hold_reason.is_none() {
                            hold_reason = Some(format!(
                                "completion failed for task {}: {error}",
                                execution.claimed.task.task_id
                            ));
                        }
                        continue;
                    }
                    if let Some(candidate) = candidate.as_deref() {
                        if let Err(error) = integrate_lane_candidate(
                            &execution.parent_worktree,
                            &execution.base_commit,
                            candidate,
                        ) {
                            let primary = format!(
                                "task {} completed in Workflow, but candidate {candidate} could not be integrated: {error}",
                                execution.claimed.task.task_id
                            );
                            // The claim is settled through the shared helper, so a settlement failure that
                            // happens HERE is reported beside the integration failure (`WO-121`) instead of
                            // being dropped by a `let _ =`.
                            let reason = match fail_claimed_lane(
                                durable.jobs,
                                opts.worker_id,
                                &execution.claimed,
                                &primary,
                                true,
                            ) {
                                Some(settlement) => with_secondary_failure(
                                    &primary,
                                    "durable claim settlement",
                                    &settlement,
                                ),
                                None => primary,
                            };
                            if hold_reason.is_none() {
                                hold_reason = Some(reason);
                            }
                            execution.worktrees.retain(
                                &execution.claimed.task.task_id,
                                &format!("integration_failed={error}"),
                            );
                            continue;
                        }
                    }
                    // Settle through the shared helper (`WO-302` Part B): the completed outcome is
                    // already recorded above, so a transient complete failure holds with the failure
                    // named instead of orphaning the job.
                    if let Some(reason) = settle_completed_lane(
                        durable.jobs,
                        opts.worker_id,
                        &execution.claimed.lease.job_id,
                        &execution.claimed.task.task_id,
                        false,
                    ) {
                        if hold_reason.is_none() {
                            hold_reason = Some(reason);
                        }
                    }
                    // The lane is done, so its tree is released through the guard (`WO-121`). A release that
                    // fails is logged by the guard AND becomes the story's reason to be held when nothing
                    // else held it: a tree Forge could not remove is a fact an operator has to see, not a
                    // line in a log nobody reads.
                    if let Some(cleanup) =
                        execution.worktrees.release(&execution.claimed.task.task_id)
                    {
                        if hold_reason.is_none() {
                            hold_reason = Some(format!(
                                "task {} completed, but its lane worktree could not be released: {cleanup}",
                                execution.claimed.task.task_id
                            ));
                        }
                    }
                    steps.push(execution.claimed.node.clone());
                    if stop_target
                        .as_ref()
                        .map(|stop| stop.contains(&execution.claimed.node))
                        .unwrap_or(false)
                    {
                        stopped_after = Some(execution.claimed.node.clone());
                    }
                }
                // A genuine business Hold beats a sibling engine fault (`WO-302` Part A): the fault
                // stays observable (its own forge-execution-stop line above, plus attached to the Hold
                // reason), but it must not erase the verdict. Only with no Hold does the fault propagate.
                match wave_disposition(hold_reason, engine_fault) {
                    WaveDisposition::Hold(reason) => {
                        eprintln!(
                            "forge-story-stop story={} instance={} wave={} reason={}",
                            story_id, instance_id, wave_index, reason
                        );
                        rt.writer()
                            .mark_story_human_hold(story_id, &reason)
                            .map_err(|error| {
                                WorkflowError::generic(format!(
                                    "mark_story_human_hold({story_id}): {error}"
                                ))
                            })?;
                        return Ok(ending(&instance_id, &steps, &stopped_after, reconciled)
                            .needs_human(instance_status(rt, &instance_id)?, Some(reason)));
                    }
                    WaveDisposition::EngineFault(error) => {
                        return Err(error);
                    }
                    WaveDisposition::Clear => {}
                }
                if stopped_after.is_some() {
                    break;
                }
                continue;
            }
            for lane in batch {
                // A planned lane is only a scheduling hint. Another lane or
                // external owner may have advanced/cancelled it since the
                // snapshot was built; re-read authoritative workflow state
                // immediately before enqueue/claim.
                let planned_task_id = lane.task.task_id.clone();
                let Some(task) = rt.list_role_tasks(story_id)?.into_iter().find(|task| {
                    task.task_id == planned_task_id && task.status == TaskStatus::Ready
                }) else {
                    eprintln!(
                        "forge-wave task={} skipped=stale-readiness",
                        planned_task_id
                    );
                    continue;
                };
                let node = task.node_id.clone().unwrap_or_default();
                let actor = task
                    .candidates
                    .first()
                    .cloned()
                    .unwrap_or_else(|| opts.worker_id.to_string());
                let mut durable_lease: Option<ForgeJobLease> = None;
                let outcome = if let Some(durable) = durable {
                    let request =
                        ForgeJobBridge::new(durable.registry).job_for_ready_task(&task)?;
                    let job_id = durable.jobs.enqueue(&request)?;
                    let lease = match durable.jobs.claim_one(&job_id, opts.worker_id) {
                        Ok(lease) => lease,
                        Err(err) if err.code() == "FORGE_JOB_NOT_CLAIMABLE" => {
                            let state = durable.jobs.inspect(&job_id)?;
                            let detail = state
                                .last_error
                                .as_deref()
                                .unwrap_or("no durable job error recorded");
                            match state.status {
                                JobStatus::Failed | JobStatus::Cancelled | JobStatus::Completed => {
                                    let reason = format!(
                                        "Forge durable job {job_id} for task {} ({node}) was already {:?} by an \
                                         earlier run, so this run executed nothing. That run's error: {detail}. \
                                         It is waiting for that turn's completion to be reconciled.",
                                        task.task_id, state.status
                                    );
                                    rt.writer()
                                        .mark_story_human_hold(story_id, &reason)
                                        .map_err(|error| {
                                            WorkflowError::generic(format!(
                                                "mark_story_human_hold({story_id}): {error}"
                                            ))
                                        })?;
                                    return Ok(ending(
                                        &instance_id,
                                        &steps,
                                        &stopped_after,
                                        reconciled,
                                    )
                                    .needs_human(
                                        instance_status(rt, &instance_id)?,
                                        Some(reason),
                                    ));
                                }
                                JobStatus::Locked | JobStatus::Pending => {
                                    return Ok(ending(
                                        &instance_id,
                                        &steps,
                                        &stopped_after,
                                        reconciled,
                                    )
                                    .exhausted(
                                        instance_status(rt, &instance_id)?,
                                        Some(format!(
                                            "Forge durable job {job_id} for task {} is {:?}; attempts={}/{} locked_by={:?}",
                                            task.task_id,
                                            state.status,
                                            state.attempts,
                                            state.max_attempts,
                                            state.locked_by
                                        )),
                                    ));
                                }
                            }
                        }
                        Err(err) => return Err(err),
                    };
                    // Durable production composes execution-scoped interruption from the registered
                    // service inside the job boundary, where the exact lease identity is available.
                    let _request =
                        ForgeJobBridge::new(durable.registry).job_for_ready_task(&task)?;
                    // Use the shared Forge turn budget regardless of the selected harness.
                    let turn_ceiling = turn_ceiling_from_env();

                    let outcome = match execute_claimed_job_unsettled(
                        durable.jobs,
                        opts.worker_id,
                        &lease,
                        &task,
                        durable.registry,
                        None,
                        turn_ceiling,
                    ) {
                        Ok(outcome) => outcome,
                        Err(err) if is_engine_fault_error(&err) => return Err(err),
                        Err(err) => {
                            let (stop_kind, role_verdict) = lane_stop_diagnostic(&err);
                            let reason = lane_failure_reason(&node, &task.task_id, &err);
                            eprintln!(
                                "forge-execution-stop story={} instance={} wave={} task={} job={} attempt={} stop={} error_code={} role_verdict={} detail={}",
                                story_id,
                                instance_id,
                                wave_index,
                                task.task_id,
                                lease.job_id,
                                lease.attempts,
                                stop_kind,
                                err.code(),
                                role_verdict,
                                err
                            );
                            rt.writer()
                                .mark_story_human_hold(story_id, &reason)
                                .map_err(|error| {
                                    WorkflowError::generic(format!(
                                        "mark_story_human_hold({story_id}): {error}"
                                    ))
                                })?;
                            return Ok(ending(&instance_id, &steps, &stopped_after, reconciled)
                                .needs_human(instance_status(rt, &instance_id)?, Some(reason)));
                        }
                    };
                    durable_lease = Some(lease);
                    outcome
                } else {
                    let Some(runner) = opts.runner else {
                        return Err(WorkflowError::generic(
                            "Direct Forge execution reached a role turn with no explicit real role runner",
                        ));
                    };
                    if let Err(err) = rt.claim_role_task(&task.task_id, &actor) {
                        if is_advance_conflict(&err) {
                            continue;
                        }
                        return Err(err);
                    }
                    match runner.run(&node, &task) {
                        Ok(o) => o,
                        Err(err) => {
                            let settle =
                                settle_forge_lane_failure(rt, &task.task_id, &actor, &err)?;
                            if settle == LaneFailureSettlement::AlreadyCompleted {
                                continue;
                            }
                            return Err(err);
                        }
                    }
                };

                if let Err(err) =
                    complete_role_task_with_transient_retry(rt, &task.task_id, &actor, &outcome)
                {
                    if let (Some(durable), Some(lease)) = (durable, durable_lease.as_ref()) {
                        if err.code() == "TASK_ALREADY_COMPLETED" {
                            if let Err(complete_error) =
                                durable.jobs.complete(&lease.job_id, opts.worker_id)
                            {
                                eprintln!(
                                    "forge-wave task {} already completed but durable job settlement failed (preserving paid turn for reconciliation): {complete_error}",
                                    task.task_id
                                );
                            }
                            continue;
                        }
                        if let Err(fail_error) = durable.jobs.fail(
                            &lease.job_id,
                            opts.worker_id,
                            &completion_failure_reason(&err),
                            true,
                        ) {
                            eprintln!(
                                "forge-wave task={} durable_settlement_failed={fail_error}",
                                task.task_id
                            );
                        }
                        return Err(err);
                    }

                    if is_advance_conflict(&err) {
                        continue;
                    }
                    let settle = settle_forge_lane_failure(rt, &task.task_id, &actor, &err)?;
                    if settle == LaneFailureSettlement::AlreadyCompleted {
                        continue;
                    }
                    return Err(err);
                }

                if let (Some(durable), Some(lease)) = (durable, durable_lease.as_ref()) {
                    if let Err(complete_error) =
                        durable.jobs.complete(&lease.job_id, opts.worker_id)
                    {
                        let reason = completion_failure_reason(&WorkflowError::generic(
                            complete_error.to_string(),
                        ));
                        eprintln!(
                            "forge-wave task {} completed but durable job settlement failed (preserving paid turn for reconciliation): {reason}",
                            task.task_id
                        );
                    }
                }
                steps.push(node.clone());
                if stop_target
                    .as_ref()
                    .map(|s| s.contains(&node))
                    .unwrap_or(false)
                {
                    stopped_after = Some(node);
                }
            }
            if stopped_after.is_some() {
                break;
            }
        }
        if stopped_after.is_some() {
            break;
        }
    }

    let tasks = rt.list_role_tasks(story_id)?;
    let instance = rt.engine().get_process_instance(&instance_id)?;

    if process_completes_story(instance.status, instance.outcome) {
        rt.writer().mark_story_complete(story_id).map_err(|error| {
            WorkflowError::generic(format!("mark_story_complete({story_id}): {error}"))
        })?;
    }

    Ok(
        ending(&instance_id, &steps, &stopped_after, reconciled).finished(
            format!("{:?}", instance.status),
            !tasks.is_empty(),
            tasks
                .iter()
                .any(|t| t.node_id.as_deref().map(is_human_gate).unwrap_or(false)),
        ),
    )
}
