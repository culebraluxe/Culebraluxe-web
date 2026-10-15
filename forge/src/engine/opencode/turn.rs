//! The measured turn slot, moved out of `mod.rs` (2026-10-14).
//!
//! `mod.rs` had grown past the boundedness ceiling and this is the one method that carries the length. The move
//! is text-exact except for `pub(super)`, which keeps it reachable from the pass that calls it.

use super::*;

impl OpenCodeHarness {
    /// One turn, with the two measurements it produces published even when the turn does not succeed.
    ///
    /// `measured_usage` and `measured_trajectory` are OUT-parameters rather than parts of the return value because the
    /// failure paths below return `Err` — and a reading that only exists on the success path is exactly how a failed
    /// turn gets recorded as a turn that nothing happened in.
    ///
    /// `profile` is the policy the lifecycle resolved for this turn, or `None` for a call site that predates the
    /// resolver. A seam handed none does not run unguarded: the node's OWN lane policy is resolved
    /// (`harness_profile::policy_for_node`), which is the same table the profile resolver reads. That is the
    /// difference between a transition state and an exemption — the transition is in WHO resolved the policy, not
    /// in whether any policy applies.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_role_with_slot_measured(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
        live_turn: &LiveTurnSlot,
        profile: Option<&HarnessProfile>,
        measured_usage: &mut Option<crate::engine::harness::HarnessUsage>,
        measured_trajectory: &mut Option<crate::engine::trajectory::TrajectoryMetrics>,
    ) -> Result<HarnessOutput> {
        let cwd = self.workspace.to_string_lossy().to_string();
        if !self.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "OpenCode harness workspace does not exist: {cwd}"
            )));
        }
        let task_text = self.task_text(node_id, task, self_heal, profile);
        let before_sha = self.run_git(&["rev-parse", "HEAD"]);
        // The vendor configuration this turn runs as, MATERIALIZED FROM the policy the lifecycle resolved: the
        // adapter selects the shipped profile that implements the granted surface and refuses when none does — it
        // never authors the policy, and it never falls back to the vendor's own default agent, whose permissions
        // Forge did not author (see `opencode::materialize`). A seam handed no profile resolves the node's OWN lane
        // first, so even there the configuration comes from a grant rather than from nothing.
        let agent = match profile {
            Some(profile) => materialize::agent_for_profile(node_id, profile)?,
            None => materialize::agent_for_lane(node_id)?,
        }
        .id;
        // The subagent gate is stated in the log for EVERY turn, both postures. The disarmed line says the
        // recorded spend is the whole turn; the armed line says it is a lower bound, because that is the
        // measured behaviour (`harness_usage`). A silence here would be the one place an undercount could hide.
        tracing::info!(
            node = %node_id,
            agent = %agent,
            "opencode-harness {}",
            opencode_agents::render_subagent_declaration(
                opencode_agents::subagents_enabled_from_env(),
                std::env::var(opencode_agents::SUBAGENTS_ENV).ok().as_deref(),
            )
        );
        // Execution receipt, before the first model token is spent: what runs, where, as whom.
        tracing::info!(
            model = %self.model,
            bin = %self.cli_bin,
            cwd = %cwd,
            "harness=opencode"
        );
        // The config travels with the process (§3): a worktree does not discover the repo-root `opencode.json`,
        // so environment delivery is the path that actually applies.
        let model_env = v2_agent_env(self.env.as_ref())?;
        let lane = VENDOR_SESSION_LANE;
        // WHICH SESSION THIS TURN RESUMES, AND WHY THE WORKTREE DECIDES (2026-10-03, ENG-FORGE-C1-BUILD-INFO-01).
        //
        // Session continuity exists to pay a story's startup cost once. It was switched on when the engine was
        // mostly stable; it has been a liability since, because the stored session was chosen WITHOUT asking
        // whether it belongs to the directory this turn is about to run in. A vendor session is scoped to the
        // project directory that created it, and `worktree::derive_worktree_path` puts the RUN id in the worktree
        // path — so a FLIP (a fresh run, a fresh worktree) left `forge_vendor_session` naming a session in a
        // directory the engine had deleted, and every turn of every run after the flip re-sent that id. The
        // vendor refuses it before the first token: `UnexpectedStatus: 500`, tokens_in=0, cost_usd=0.000000 —
        // three attempts, $0.00, a Hold. Reproduced on demand: the same command without `--session` answers `ok`.
        //
        // THE DISCRIMINATOR IS THE DIRECTORY, NOT A JOB KIND. Retry or flip, the turn knows which worktree it is
        // in, and that is the same boundary the vendor enforces — so no mode field, no schema change, and nothing
        // a story would have to be told about itself:
        //
        //   * a RETRY inside a run keeps its worktree, so it keeps its directory, so it keeps its session and the
        //     startup cost stays paid once (`--session` = the exact id, as before);
        //   * a FLIP mints a new worktree, so the stored id is refused and the turn opens a fresh session in the
        //     directory it is actually in (`resume_session` above).
        //
        // Both facts are read here, and the probe is skipped when the marker already answered — which is the
        // common case inside a live run.
        let workspace_session = if session_continuity_enabled() {
            read_session_id(&cwd)
        } else {
            None
        };
        let lane_session = if session_continuity_enabled() {
            self.story_id.as_deref().and_then(|story| {
                vendor_session::read_vendor_session_id(story, lane)
                    .ok()
                    .flatten()
            })
        } else {
            None
        };
        let lane_session_lives_here = workspace_session.is_none()
            && lane_session.as_deref().is_some_and(|id| {
                crate::engine::harness_usage::session_lives_in(
                    &self.cli_bin,
                    &cwd,
                    Some(&model_env),
                    id,
                )
            });
        let session = resume_session(workspace_session, lane_session, lane_session_lives_here);
        let continue_session =
            session_continuity_enabled() && session.is_none() && session_marker_path(&cwd).exists();
        let opts = OpenCodeStartOptions {
            cli_bin: &self.cli_bin,
            cwd: &cwd,
            model: &self.model,
            task: task_text.as_str(),
            env: Some(&model_env),
            auto_approve: self.auto_approve,
            session: session.as_deref(),
            continue_session,
            agent: Some(agent),
            max_turn: turn_ceiling(std::env::var(TURN_CEILING_ENV).ok().as_deref()),
        };
        // Read BEFORE the turn: a resumed session's totals are cumulative, so its spend is a difference.
        let baseline = UsageBaseline::before_turn(
            &cwd,
            session.as_deref(),
            continue_session,
            &self.cli_bin,
            Some(&model_env),
        );
        // A FRESH turn owns the slot: a reason left over from the previous turn would be reported against work it
        // never touched.
        if let Ok(mut slot) = live_turn.lock() {
            *slot = crate::engine::opencode_client::LiveTurn::default();
        }
        // The live spend cap, read per turn the way the runner reads its own knobs: a cap is an operator decision,
        // not a constant. Unset stays unset — Forge does not invent a dollar ceiling for work it was asked to do,
        // and the turn-count ceiling below is the cap that always applies.
        //
        // WHAT THIS SEAM CAN AND CANNOT ENFORCE. The GENERATION turn cap is the DRIVER's to enforce
        // (`executor::drive_forge_story` counts dispatches per generation, which is the unit V1 measured: "a healthy
        // FEATURE generation costs five turns: architect, lead_pre, smith, post, qa"). This harness sees exactly one
        // turn and cannot count a generation's, so it does not pretend to. What it can do is stop the turn it is
        // holding, mid-flight, the moment the turn's own measured spend passes the cap — which is the difference
        // between a cap and an obituary.
        let spend_cap = self.spend_cap_usd;
        // C4, resolved: the thresholds THIS turn is measured against. A resolved profile is authoritative; a seam
        // handed none resolves the node's OWN lane (`harness_profile::policy_for_node`) rather than enforcing
        // nothing. Thresholds are configuration, and the configuration for a node is a fact of its lane — an
        // adapter that invented its own would be authoring policy, and an adapter that skipped the guard entirely
        // would leave the loop this guard exists to catch running to the turn ceiling.
        let mut runaway = match profile {
            Some(profile) => Some(LiveRunaway::new(profile.runaway_policy.clone())),
            None => crate::engine::harness_profile::policy_for_node(node_id)
                .ok()
                .map(|policy| LiveRunaway::new(policy.runaway_policy)),
        };
        let mut transcript = String::new();
        let mut scanner = opencode_events::RunEventScanner::default();
        // WO-120: consecutive lines the scanner cannot read. Reset on every readable line — a line
        // or two of vendor noise is weather — and the turn stops when the trip below fires, because a
        // stream Forge cannot read is a turn whose spend the cap cannot see.
        let mut parse_failures: usize = 0;
        let mut on_line = |line: &str| -> Option<StreamStop> {
            // Kept verbatim: the transcript is what the turn is READ from below, so the live view and the recorded
            // view cannot disagree.
            transcript.push_str(line);
            transcript.push('\n');
            match scanner.feed(line) {
                Ok(()) => parse_failures = 0,
                Err(error) => {
                    parse_failures += 1;
                    tracing::warn!(
                        failures = parse_failures,
                        cap = STREAM_PARSE_FAILURE_CAP,
                        error = %error,
                        "opencode-harness: stream line failed to parse"
                    );
                    if let Some(stop) = stream_parse_trip(parse_failures) {
                        return Some(stop);
                    }
                }
            }
            // A FLOOR, never a total: the live build does not reliably emit a `step_finish` for a turn's terminal
            // step (see `engine::opencode_events`). A floor is the safe side to stop on — it can only delay a stop,
            // never cause one for a turn that is still inside its cap.
            let spend = scanner.turn().usage.as_ref().map(|usage| usage.cost_usd);
            let budget = match spend_cap {
                Some(cap) if spend_cap::forge_spend_should_hold(spend, Some(cap)) => {
                    Some(budget_stop(spend, cap))
                }
                _ => None,
            };
            if budget.is_some() {
                return budget;
            }
            // C4, live: the turn's own actions measured against the thresholds its lane's profile resolved — the
            // same fold the trajectory is built from, read as it arrives. A turn that is over budget stops for the
            // BUDGET, which is why this is consulted second: the more specific, cheaper stop wins, and a stuck turn
            // inside its cap still stops here rather than thrashing to the ceiling.
            let stuck = runaway
                .as_mut()
                .and_then(|runaway| runaway.observe(scanner.actions()));
            if let Some(stop) = &stuck {
                tracing::warn!(
                    node = %node_id,
                    code = %stop.code,
                    detail = %stop.detail,
                    "opencode-harness live stop: runaway"
                );
            }
            stuck
        };
        let result = match &self.start_run {
            Some(start) => start(opts, &mut on_line, &live_turn),
            None => start_opencode_run_streaming(opts, &mut on_line, &live_turn),
        };
        drop(on_line);
        // The heartbeat Forge actually observed. Recorded, never acted on: the loop counts the intervals in which
        // the vendor said nothing, and silence is not a verdict — a long tool call is silent and healthy.
        if result.silent_ticks > 0 || result.stop.is_some() {
            let stopped_by = result
                .stop
                .as_ref()
                .map(|stop| stop.code.as_str())
                .unwrap_or("(none)");
            tracing::info!(
                node = %node_id,
                lines = result.lines,
                last_line_ms = result.last_line_ms,
                silent_ticks = result.silent_ticks,
                stopped_by = %stopped_by,
                "opencode-harness turn"
            );
        }
        // A stop from OUTSIDE the turn leaves no `stop` in the result: the process simply died, and without this it
        // would be reported as a crash with no stderr. The reason was stored with the turn precisely so it survives
        // to here.
        let interrupt_reason = live_turn.lock().ok().and_then(|mut slot| {
            let reason = slot.interrupt_reason.clone();
            *slot = crate::engine::opencode_client::LiveTurn::default();
            reason
        });
        if let Some(reason) = interrupt_reason.as_deref() {
            tracing::warn!(
                exit_code = ?result.exit_code,
                reason = %reason,
                "opencode-harness interrupt_observed=true process_reaped=true"
            );
        }
        let stop = result
            .stop
            .clone()
            .or_else(|| interrupt_reason.map(|reason| interrupt_stop(&reason)));
        // TERMINATION TELEMETRY. One event per attempt Forge stopped, carrying the identity the attempt ran under,
        // the canonical reason, and the code and detail a reader argues with. This is the "termination telemetry
        // persisted" half of the runaway contract: the stop is not only an emitted error, it is a recorded decision
        // that names WHY (in the one vocabulary) as well as what.
        //
        // The identity comes from the resolved profile where there is one, and names the node where the seam
        // resolved none — an unresolved turn still stopped for a reason, and a record that omitted it would be
        // least readable exactly where the guard is least expected.
        if let Some(stop) = stop.as_ref() {
            let (profile_id, profile_version) = match profile {
                Some(profile) => (
                    profile.identity.profile_id.to_string(),
                    Some(profile.identity.profile_version),
                ),
                None => ("(unresolved)".to_string(), None),
            };
            tracing::warn!(
                node = %node_id,
                profile = %profile_id,
                profile_version = ?profile_version,
                reason = %stop.reason.label(),
                code = %stop.code,
                detail = %stop.detail,
                "forge termination"
            );
        }
        // Read the V2 structured contract BEFORE judging the turn. A successful exit whose stream Forge cannot
        // read is NOT a successful turn (§3); failing closed here is the difference between "the harness is
        // broken" and "the role said nothing", which Forge would otherwise charge to the model.
        let turn = opencode_events::parse_run_events(&transcript);
        let reported_session = turn
            .as_ref()
            .ok()
            .and_then(|events| events.session_id.clone());
        // What the turn spent.
        //
        // PREFERRED: the vendor's `session export`, whose totals are authoritative. FALLBACK: the sum of this
        // run's own `step_finish` events, used only when the export cannot be read — the live 2.0.21 build does
        // not reliably emit a `step_finish` for a turn's TERMINAL step, so that sum is a LOWER BOUND, never the
        // figure Forge prefers. It is taken rather than dropped because a FAILED turn often has no other
        // reading, and §7 requires that spend not to vanish with the turn.
        let usage = baseline
            .after_turn(reported_session.as_deref())
            .map(|u| crate::engine::harness::HarnessUsage {
                session_id: u.session_id,
                tokens_input: u.tokens_input,
                tokens_output: u.tokens_output,
                cost_usd: u.cost_usd,
            })
            .or_else(|| turn.as_ref().ok().and_then(|events| events.usage.clone()));
        *measured_usage = usage.clone();
        // WHAT THE TURN DID — captured HERE, before any verdict below can return.
        //
        // Every exit from this point on (a Forge stop, a vendor failure, a stream Forge cannot read) still emitted
        // tool calls, and those calls ARE the measurement. Summarizing only on the success path recorded a failed
        // turn as a turn in which nothing happened, which is how a lane that thrashed for twenty edits and then
        // died looked identical to one that never started. `None` stays `None` when the stream could not be parsed:
        // unmeasured is not the same as measured zero, and a zero here would be a fabricated reading.
        if let Ok(events) = &turn {
            *measured_trajectory = Some(crate::engine::trajectory::summarize(&events.tool_calls));
        }
        let spent = usage
            .as_ref()
            .map(|u| {
                format!(
                    " (spent tokens_in={} tokens_out={} cost_usd={:.6} session={})",
                    u.tokens_input, u.tokens_output, u.cost_usd, u.session_id
                )
            })
            .unwrap_or_default();
        if let Some(stop) = stop.as_ref() {
            // A TURN FORGE STOPPED IS NOT A TURN THAT FINISHED. However much text arrived before the stop, it is a
            // truncated answer, and reading it as one is how a capped generation gets recorded as productive. The
            // code, the canonical reason and the detail are all in the record, and the spend measured up to the stop
            // is with them — the reason is what makes the err CLASSIFIABLE (`planning::StopReason`), the code and
            // detail are what make it arguable.
            return Err(WorkflowError::generic(format!(
                "opencode-harness stopped for {node_id}: {} ({}) — {}{spent}",
                stop.code,
                stop.reason.label(),
                stop.detail
            )));
        }
        if result.status != crate::engine::opencode_client::OpenCodeRunStatus::Success {
            // A failed turn still cost money. The error carries the reading so the spend is at least on the
            // record of the failure instead of vanishing with it.
            //
            // The vendor's own error event is preferred when it reported one: V2's stdout is a machine
            // transcript, so pasting it into an exception is noise, and its stderr is often EMPTY on a provider
            // refusal (an unknown model id arrives on stdout as {"name":"UnknownError","message":…}).
            let detail = match &turn {
                Ok(events) => events.error.clone().unwrap_or_else(|| {
                    if result.stderr.trim().is_empty() {
                        format!("no stderr; {} v2 events read", events.events.len())
                    } else {
                        result.stderr.trim().to_string()
                    }
                }),
                Err(error) => format!("unreadable v2 stream: {error}"),
            };
            return Err(WorkflowError::generic(format!(
                "opencode-harness failed for {node_id} exit={:?}{spent}: {detail}",
                result.exit_code
            )));
        }
        let turn = turn.map_err(|error| {
            WorkflowError::generic(format!(
                "opencode-harness produced a v2 stream Forge cannot read for {node_id}{spent}: {error}"
            ))
        })?;
        // An exit 0 that reported a vendor error and produced no output is a failure, not an empty success.
        if turn.assistant_text.trim().is_empty() {
            if let Some(error) = turn.error.as_deref() {
                return Err(WorkflowError::generic(format!(
                    "opencode-harness reported an error with no output for {node_id}{spent}: {error}"
                )));
            }
        }
        // THE V2 SESSION FIX (§4): the id the vendor actually used comes from the turn itself, so a FRESH run
        // persists a real session and the next turn resumes it explicitly. V1 had no such reading and wrote
        // back the id it had *asked* for — `None` on a fresh turn — so a lane's first session was never kept.
        let actual_session = reported_session;
        if let Some(story) = self.story_id.as_deref() {
            if let Err(error) = vendor_session::write_vendor_session_id(
                story,
                VENDOR_SESSION_LANE,
                actual_session.as_deref().or(session.as_deref()),
            ) {
                tracing::warn!(
                    story = %story,
                    error = %error,
                    "opencode-harness: vendor session persist failed"
                );
                return Err(vendor_session_persist_error(
                    "opencode-harness",
                    node_id,
                    story,
                    &error,
                ));
            }
        }
        if let Some(id) = actual_session.as_deref() {
            write_session_id(&cwd, Some(id)).map_err(|error| {
                tracing::warn!(
                    workspace = %cwd,
                    error = %error,
                    "opencode-harness: session marker write failed"
                );
                WorkflowError::generic(format!(
                    "opencode-harness could not persist the session marker for {node_id} in {cwd}: {error}"
                ))
            })?;
        }
        // The role output is the assistant's text (§3), never the NDJSON transcript it arrived in.
        let raw = turn.assistant_text;
        // FACTS ONLY. What the turn left in the repository is reported here; whether it is an acceptable
        // candidate is the delivering lane's judgement (`roles::smith::judge_delivered_candidate`), applied by
        // the lane's hooks the moment this output reaches the lifecycle.
        let candidate_sha = self.run_git(&["rev-parse", "HEAD"]);
        let execution_base = self
            .execution_workspace
            .as_ref()
            .map(|workspace| workspace.base_commit.clone())
            .or(before_sha);
        let assay = if self.assay_commands.is_empty() {
            self.packet.assay_commands.clone()
        } else {
            self.assay_commands.clone()
        };
        Ok(HarnessOutput {
            // The same summary captured above, not a second reading: the success path and the failure paths must
            // report the turn's work from one source or they can disagree about it.
            trajectory: (*measured_trajectory).clone(),
            raw,
            candidate_sha,
            assay_commands: assay,
            acceptance_mapped: self.acceptance_mapped,
            refusal: None,
            execution_base,
            usage,
        })
    }
}
