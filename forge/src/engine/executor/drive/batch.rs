//! The durable lane batch: claiming a batch of ready lanes, preparing their worktrees, executing them and
//! recording what a failed lane leaves behind. Moved out of `drive.rs` on 2026-10-14 to keep that file under
//! the boundedness ceiling — move-only apart from `pub(super)` on the entry points the parent still calls,
//! which it reaches through its own glob import.
//!
//! The public shape of a drive (`DriveForgeStoryResult`, the options, the two entry points) stays in
//! `drive.rs`; this file is what a claimed batch does.

use super::*;

/// Fail ONE lane's durable claim, and return a settlement failure that happens on the way (`WO-121`).
///
/// The failure is RETURNED, and the caller folds it into the reason it reports for the lane: a claim
/// that could not be released is a second fact about the lane, not a footnote to it. The line names
/// the job so the stale-claim sweep can find it.
pub(super) fn fail_claimed_lane(
    jobs: &dyn JobService,
    worker_id: &str,
    lane: &ClaimedDurableLane,
    reason: &str,
    permanent: bool,
) -> Option<String> {
    match jobs.fail(&lane.lease.job_id, worker_id, reason, permanent) {
        Ok(()) => None,
        Err(error) => {
            eprintln!(
                "forge-wave job={} could_not_release_claim error={error}",
                lane.lease.job_id
            );
            Some(error.to_string())
        }
    }
}

pub(super) fn fail_claimed_batch(
    jobs: &dyn JobService,
    worker_id: &str,
    lanes: &[ClaimedDurableLane],
    reason: &str,
) {
    for lane in lanes {
        fail_claimed_lane(jobs, worker_id, lane, reason, false);
    }
}

/// The primary failure decides. A failure that happens *while* it is being handled is reported beside
/// it, never instead of it (`WO-121`): the primary text stays first, so a reader — and the doctor that
/// scans these lines — still recognises the fault that stopped the lane.
pub(super) fn with_secondary_failure(primary: &str, what: &str, failure: &str) -> String {
    format!("{primary}; additionally {what} failed: {failure}")
}

/// What the guard calls to remove a lane worktree: `discard_unstarted_lane_worktree`'s own signature.
/// The error is a `String` because that is the worktree module's error type — the guard reports it
/// beside whatever failure it is already handling rather than converting it (`WO-121`).
pub(super) type LaneWorktreeRemover =
    Box<dyn Fn(&Path, &Path, &str, &str) -> std::result::Result<(), String> + Send>;

/// Where one lane's worktree is, and what Forge decided to do with it.
///
/// `WO-121`. From `provision_worker_workspace` until the drive loop has settled the lane, the worktree
/// is a resource this module owns. Ownership has ONE rule: the disposition is decided and named on
/// every exit, including an unwind. Four `let _ = discard_unstarted_lane_worktree(..)` calls and a
/// `join().expect(..)` were each able to end a lane with a tree nothing could explain.
///
/// Two dispositions, and the difference matters:
///   * `release` — remove the tree, and the branch with it only while the branch still sits at its
///     base. A branch that moved holds a candidate, so it stays, and the log names it.
///   * `retain` — keep the tree, and say where it is and which branch it holds, because a lane that
///     failed may hold uncommitted work and paid code outranks a tidy directory (`MEMORY.md`,
///     "PAID CODE > GIT SHA"). A panic decides this the safe way: it records the retention instead of
///     removing a tree nobody read.
///
/// `Drop` is the backstop. A lane that never reported still gets a disposition and a log line, so no
/// exit — success, error, or panic — can leave a worktree orphaned and unexplained.
pub(super) struct LaneWorktree {
    repo_root: PathBuf,
    path: PathBuf,
    branch_name: String,
    base_commit: String,
    disposition: LaneWorktreeDisposition,
    remove: LaneWorktreeRemover,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum LaneWorktreeDisposition {
    Undecided,
    Released,
    Retained,
}

impl LaneWorktree {
    pub(super) fn new(
        repo_root: PathBuf,
        path: PathBuf,
        branch_name: String,
        base_commit: String,
    ) -> Self {
        Self::with_remover(
            repo_root,
            path,
            branch_name,
            base_commit,
            Box::new(discard_unstarted_lane_worktree),
        )
    }

