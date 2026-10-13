use super::*;

impl RoleHarness for OpenCodeHarness {
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        self.run_role_with_slot(node_id, task, self_heal, &self.live_turn)
    }

    /// The structured report for a turn: the outcome AND the two readings taken from the vendor's own stream.
    ///
    /// `trajectory` is what the turn DID, however the turn ended. A failed or interrupted turn whose stream Forge
    /// could read reports the calls it made before it died; only an unreadable stream reports `None`, which is
    /// UNMEASURED — never a zero.
    fn run_role_report(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        let mut measured_usage = None;
        let mut measured_trajectory = None;
        let outcome = self.run_role_with_slot_measured(
            node_id,
            task,
            self_heal,
            &self.live_turn,
            &mut measured_usage,
            &mut measured_trajectory,
        );
        crate::engine::runner::HarnessTurnReport {
            trajectory: measured_trajectory,
            outcome,
            measured_usage,
        }
    }

    fn run_role_scoped(
        &self,
        execution_id: &str,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        let slot = self.execution_slot(execution_id)?;
        if slot
            .lock()
            .map_err(|_| WorkflowError::generic("execution interrupt slot is poisoned"))?
            .interrupt_reason
            .is_some()
        {
            return Err(WorkflowError::generic(format!(
                "{}: execution was cancelled before model launch",
                crate::engine::opencode::TURN_INTERRUPTED_CODE
            )));
        }
        self.run_role_with_slot(node_id, task, self_heal, &slot)
    }

    fn run_role_scoped_report(
        &self,
        execution_id: &str,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        let mut measured_usage = None;
        let mut measured_trajectory = None;
        let outcome = self.execution_slot(execution_id).and_then(|slot| {
            if slot
                .lock()
                .map_err(|_| WorkflowError::generic("execution interrupt slot is poisoned"))?
                .interrupt_reason
                .is_some()
            {
                // Cancelled BEFORE launch: no vendor process ran, so there is nothing observed to report. That is
                // the one case where an empty report is the honest one — the turn never spent anything.
                return Err(WorkflowError::generic(format!(
                    "{}: execution was cancelled before model launch",
                    crate::engine::opencode::TURN_INTERRUPTED_CODE
                )));
            }
            self.run_role_with_slot_measured(
                node_id,
                task,
                self_heal,
                &slot,
                &mut measured_usage,
                &mut measured_trajectory,
            )
        });
        crate::engine::runner::HarnessTurnReport {
            trajectory: measured_trajectory,
            outcome,
            measured_usage,
        }
    }

    fn fork_for_workspace(
        &self,
        workspace: ExecutionWorkspace,
    ) -> Result<Option<Arc<dyn RoleHarness>>> {
        Ok(Some(Arc::new(OpenCodeHarness::fork_for_workspace(
            self, workspace,
        )?)))
    }

    fn begin_execution(&self, execution_id: &str) -> Result<()> {
        self.execution_slot(execution_id).map(|_| ())
    }

    /// Stop the turn this harness is running, if it is running one.
    ///
    /// The vendor's own `POST /session/{id}/abort` is NOT reachable from here: a `--standalone` run speaks stdio to a
    /// private child server and exposes no addressable HTTP endpoint (measured — see `engine::opencode_client`). The
    /// primitive is therefore a signal to the process group Forge spawned, and the reason is stored on the turn so
    /// the read that follows reports an interruption rather than a crash with no stderr.
    fn interrupt_execution(&self, reason: &str) -> Result<Option<TurnTermination>> {
        let running = {
            let mut slot = self.live_turn.lock().map_err(|_| {
                WorkflowError::generic("the live-turn slot is poisoned; refusing to claim a stop")
            })?;
            let Some(running) = slot.running else {
                // Nothing is running: an answer, not a failure.
                return Ok(None);
            };
            slot.interrupt_reason = Some(reason.to_string());
            running
        };
        let termination = running.terminate();
        eprintln!(
            "opencode-harness interrupted pid={} reason={reason} existed={} killed={} signalled={:?}",
            running.pid, termination.existed, termination.killed, termination.signalled
        );
        Ok(Some(termination))
    }

    fn interrupt_execution_scoped(
        &self,
        execution_id: &str,
        reason: &str,
    ) -> Result<Option<TurnTermination>> {
        let slot = self.execution_slot(execution_id)?;
        let running = {
            let mut turn = slot
                .lock()
                .map_err(|_| WorkflowError::generic("execution interrupt slot is poisoned"))?;
            turn.interrupt_reason = Some(reason.to_string());
            turn.running
        };
        let Some(running) = running else {
            eprintln!("forge-interrupt execution={execution_id} requested=true delivered=pending reason={reason}");
            return Ok(None);
        };
        let termination = running.terminate();
        eprintln!(
            "forge-interrupt execution={execution_id} requested=true delivered=true pid={} existed={} killed={} signalled={:?} reason={reason}",
            running.pid, termination.existed, termination.killed, termination.signalled
        );
        Ok(Some(termination))
    }

    fn finish_execution(&self, execution_id: &str) {
        if let Ok(mut slots) = self.execution_turns.lock() {
            slots.remove(execution_id);
        }
    }

    fn supports_interrupt(&self) -> bool {
        true
    }

    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool {
        self.run_git(&["cat-file", "-e", &format!("{base_ref}:{path}")])
            .is_some()
            || self
                .run_git(&["ls-tree", "--name-only", base_ref, path])
                .map(|s| !s.is_empty())
                .unwrap_or(false)
    }

    fn assay_cwd(&self) -> &Path {
        if let Some(ws) = &self.execution_workspace {
            return Path::new(&ws.worktree_path);
        }
        self.workspace.as_path()
    }

    fn candidate_probe(&self) -> Option<&dyn crate::engine::runner::CandidateProbe> {
        Some(self)
    }

    fn production_probe(&self) -> Option<&dyn crate::engine::runner::ProductionProbe> {
        Some(self)
    }

    fn execution_base_commit(&self) -> Option<&str> {
        self.execution_workspace
            .as_ref()
            .map(|workspace| workspace.base_commit.as_str())
    }

    fn execution_workspace(&self) -> Option<&ExecutionWorkspace> {
        self.execution_workspace.as_ref()
    }

    fn run_command(&self, command: &str) -> CommandResult {
        use crate::engine::assay::{
            assay_timeout, cancellation_signal, is_cmd_timeout, spawn_scoped_shell_with_env,
            CeilingOutcome, CMD_CANCELLED_EXIT, CMD_TIMEOUT_CODE, CMD_TIMEOUT_EXIT,
        };
        let cwd = self.assay_cwd();
        // FIX-007: per-worktree CARGO_TARGET_DIR for build isolation, sanitized env (no secrets).
        let cargo_target_dir = derive_cargo_target_dir(cwd);
        let mut assay_env = sanitize_assay_env(std::env::vars().collect());
        assay_env.insert(
            "CARGO_TARGET_DIR".into(),
            cargo_target_dir.to_string_lossy().to_string(),
        );
        // FIX-005: bounded — a hung assay held the story claim indefinitely because this shell had
        // no deadline. The ceiling kills the command's tree; the claim is released by returning, and the
        // story requeues on the `CMD_TIMEOUT` blocker the adjudicator reads from the excerpt.
        let child = match spawn_scoped_shell_with_env(command, cwd, &assay_env) {
            Ok(child) => child,
            Err(e) => {
                return CommandResult {
                    cancelled: false,
                    command: command.into(),
                    exit_code: -1,
                    passed: false,
                    excerpt: e.to_string(),
                    unmeasurable: true,
                    output: String::new(),
                };
            }
        };
        match crate::engine::assay::wait_with_ceiling(child, assay_timeout()) {
            CeilingOutcome::TimedOut(hit) => CommandResult {
                cancelled: false,
                command: command.into(),
                exit_code: CMD_TIMEOUT_EXIT,
                passed: false,
                excerpt: format!(
                    "{CMD_TIMEOUT_CODE}: assay command timed out after {}s and was killed (pid {}): {command}",
                    hit.ceiling.as_secs(),
                    hit.pid
                ),
                unmeasurable: true,
                output: String::new(),
            },
            CeilingOutcome::Finished(Err(e)) => CommandResult {
                cancelled: false,
                command: command.into(),
                exit_code: -1,
                passed: false,
                excerpt: format!("could not observe assay command: {e}"),
                unmeasurable: true,
                output: String::new(),
            },
            CeilingOutcome::Finished(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                // FIX-003: BOTH streams are evidence — cargo diagnostics print to stderr while test
                // harnesses print to stdout, and keeping only one silently discards the compiler error.
                let text = crate::engine::assay::combine_command_output(&stdout, &stderr);
                let signal = cancellation_signal(&out.status);
                let code = if signal.is_some() {
                    CMD_CANCELLED_EXIT
                } else {
                    out.status.code().unwrap_or(1)
                };
                let text = if let Some(signal) = signal {
                    format!("CMD_CANCELLED: assay command received signal {signal}\n{text}")
                } else {
                    text
                };
                let result = CommandResult {
                    cancelled: signal.is_some(),
                    command: command.into(),
                    exit_code: code,
                    passed: code == 0,
                    excerpt: text
                        .lines()
                        .take(crate::engine::assay::COMMAND_EXCERPT_LINES)
                        .collect::<Vec<_>>()
                        .join("\n"),
                    unmeasurable: signal.is_some(),
                    output: text,
                };
                debug_assert!(
                    !is_cmd_timeout(&result),
                    "a finished command must not wear the timeout marker"
                );
                result
            }
        }
    }
}