    pub(super) fn with_remover(
        repo_root: PathBuf,
        path: PathBuf,
        branch_name: String,
        base_commit: String,
        remove: LaneWorktreeRemover,
    ) -> Self {
        Self {
            repo_root,
            path,
            branch_name,
            base_commit,
            disposition: LaneWorktreeDisposition::Undecided,
            remove,
        }
    }

    /// A guard for a lane whose owner exited before it could hand the tree over — an early return or a
    /// panic. That exit left the tree in place and its own guard logged where, so this disposition is
    /// already settled and a second removal here would misreport what happened.
    pub(super) fn retained_by_its_lane(path: PathBuf, branch_name: String) -> Self {
        Self {
            repo_root: PathBuf::new(),
            path,
            branch_name,
            base_commit: String::new(),
            disposition: LaneWorktreeDisposition::Retained,
            remove: Box::new(|_, _, _, _| Ok(())),
        }
    }

    /// What the guard decided. Read by the tests that assert a lane's exit disposition (`WO-121`).
    #[cfg(test)]
    pub(super) fn disposition(&self) -> LaneWorktreeDisposition {
        self.disposition
    }

    /// Release the tree. A cleanup failure is RETURNED, never swallowed: the caller is already handling
    /// a primary failure and has to report both.
    pub(super) fn release(&mut self, task_id: &str) -> Option<String> {
        if self.disposition != LaneWorktreeDisposition::Undecided {
            return None;
        }
        self.disposition = LaneWorktreeDisposition::Released;
        match (self.remove)(
            &self.repo_root,
            &self.path,
            &self.branch_name,
            &self.base_commit,
        ) {
            Ok(()) => None,
            Err(error) => {
                eprintln!(
                    "forge-wave task={task_id} worktree_cleanup_failed={error}; branch={} remains reachable",
                    self.branch_name
                );
                Some(error)
            }
        }
    }

    /// Keep the tree and name it: the path is what an operator inspects, and the branch is what
    /// survives the tree.
    pub(super) fn retain(&mut self, task_id: &str, why: &str) {
        if self.disposition != LaneWorktreeDisposition::Undecided {
            return;
        }
        self.disposition = LaneWorktreeDisposition::Retained;
        eprintln!(
            "forge-wave task={task_id} {why}; isolated worktree retained at {} branch={}",
            self.path.display(),
            self.branch_name
        );
    }
}

impl Drop for LaneWorktree {
    fn drop(&mut self) {
        if self.disposition == LaneWorktreeDisposition::Undecided {
            // Nobody released or retained this tree: its lane unwound before it reported. Retain it —
            // a panic is not evidence that the tree is empty — but never silently.
            self.disposition = LaneWorktreeDisposition::Retained;
            eprintln!(
                "forge-wave worktree_disposition=retained_on_exit worktree={} branch={} reason=the lane exited before it handed the tree over",
                self.path.display(),
                self.branch_name
            );
        }
    }
}

pub(super) fn current_head(repo: &std::path::Path) -> Result<String> {
    let output = Command::new(crate::engine::worktree::git_binary())
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(repo)
        .output()
        .map_err(|error| WorkflowError::generic(format!("git rev-parse HEAD: {error}")))?;
    if !output.status.success() {
        return Err(WorkflowError::generic(format!(
            "could not resolve the story worktree HEAD: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn prepare_durable_batch<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    planned: &[WaveLane<ActiveForgeRoleTask>],
    durable: DurableForgeExecution<'_, '_>,
    worker_id: &str,
) -> Result<Vec<PreparedDurableLane>> {
    let mut claimed = Vec::new();
    for lane in planned {
        let task_id = lane.task.task_id.clone();
        let current_tasks = match rt.list_role_tasks(story_id) {
            Ok(tasks) => tasks,
            Err(error) => {
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        };
        let Some(task) = current_tasks
            .into_iter()
            .find(|task| task.task_id == task_id && task.status == TaskStatus::Ready)
        else {
            eprintln!("forge-wave task={task_id} skipped=stale-readiness");
            continue;
        };
        let node = task.node_id.clone().unwrap_or_default();
        let actor = task
            .candidates
            .first()
            .cloned()
            .unwrap_or_else(|| worker_id.to_string());
        let request = match ForgeJobBridge::new(durable.registry).job_for_ready_task(&task) {
            Ok(request) => request,
            Err(error) => {
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        };
        let job_id = match durable.jobs.enqueue(&request) {
            Ok(job_id) => job_id,
            Err(error) => {
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        };
        let lease = match durable.jobs.claim_one(&job_id, worker_id) {
            Ok(lease) => lease,
            Err(error) => {
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        };
        claimed.push(ClaimedDurableLane {
            task,
            actor,
            node,
            lease,
        });
    }

    let mut prepared = Vec::new();
    for lane in claimed.iter() {
        let preparation = (|| -> Result<PreparedDurableLane> {
            let service = durable.registry.resolve(&lane.lease.service_key)?;
            let runner = service.runner();
            let Some(ports) = runner.turn_ports() else {
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    format!(
                        "service {} does not expose production turn ports for isolated execution",
                        lane.lease.service_key
                    ),
                ));
            };
            let harness = ports.harness();
            if !harness.supports_interrupt() {
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    format!(
                        "harness for service {} cannot isolate and interrupt a concurrent execution",
                        lane.lease.service_key
                    ),
                ));
            }
            let Some(parent_execution) = harness.execution_workspace().cloned() else {
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    "concurrent durable lanes require a provisioned story worktree; enable FORGE_PROVISION=1",
                ));
            };
            if !parent_execution.branch_name.starts_with("agent/") {
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    format!(
                        "story worktree branch {:?} is not an isolated agent branch",
                        parent_execution.branch_name
                    ),
                ));
            }
            let parent_worktree = PathBuf::from(&parent_execution.worktree_path);
            if !parent_worktree.is_dir() {
                return Err(WorkflowError::generic(format!(
                    "story worktree is missing: {}",
                    parent_worktree.display()
                )));
            }
            let repo_root =
                resolve_repo_root(Some(&parent_worktree)).map_err(WorkflowError::generic)?;
            let worktrees_root = repo_root
                .parent()
                .unwrap_or(&repo_root)
                .join(DEFAULT_WORKTREES_DIRNAME);
            let base_commit = current_head(&parent_worktree)?;
            let run_id = format!(
                "lane-{}-{}-{}-{}",
                lane.lease.job_id,
                lane.lease.attempts,
                lane.lease.process_instance_id,
                std::process::id()
            );
            let workspace = provision_worker_workspace(
                Some(&parent_worktree),
                story_id,
                Some(&run_id),
                Some(&base_commit),
                Some(&worktrees_root),
            )
            .map_err(WorkflowError::generic)?;
            let branch_name = workspace.branch_name.clone();
            let execution_workspace = ExecutionWorkspace {
                worktree_path: workspace.worktree_path.display().to_string(),
                branch_name: branch_name.clone(),
                base_ref: workspace.base_ref,
                base_commit: workspace.base_commit,
            };
            // The lane's tree is owned from here on (`WO-121`). Every exit below either releases it or
            // retains it with a name; the guard's `Drop` covers only the exit nobody wrote.
            let mut worktrees = LaneWorktree::new(
                repo_root.clone(),
                workspace.worktree_path.clone(),
                branch_name.clone(),
                base_commit.clone(),
            );
            let forked_runner = match runner.fork_for_workspace(execution_workspace) {
                Ok(Some(runner)) => runner,
                Ok(None) => {
                    let primary = format!(
                        "service {} harness cannot create an isolated lane execution",
                        lane.lease.service_key
                    );
                    return Err(WorkflowError::conflict(
                        "FORGE_PARALLEL_LANES_UNAVAILABLE",
                        match worktrees.release(&lane.task.task_id) {
                            Some(cleanup) => {
                                with_secondary_failure(&primary, "worktree discard", &cleanup)
                            }
                            None => primary,
                        },
                    ));
                }
                Err(error) => {
                    // The fork error is returned as it came: its code is how the caller classifies it,
                    // and a message is not worth losing a classification for. The cleanup failure is
                    // reported beside it instead (`WO-121`).
                    if let Some(cleanup) = worktrees.release(&lane.task.task_id) {
                        eprintln!(
                            "forge-wave task={} lane_fork_failed={error} worktree_discard_failed={cleanup}",
                            lane.task.task_id
                        );
                    }
                    return Err(error);
                }
            };
            if !forked_runner
                .turn_ports()
                .map(|ports| ports.harness().supports_interrupt())
                .unwrap_or(false)
            {
                let primary = format!(
                    "forked runner for service {} lost execution-scoped interruption",
                    lane.lease.service_key
                );
                return Err(WorkflowError::conflict(
                    "FORGE_PARALLEL_LANES_UNAVAILABLE",
                    match worktrees.release(&lane.task.task_id) {
                        Some(cleanup) => {
                            with_secondary_failure(&primary, "worktree discard", &cleanup)
                        }
                        None => primary,
                    },
                ));
            }
            Ok(PreparedDurableLane {
                claimed: ClaimedDurableLane {
                    task: lane.task.clone(),
                    actor: lane.actor.clone(),
                    node: lane.node.clone(),
                    lease: lane.lease.clone(),
                },
                runner: forked_runner,
                worktree_path: workspace.worktree_path,
                branch_name,
                parent_worktree,
                base_commit,
                worktrees,
            })
        })();
        match preparation {
            Ok(prepared_lane) => prepared.push(prepared_lane),
            Err(error) => {
                // The batch is abandoned, so every lane that was already provisioned is released — and
                // a release that fails is reported beside the error that abandoned the batch, never
                // instead of it (`WO-121`).
                for lane in &mut prepared {
                    if let Some(cleanup) = lane.worktrees.release(&lane.claimed.task.task_id) {
                        eprintln!(
                            "forge-wave task={} batch_abandoned={error} worktree_discard_failed={cleanup}",
                            lane.claimed.task.task_id
                        );
                    }
                }
                fail_claimed_batch(durable.jobs, worker_id, &claimed, &error.to_string());
                return Err(error);
            }
        }
    }
    Ok(prepared)
}

pub(super) fn execute_durable_batch(
    prepared: Vec<PreparedDurableLane>,
    jobs: &dyn JobService,
    worker_id: &str,
    turn_ceiling: Option<std::time::Duration>,
    story_id: &str,
    wave_index: usize,
) -> Vec<LaneExecutionResult> {
    let active_lanes = Arc::new(AtomicUsize::new(0));
    let lane_fallbacks: Vec<LaneFallback> = prepared.iter().map(LaneFallback::of).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = prepared
            .into_iter()
            .map(|prepared_lane| {
                let active_lanes = Arc::clone(&active_lanes);
                let story_id = story_id.to_string();
                let fallback_claimed = prepared_lane.claimed.clone();
                let fallback_worktree = prepared_lane.worktree_path.clone();
                let fallback_parent = prepared_lane.parent_worktree.clone();
                let fallback_base = prepared_lane.base_commit.clone();
                let fallback_branch = prepared_lane.branch_name.clone();
                scope.spawn(move || {
                    let active = active_lanes.fetch_add(1, Ordering::SeqCst) + 1;
                    eprintln!(
                        "forge-wave-lane story={} instance={} wave={} task={} job={} attempt={} active_lanes={} event=start",
                        story_id,
                        fallback_claimed.task.process_instance_id,
                        wave_index,
                        fallback_claimed.task.task_id,
                        fallback_claimed.lease.job_id,
                        fallback_claimed.lease.attempts,
                        active
                    );
                    let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || -> Result<LaneExecutionResult> {
                            let PreparedDurableLane {
                                claimed,
                                runner,
                                worktree_path: _,
                                branch_name: _,
                                parent_worktree,
                                base_commit,
                                worktrees,
                            } = prepared_lane;
                            let services = ForgeLaneServices::new(runner.as_ref());
                            let registry = services.registry()?;
                            let outcome = execute_claimed_job_unsettled(
                                jobs,
                                worker_id,
                                &claimed.lease,
                                &claimed.task,
                                &registry,
                                None,
                                turn_ceiling,
                            );
                            Ok(LaneExecutionResult {
                                claimed,
                                parent_worktree,
                                base_commit,
                                worktrees,
                                settlement_failure: None,
                                outcome,
                            })
                        },
                    ));
                    let result = match execution {
                        Ok(Ok(result)) => result,
                        Ok(Err(error)) => {
                            let settlement_failure = fail_claimed_lane(
                                jobs,
                                worker_id,
                                &fallback_claimed,
                                &error.to_string(),
                                true,
                            );
                            let worktrees = LaneWorktree::retained_by_its_lane(
                                fallback_worktree.clone(),
                                fallback_branch.clone(),
                            );
                            LaneExecutionResult {
                                claimed: fallback_claimed,
                                parent_worktree: fallback_parent,
                                base_commit: fallback_base,
                                worktrees,
                                settlement_failure,
                                outcome: Err(error),
                            }
                        }
                        Err(_) => {
                            let reason = "lane worker panicked before it could report an outcome";
                            let settlement_failure =
                                fail_claimed_lane(jobs, worker_id, &fallback_claimed, reason, true);
                            let worktrees = LaneWorktree::retained_by_its_lane(
                                fallback_worktree.clone(),
                                fallback_branch.clone(),
                            );
                            LaneExecutionResult {
                                claimed: fallback_claimed,
                                parent_worktree: fallback_parent,
                                base_commit: fallback_base,
                                worktrees,
                                settlement_failure,
                                outcome: Err(WorkflowError::generic(reason)),
                            }
                        }
                    };
                    let active = active_lanes.fetch_sub(1, Ordering::SeqCst) - 1;
                    eprintln!(
                        "forge-wave-lane story={} instance={} wave={} task={} job={} attempt={} active_lanes={} event=joined",
                        story_id,
                        result.claimed.task.process_instance_id,
                        wave_index,
                        result.claimed.task.task_id,
                        result.claimed.lease.job_id,
                        result.claimed.lease.attempts,
                        active
                    );
                    result
                })
            })
            .collect();
        // A thread that panicked OUTSIDE the lane's inner `catch_unwind` used to escape through this
        // `join().expect(..)` as a panic that named no lane and left every worktree in the batch behind
        // it (`WO-121` §3). It is a recorded per-lane failure now: the tree was dispositioned by the
        // lane's own guard during the unwind, and the claim is settled here.
        let mut results = Vec::with_capacity(handles.len());
        for (handle, fallback) in handles.into_iter().zip(lane_fallbacks) {
            match handle.join() {
                Ok(result) => results.push(result),
                Err(_) => {
                    let reason = "lane worker thread panicked before it could report an outcome";
                    results.push(lane_that_never_reported(jobs, worker_id, fallback, reason));
                }
            }
        }
        results
    })
}
