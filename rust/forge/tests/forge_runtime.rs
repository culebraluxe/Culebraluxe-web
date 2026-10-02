use std::sync::Arc;

use forge::engine::*;
use workflow::{json, ApplicationCommandOutcome, ApplicationCommandRequest, ProcessStatus, Value};

fn runtime() -> (ForgeRuntime, Arc<RecordingWriter>) {
    let writer = Arc::new(RecordingWriter::default());
    let mut rt = ForgeRuntime::in_memory(writer.clone()).expect("XML + topology + seed");
    (rt, writer)
}

fn runtime_compact() -> (ForgeRuntime, Arc<RecordingWriter>) {
    let writer = Arc::new(RecordingWriter::default());
    let mut rt = ForgeRuntime::in_memory_compact(writer.clone()).expect("compact fixture");
    (rt, writer)
}

fn feature_ev() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        scout_required: Some(false),
        ..Default::default()
    }
}
/// A registration is keyed by `(tenant_id, key, version)`, not by the definition's own id — the Neon schema
/// gives `process_definitions.id` a uuid default while the engine's definition carries a human id
/// (`FORGE_SDLC-v6`). Registering the same triple twice must therefore adopt the registered identity instead
/// of adding a second registration, or every engine start against Neon binds a non-uuid into a uuid column
/// (measured 2026-09-29: `invalid input syntax for type uuid: "FORGE_SDLC-v6"`).
#[test]
fn a_second_registration_adopts_the_registered_identity() {
    let writer = Arc::new(RecordingWriter::default());
    let rt = ForgeRuntime::in_memory_compact(writer).expect("compact fixture");
    let registered = |rt: &ForgeRuntime| {
        rt.engine()
            .store()
            .with_tx(|tx| tx.load_definition(FORGE_SDLC_KEY, Some(FORGE_SDLC_VERSION), None))
            .expect("the fixture registers its definition")
    };
    let before = registered(&rt);
    assert_eq!(
        before.id, "forge-sdlc-compact",
        "the fixture's id is the registered identity"
    );

    // The XML definition carries the same key and version under a different (human) id.
    rt.engine()
        .seed_definition(forge_sdlc_definition())
        .expect("re-registration of a registered definition is a no-op");

    let after = registered(&rt);
    assert_eq!(after.id, before.id, "the registered identity wins");
    assert_eq!(
        after.name, before.name,
        "and the registered row is untouched"
    );
}

#[test]
fn topology_of_compact_graph_is_valid() {
    let def = forge_sdlc_definition();
    let top = topology_from_graph(&def.key, def.version, &def.definition);
    ensure_topology(&top).unwrap();
    assert_eq!(def.key, FORGE_SDLC_KEY);
    assert_eq!(def.version, FORGE_SDLC_VERSION);
}

#[test]
fn re_command_is_not_found() {
    let (mut rt, _) = runtime();
    let res = rt.port().dispatch(&ApplicationCommandRequest {
        command_id: "c1".into(),
        command_type: "deal.update".into(),
        subject_type: Some("story".into()),
        subject_id: Some("s1".into()),
        correlation_id: "pi".into(),
        causation_id: None,
        input: json!({}),
    });
    assert_eq!(res.outcome, ApplicationCommandOutcome::NotFound);
}

#[test]
fn role_command_fails_closed() {
    let (mut rt, _) = runtime();
    let res = rt.port().dispatch(&ApplicationCommandRequest {
        command_id: "c1".into(),
        command_type: commands::RUN_SMITH.into(),
        subject_type: Some("story".into()),
        subject_id: Some("s1".into()),
        correlation_id: "pi".into(),
        causation_id: None,
        input: json!({}),
    });
    assert_eq!(res.outcome, ApplicationCommandOutcome::PreconditionFailure);
    assert!(res.message.unwrap().contains("claimed Forge engine task"));
}

#[test]
fn hold_command_writes() {
    let (mut rt, writer) = runtime();
    let res = rt.port().dispatch(&ApplicationCommandRequest {
        command_id: "c1".into(),
        command_type: commands::STORY_MARK_HOLD.into(),
        subject_type: Some("story".into()),
        subject_id: Some("s1".into()),
        correlation_id: "pi".into(),
        causation_id: None,
        input: json!({ "storyId": "s1", "reason": "blocked" }),
    });
    assert_eq!(res.outcome, ApplicationCommandOutcome::Success);
    assert_eq!(
        writer.holds.lock().unwrap().as_slice(),
        &[("s1".into(), "blocked".into())]
    );
}

#[test]
fn start_feature_parks_at_architect() {
    let (mut rt, _) = runtime();
    let started = rt.start_story("story-1", "FEATURE", feature_ev()).unwrap();
    let inst = rt
        .engine()
        .get_process_instance(&started.process_instance_id)
        .unwrap();
    assert_eq!(inst.status, ProcessStatus::Active);
    let tasks = rt.list_role_tasks("story-1").unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].node_id.as_deref(), Some("architect"));
    assert!(tasks[0].candidates.iter().any(|c| c == "architect"));
}

#[test]
fn second_start_same_story_conflicts() {
    let (mut rt, _) = runtime();
    let ev = ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        ..Default::default()
    };
    rt.start_story("story-1", "FEATURE", ev.clone()).unwrap();
    let err = rt.start_story("story-1", "FEATURE", ev).unwrap_err();
    assert_eq!(err.code(), "INSTANCE_ALREADY_ACTIVE");
}

#[test]
fn lead_then_smith_advances() {
    let (mut rt, _) = runtime_compact();
    rt.start_story(
        "story-1",
        "FEATURE",
        ForgeGateEvidence {
            work_type: Some("FEATURE".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let task = &rt.list_role_tasks("story-1").unwrap()[0];
    rt.claim_role_task(&task.task_id, "lead").unwrap();
    let receipt = rt
        .complete_role_task(
            &task.task_id,
            "lead",
            Some("smith"),
            ForgeGateEvidence::default(),
        )
        .unwrap();
    assert_eq!(receipt, completion_receipt_id(&task.task_id));
    let tasks = rt.list_role_tasks("story-1").unwrap();
    assert_eq!(tasks[0].node_id.as_deref(), Some("smith"));
}

#[test]
fn research_parks_at_research_scout() {
    let (mut rt, _) = runtime();
    rt.start_story(
        "story-r",
        "RESEARCH",
        ForgeGateEvidence {
            work_type: Some("RESEARCH".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let tasks = rt.list_role_tasks("story-r").unwrap();
    assert_eq!(tasks[0].node_id.as_deref(), Some("research_scout"));
}

#[test]
fn pending_evidence_wins_over_durable() {
    let durable = ForgeGateEvidence {
        candidate_sha: Some("old".into()),
        qa_verified_sha: Some("old".into()),
        qa_passed: Some(false),
        ..Default::default()
    };
    let pending = ForgeGateEvidence {
        candidate_sha: Some("new".into()),
        qa_verified_sha: Some("new".into()),
        qa_passed: Some(true),
        ..Default::default()
    };
    let merged = pending.merge_over(&durable);
    let facts = merged.to_facts();
    assert_eq!(
        facts.get("candidateSha").and_then(Value::as_str),
        Some("new")
    );
    assert_eq!(facts.get("qaPassed"), Some(&Value::from(true)));
}

#[test]
fn smith_layers_and_fake_edges() {
    let a = SmithWorkNode {
        id: "a".into(),
        purpose: "schema".into(),
        inputs: vec![],
        outputs: vec!["schema".into()],
        depends_on: vec![],
        scope: "db".into(),
    };
    let b = SmithWorkNode {
        id: "b".into(),
        purpose: "ui".into(),
        inputs: vec!["copy".into()],
        outputs: vec!["ui".into()],
        depends_on: vec!["a".into()],
        scope: "web".into(),
    };
    let plan = plan_smith_layers(&[a.clone(), b.clone()], 2);
    assert!(plan.valid);
    assert_eq!(
        plan.layers,
        vec![vec!["a".to_string()], vec!["b".to_string()]]
    );
    assert_eq!(
        fake_edge_candidates(&[a.clone(), b.clone()]),
        vec![("a".to_string(), "b".to_string())]
    );
    let (ok, reason) = split_eligibility(&[a, b]);
    assert!(!ok);
    assert!(reason.contains("sequential"));
}

#[test]
fn resume_refuses_unclaimed_fork_sibling() {
    let (mut rt, _) = runtime();
    let open = OpenForgeTask {
        task_id: "t1".into(),
        node_id: Some("smith".into()),
        claimed: false,
        fork_child: true,
        open_siblings: 1,
    };
    let err = rt.assert_resume_safe(&open).unwrap_err();
    assert_eq!(err.code(), "SPLIT_SIBLING_UNCLAIMED");
}

#[test]
fn start_marks_story_in_progress() {
    let (mut rt, writer) = runtime();
    rt.start_story(
        "story-1",
        "FEATURE",
        ForgeGateEvidence {
            work_type: Some("FEATURE".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        writer.in_progress.lock().unwrap().as_slice(),
        &["story-1".to_string()]
    );
}

#[test]
fn completion_unit_is_exactly_once() {
    let ledger = Arc::new(MemoryLedger::new());
    let writer = Arc::new(RecordingWriter::default());
    let mut rt = ForgeRuntime::in_memory_compact(writer)
        .unwrap()
        .with_ledger(ledger.clone());
    rt.start_story(
        "story-1",
        "FEATURE",
        ForgeGateEvidence {
            work_type: Some("FEATURE".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let task = &rt.list_role_tasks("story-1").unwrap()[0];
    rt.claim_role_task(&task.task_id, "lead").unwrap();
    rt.complete_role_task(
        &task.task_id,
        "lead",
        Some("smith"),
        ForgeGateEvidence {
            candidate_sha: Some("abc".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(ledger
        .has_final(&completion_receipt_id(&task.task_id))
        .unwrap());
    assert_eq!(
        ledger
            .evidence_for("story-1")
            .unwrap()
            .candidate_sha
            .as_deref(),
        Some("abc")
    );
    assert_eq!(rt.reconcile_completions("story-1").unwrap(), 0);
}

#[test]
fn reconcile_finishes_orphaned_transition() {
    let ledger = Arc::new(MemoryLedger::new());
    let writer = Arc::new(RecordingWriter::default());
    let mut rt = ForgeRuntime::in_memory_compact(writer)
        .unwrap()
        .with_ledger(ledger.clone());
    rt.start_story(
        "story-1",
        "FEATURE",
        ForgeGateEvidence {
            work_type: Some("FEATURE".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let task = rt.list_role_tasks("story-1").unwrap()[0].clone();
    rt.claim_role_task(&task.task_id, "lead").unwrap();
    rt.engine()
        .complete_task(workflow::CompleteTaskParams {
            task_id: task.task_id.clone(),
            user_id: "lead".into(),
            form_data: json!({ "candidateSha": "sha" }),
            transition_name: Some("smith".into()),
        })
        .unwrap();
    assert!(!ledger
        .has_final(&completion_receipt_id(&task.task_id))
        .unwrap());
    assert_eq!(rt.reconcile_completions("story-1").unwrap(), 1);
    assert!(ledger
        .has_final(&completion_receipt_id(&task.task_id))
        .unwrap());
}

fn compact() -> ForgeRuntime {
    runtime_compact().0
}

fn advance_to_qa(rt: &mut ForgeRuntime) {
    let ev = ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        ..Default::default()
    };
    rt.start_story("story-1", "FEATURE", ev).unwrap();
    let t = rt.list_role_tasks("story-1").unwrap()[0].clone();
    rt.claim_role_task(&t.task_id, "lead").unwrap();
    rt.complete_role_task(
        &t.task_id,
        "lead",
        Some("solo"),
        ForgeGateEvidence::default(),
    )
    .unwrap();
    let t = rt.list_role_tasks("story-1").unwrap()[0].clone();
    assert_eq!(t.node_id.as_deref(), Some("lead_implement"));
    rt.claim_role_task(&t.task_id, "lead").unwrap();
    rt.complete_role_task(
        &t.task_id,
        "lead",
        Some("complete"),
        ForgeGateEvidence::default(),
    )
    .unwrap();
    let t = rt.list_role_tasks("story-1").unwrap()[0].clone();
    assert_eq!(t.node_id.as_deref(), Some("qa_result"));
}

#[test]
fn publish_without_release_executor_fails_closed() {
    let (mut rt, _) = runtime_compact();
    advance_to_qa(&mut rt);
    let t = rt.list_role_tasks("story-1").unwrap()[0].clone();
    rt.claim_role_task(&t.task_id, "qa").unwrap();
    rt.complete_role_task(&t.task_id, "qa", Some("pass"), ForgeGateEvidence::default())
        .unwrap();
    let inst = rt
        .engine()
        .list_instances(None, None, Some(FORGE_SDLC_KEY), None, 10, 0)
        .unwrap();
    assert_eq!(inst[0].status, ProcessStatus::Error);
}

#[test]
fn publish_with_release_executor_completes() {
    let writer = Arc::new(RecordingWriter::default());
    let mut rt = ForgeRuntime::in_memory_compact_full(writer, Some(Arc::new(OkRelease))).unwrap();
    advance_to_qa(&mut rt);
    let t = rt.list_role_tasks("story-1").unwrap()[0].clone();
    rt.claim_role_task(&t.task_id, "qa").unwrap();
    rt.complete_role_task(&t.task_id, "qa", Some("pass"), ForgeGateEvidence::default())
        .unwrap();
    let inst = rt
        .engine()
        .list_instances(None, None, Some(FORGE_SDLC_KEY), None, 10, 0)
        .unwrap();
    assert_eq!(inst[0].status, ProcessStatus::Completed);
}

#[test]
fn wake_is_idempotent() {
    let (mut rt, _) = runtime();
    let ev = feature_ev();
    let first = rt.wake_story("story-1", "FEATURE", ev.clone()).unwrap();
    assert!(first.started);
    assert_eq!(
        first.open.as_ref().and_then(|o| o.node_id.as_deref()),
        Some("architect")
    );
    let second = rt.wake_story("story-1", "FEATURE", ev).unwrap();
    assert!(!second.started);
    assert_eq!(second.instance_id, first.instance_id);
}

#[test]
fn resume_claims_without_completing() {
    let (mut rt, _) = runtime();
    rt.wake_story("story-1", "FEATURE", feature_ev()).unwrap();
    let open = rt.resume_open("story-1", "architect").unwrap();
    assert!(open.claimed);
    let tasks = rt.list_role_tasks("story-1").unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, workflow::TaskStatus::Reserved);
}

#[test]
fn production_drive_refuses_synthetic_runner() {
    let (mut rt, _) = runtime();
    let err = executor::drive_forge_story(
        &mut rt,
        "story-1",
        executor::DriveForgeStoryOptions {
            work_type: "FEATURE",
            evidence: feature_ev(),
            runner: None,
            allow_synthetic_runner: false,
            max_steps: 4,
            worker_id: "forge",
            split_concurrency: 1,
            stop_after: None,
            turn_cap: executor::DriveForgeStoryOptions::turn_cap_from_env(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("explicit real role runner"));
}

#[test]
fn drive_stops_after_architect() {
    let (mut rt, _) = runtime();
    let out = executor::drive_forge_story(
        &mut rt,
        "story-1",
        executor::DriveForgeStoryOptions {
            work_type: "FEATURE",
            evidence: feature_ev(),
            runner: Some(&executor::DefaultForgeRoleRunner),
            allow_synthetic_runner: true,
            max_steps: 8,
            worker_id: "forge",
            split_concurrency: 1,
            stop_after: Some(executor::ForgeStopTarget::Role("architect")),
            turn_cap: executor::DriveForgeStoryOptions::turn_cap_from_env(),
        },
    )
    .unwrap();
    assert_eq!(out.stopped_after.as_deref(), Some("architect"));
    assert!(out.steps.iter().any(|s| s == "architect"));
}

/// The generation turn cap (§10): the generation STOPS before dispatching past it, and it says which cap it ran into.
///
/// The unit is what V1 measured — a dispatched ROLE turn, "architect, lead_pre, smith, post, qa" — not a vendor step
/// inside one of them. The failure this prevents is not slowness: it is a generation that keeps looking productive one
/// turn at a time and is read as "still working" instead of "looping".
#[test]
fn a_generation_stops_at_the_turn_cap_before_dispatching_past_it() {
    let (mut rt, _) = runtime();
    let out = executor::drive_forge_story(
        &mut rt,
        "story-1",
        executor::DriveForgeStoryOptions {
            work_type: "FEATURE",
            evidence: feature_ev(),
            runner: Some(&executor::DefaultForgeRoleRunner),
            allow_synthetic_runner: true,
            // Room for many waves: what stops this generation has to be the CAP rather than the wave ceiling.
            max_steps: 20,
            worker_id: "forge",
            split_concurrency: 1,
            stop_after: None,
            turn_cap: 1,
        },
    )
    .unwrap();

    assert_eq!(
        out.steps.len(),
        1,
        "one turn is one dispatch, and nothing may run past the cap: {:?}",
        out.steps
    );
    let reason = out
        .blocked_reason
        .expect("a capped generation must name itself in its own record");
    assert!(reason.contains("MODEL_TURN_CAP"), "{reason}");
    assert!(
        reason.contains("already dispatched 1 turns (cap 1)"),
        "the reason must state the count and the cap: {reason}"
    );
    assert!(
        reason.contains("FORGE_MAX_MODEL_TURNS_PER_GENERATION"),
        "and must say how to authorise a longer run: {reason}"
    );
}

#[test]
fn evidence_marker_does_not_invent_lead_decision_on_lead_pre() {
    let ev = role_mapping::parse_forge_evidence_marker(
        "prose\nFORGE_EVIDENCE_JSON: {\"leadDecision\":\"SMITH\",\"qaPassed\":true}\n",
    );
    assert_eq!(ev.lead_decision.as_deref(), Some("SMITH"));
    assert_eq!(ev.qa_passed, Some(true));
    let lead = agents::forge_agent_collect(
        "lead_pre",
        ForgeGateEvidence::default(),
        "FORGE_EVIDENCE_JSON: {\"leadDecision\":\"SMITH\"}",
        &phase::RoleEffectPorts::default(),
    )
    .unwrap();
    assert!(lead.lead_decision.is_none());
}

#[test]
fn wave_defers_overlapping_surfaces() {
    let lanes = vec![
        executor::WaveLane {
            lane: "a".into(),
            surface: Some(vec!["src/foo.rs".into()]),
            fanout: false,
            task: 1,
        },
        executor::WaveLane {
            lane: "b".into(),
            surface: Some(vec!["src/foo.rs".into()]),
            fanout: false,
            task: 2,
        },
        executor::WaveLane {
            lane: "c".into(),
            surface: Some(vec!["src/bar.rs".into()]),
            fanout: false,
            task: 3,
        },
    ];
    let plan = executor::plan_wave(&lanes, 2);
    assert!(!plan.refusals.is_empty());
    assert!(plan.batches.iter().all(|b| b.len() <= 2));
}

struct ScriptedHarness {
    raw: String,
    sha: Option<String>,
    commands: Vec<String>,
    mapped: bool,
    cmd_ok: bool,
}

impl runner::RoleHarness for ScriptedHarness {
    /// Where this harness would run commands from. A scripted harness does not shell out, so the test's own working
    /// directory is the honest answer — and it has to be declared because `assay_cwd` is part of the trait.
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_role(
        &self,
        _n: &str,
        _t: &runtime::ActiveForgeRoleTask,
        _d: Option<&str>,
    ) -> workflow::Result<runner::HarnessOutput> {
        Ok(runner::HarnessOutput {
            raw: self.raw.clone(),
            candidate_sha: self.sha.clone(),
            assay_commands: self.commands.clone(),
            acceptance_mapped: self.mapped,
            refusal: None,
            execution_base: None,
            usage: None,
        })
    }
    fn exists_on_base_ref(&self, _b: &str, _p: &str) -> bool {
        true
    }
    fn run_command(&self, command: &str) -> assay::CommandResult {
        assay::CommandResult {
            command: command.into(),
            exit_code: if self.cmd_ok { 0 } else { 1 },
            passed: self.cmd_ok,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }
    }
}

#[test]
fn production_runner_holds_architect_without_handoff() {
    let h = ScriptedHarness {
        raw: "I thought about the plan".into(),
        sha: None,
        commands: vec![],
        mapped: false,
        cmd_ok: true,
    };
    let role = runner::ProductionRoleRunner::new(&h, ForgeGateEvidence::default());
    let task = runtime::ActiveForgeRoleTask {
        task_id: "t".into(),
        process_instance_id: "p".into(),
        story_id: "s".into(),
        token_id: Some("k".into()),
        node_id: Some("architect".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],
    };
    let out = executor::ForgeRoleRunner::run(&role, "architect", &task).unwrap();
    assert!(out
        .evidence
        .deliverable_rejection
        .as_deref()
        .unwrap_or("")
        .contains("ARCHITECT"));
}

/// A held lane names the story it was listed for — never the process-instance UUID.
///
/// 2026-09-29: the runner substituted the process-instance UUID for the story id at all three write sites.
/// `forge_hold_record.story_id` is a foreign key to `storyboard_story(id)`, whose ids are human keys, so the
/// hold row was rejected and the rejection was discarded with `let _ =`: a rejected deliverable recorded
/// nothing at all. This test fails on the old code (`"s"` was replaced by `"p"`).
#[test]
fn a_held_lane_writes_the_story_id_not_the_instance_uuid() {
    let h = ScriptedHarness {
        raw: "I thought about the plan".into(),
        sha: None,
        commands: vec![],
        mapped: false,
        cmd_ok: true,
    };
    let writer = RecordingWriter::default();
    let mut role = runner::ProductionRoleRunner::new(&h, ForgeGateEvidence::default());
    role.writer = Some(&writer as &dyn ForgeStateWriter);
    let task = runtime::ActiveForgeRoleTask {
        task_id: "t".into(),
        process_instance_id: "p".into(),
        story_id: "ENG-GUARD-REPO-RUST-01".into(),
        token_id: Some("k".into()),
        node_id: Some("architect".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],
    };
    let out = executor::ForgeRoleRunner::run(&role, "architect", &task).unwrap();
    assert!(out.evidence.deliverable_rejection.is_some());

    let holds = writer.holds.lock().unwrap();
    assert_eq!(holds[0].0, "ENG-GUARD-REPO-RUST-01");
    let opened = writer.opened_holds.lock().unwrap();
    assert_eq!(opened[0].0, "ENG-GUARD-REPO-RUST-01");
    assert_eq!(opened[0].1, "DELIVERABLE_REJECTED");
    assert_ne!(opened[0].0, task.process_instance_id);
}

/// A task with no story id is refused before anything is written: an identity-bearing record is never written
/// against an identity the run does not have, and never against a substitute.
#[test]
fn a_role_task_without_a_story_id_is_refused() {
    let h = ScriptedHarness {
        raw: "I thought about the plan".into(),
        sha: None,
        commands: vec![],
        mapped: false,
        cmd_ok: true,
    };
    let writer = RecordingWriter::default();
    let mut role = runner::ProductionRoleRunner::new(&h, ForgeGateEvidence::default());
    role.writer = Some(&writer as &dyn ForgeStateWriter);
    let task = runtime::ActiveForgeRoleTask {
        task_id: "t".into(),
        process_instance_id: "p".into(),
        story_id: String::new(),
        token_id: Some("k".into()),
        node_id: Some("architect".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],
    };
    let error = match executor::ForgeRoleRunner::run(&role, "architect", &task) {
        Ok(_) => panic!("a story-less task is refused"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("no story id"), "{error}");
    assert!(writer.holds.lock().unwrap().is_empty());
    assert!(writer.opened_holds.lock().unwrap().is_empty());
}

/// A hold that cannot be recorded fails the lane instead of vanishing behind `let _ =`.
#[test]
fn a_hold_that_cannot_be_recorded_fails_the_lane() {
    struct BrokenHold;
    impl ForgeStateWriter for BrokenHold {
        fn mark_story_human_hold(&self, _s: &str, _r: &str) -> Result<(), String> {
            Ok(())
        }
        fn mark_story_complete(&self, _s: &str) -> Result<(), String> {
            Ok(())
        }
        fn mark_story_in_progress(&self, _s: &str) -> Result<(), String> {
            Ok(())
        }
        fn stamp_run_candidate(&self, _r: &str, _s: &str) -> Result<(), String> {
            Ok(())
        }
        fn append_run_detail(&self, _r: &str, _d: &str) -> Result<(), String> {
            Ok(())
        }
        fn open_hold(&self, _i: &forge::engine::hold::OpenHold) -> Result<String, String> {
            Err("no such story".into())
        }
        fn record_tool_artifact(&self, _i: &db::NewToolArtifact) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn record_run_usage(
            &self,
            _r: &str,
            _u: &forge::engine::harness_usage::HarnessUsage,
        ) -> Result<(), String> {
            Ok(())
        }
    }
    let h = ScriptedHarness {
        raw: "I thought about the plan".into(),
        sha: None,
        commands: vec![],
        mapped: false,
        cmd_ok: true,
    };
    let writer = BrokenHold;
    let mut role = runner::ProductionRoleRunner::new(&h, ForgeGateEvidence::default());
    role.writer = Some(&writer as &dyn ForgeStateWriter);
    let task = runtime::ActiveForgeRoleTask {
        task_id: "t".into(),
        process_instance_id: "p".into(),
        story_id: "ENG-GUARD-REPO-RUST-01".into(),
        token_id: Some("k".into()),
        node_id: Some("architect".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["architect".into()],
    };
    let error = match executor::ForgeRoleRunner::run(&role, "architect", &task) {
        Ok(_) => panic!("an unrecordable hold is a failed lane"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("forge_hold_record"), "{error}");
    assert!(
        error.to_string().contains("ENG-GUARD-REPO-RUST-01"),
        "{error}"
    );
}

/// The QA lane's own measurement is recorded as a `forge_tool_artifact` row keyed to the run it executed
/// (migration 130) — through the state-writer port, so a writer-less run records nothing rather than writing into
/// whichever pool happens to be installed.
#[test]
fn a_qa_lane_records_its_measurement_as_an_artifact() {
    let h = ScriptedHarness {
        raw: "assay complete".into(),
        sha: Some("abc1234".into()),
        commands: vec!["cargo test -p db".into()],
        mapped: true,
        cmd_ok: true,
    };
    let writer = RecordingWriter::default();
    let role = runner::ProductionRoleRunner::new(
        &h,
        ForgeGateEvidence {
            candidate_sha: Some("abc1234".into()),
            ..Default::default()
        },
    )
    .with_story_run(Some("11111111-1111-1111-1111-111111111111".into()));
    let mut role = role;
    role.writer = Some(&writer as &dyn ForgeStateWriter);
    let task = runtime::ActiveForgeRoleTask {
        task_id: "t".into(),
        process_instance_id: "p".into(),
        story_id: "ENG-PROOF-ARTIFACT-01".into(),
        token_id: Some("k".into()),
        node_id: Some("qa_verify".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["qa_verify".into()],
    };
    executor::ForgeRoleRunner::run(&role, "qa_verify", &task)
        .expect("a clean assay is a clean lane");

    let recorded = writer.artifacts.lock().unwrap();
    assert_eq!(recorded.len(), 1, "one measurement, one artifact");
    let artifact = &recorded[0];
    assert_eq!(artifact.story_id, "ENG-PROOF-ARTIFACT-01");
    assert_eq!(
        artifact.story_run_id.as_deref(),
        Some("11111111-1111-1111-1111-111111111111"),
        "the artifact names the execution it came out of"
    );
    assert_eq!(artifact.tool, "assay");
    assert_eq!(artifact.kind, "qa-assay-evidence");
    assert_eq!(artifact.verdict.as_deref(), Some("PASS"));
    assert_eq!(artifact.sha.as_deref(), Some("abc1234"));
}

/// A failing assay records `FAIL`, and a lane that measured nothing records `UNPROVEN` — three answers, because a
/// measurement nobody took is not a failed measurement.
#[test]
fn the_recorded_verdict_is_the_lanes_own_reading() {
    for (commands, mapped, cmd_ok, expected) in [
        (vec!["cargo test".to_string()], true, false, "FAIL"),
        (vec![], true, true, "FAIL"),
        (vec!["cargo test".to_string()], false, true, "UNPROVEN"),
    ] {
        let h = ScriptedHarness {
            raw: "assay".into(),
            sha: None,
            commands,
            mapped,
            cmd_ok,
        };
        let writer = RecordingWriter::default();
        let mut role = runner::ProductionRoleRunner::new(&h, ForgeGateEvidence::default());
        role.writer = Some(&writer as &dyn ForgeStateWriter);
        let task = runtime::ActiveForgeRoleTask {
            task_id: "t".into(),
            process_instance_id: "p".into(),
            story_id: "ENG-PROOF-ARTIFACT-02".into(),
            token_id: None,
            node_id: Some("qa_verify".into()),
            status: workflow::TaskStatus::Ready,
            assignee: None,
            candidates: vec!["qa_verify".into()],
        };
        executor::ForgeRoleRunner::run(&role, "qa_verify", &task).expect("the lane completes");
        let recorded = writer.artifacts.lock().unwrap();
        assert_eq!(
            recorded[0].verdict.as_deref(),
            Some(expected),
            "the lane's own reading is what is recorded"
        );
    }
}

/// A measurement the engine cannot record is a failed lane, exactly as an unrecordable hold is: nothing here is
/// `let _ =`d, so evidence that could not be stored is visible as a failure rather than as silence.
#[test]
fn an_artifact_that_cannot_be_recorded_fails_the_lane() {
    struct BrokenArtifact;
    impl ForgeStateWriter for BrokenArtifact {
        fn mark_story_human_hold(&self, _s: &str, _r: &str) -> Result<(), String> {
            Ok(())
        }
        fn mark_story_complete(&self, _s: &str) -> Result<(), String> {
            Ok(())
        }
        fn mark_story_in_progress(&self, _s: &str) -> Result<(), String> {
            Ok(())
        }
        fn stamp_run_candidate(&self, _r: &str, _s: &str) -> Result<(), String> {
            Ok(())
        }
        fn append_run_detail(&self, _r: &str, _d: &str) -> Result<(), String> {
            Ok(())
        }
        fn open_hold(&self, _i: &forge::engine::hold::OpenHold) -> Result<String, String> {
            Ok("hold-1".into())
        }
        fn record_tool_artifact(&self, _i: &db::NewToolArtifact) -> Result<Option<String>, String> {
            Err("artifact table is unreachable".into())
        }
        fn record_run_usage(
            &self,
            _r: &str,
            _u: &forge::engine::harness_usage::HarnessUsage,
        ) -> Result<(), String> {
            Ok(())
        }
    }
    let h = ScriptedHarness {
        raw: "assay complete".into(),
        sha: None,
        commands: vec!["cargo test".into()],
        mapped: true,
        cmd_ok: true,
    };
    let writer = BrokenArtifact;
    let mut role = runner::ProductionRoleRunner::new(&h, ForgeGateEvidence::default());
    role.writer = Some(&writer as &dyn ForgeStateWriter);
    let task = runtime::ActiveForgeRoleTask {
        task_id: "t".into(),
        process_instance_id: "p".into(),
        story_id: "ENG-PROOF-ARTIFACT-03".into(),
        token_id: None,
        node_id: Some("qa_verify".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["qa_verify".into()],
    };
    let error = match executor::ForgeRoleRunner::run(&role, "qa_verify", &task) {
        Ok(_) => panic!("a measurement nobody could store is a failed lane"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("record_tool_artifact"),
        "{error}"
    );
    assert!(
        error.to_string().contains("ENG-PROOF-ARTIFACT-03"),
        "{error}"
    );
}

#[test]
fn assay_empty_plan_fails() {
    let report = assay::adjudicate_assay(&[], &[], true);
    assert_eq!(report.verdict, assay::AssayVerdict::Fail);
    assert_eq!(report.blockers, vec!["NO_ASSAY_COMMANDS"]);
}

#[test]
fn publish_without_qa_pass_records_conflict() {
    use release::{ClosedReleaseOps, DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope};
    struct Mem;
    impl EvidenceStore for Mem {
        fn read(&self, _: &str) -> ForgeGateEvidence {
            ForgeGateEvidence {
                candidate_sha: Some("abc1234".into()),
                qa_passed: Some(false),
                ..Default::default()
            }
        }
        fn merge(&self, _: &str, _: &str, patch: ForgeGateEvidence) {
            assert_eq!(patch.publish_succeeded, Some(false));
        }
        fn latest_refresh_command_id(&self, _: &str) -> Option<String> {
            None
        }
        fn frozen_proofs(&self, _: &str) -> Vec<String> {
            vec![]
        }
    }
    let exec = DbForgeReleaseExecutor {
        operations: ClosedReleaseOps,
        evidence: Mem,
        pending: None,
    };
    let r = exec.execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: "c1".into(),
        process_instance_id: "p".into(),
        story_id: "s".into(),
    });
    assert_eq!(r.outcome, ApplicationCommandOutcome::Success);
    assert!(r.message.unwrap().contains("QA has not passed"));
}

/// `FORGE_ALLOW_PUBLISH` is a KILL switch: unset publishes. Read as an opt-in key (`== Some("1")`) it refused
/// the candidate of every run launched outside the scheduler's `.env.scheduler`, and filed the refusal under a
/// git conflict's name — the exact shape that left TST-ACCOUNTING-CORE-008's 453-line candidate off
/// `origin/main` on 2026-10-01. Held as a pure predicate because the process environment is global and these
/// tests run in parallel, where `set_var` would reach across them.
#[test]
fn the_publish_switch_is_off_only_when_said_in_words() {
    use git_publish::publish_switch_off;
    assert!(
        !publish_switch_off(None),
        "an unset switch publishes: absence is not a refusal"
    );
    assert!(!publish_switch_off(Some("1")));
    assert!(
        !publish_switch_off(Some("  1  ")),
        "padding is not a refusal"
    );
    assert!(
        !publish_switch_off(Some("")),
        "empty reads as unset, not as off"
    );
    assert!(publish_switch_off(Some("0")));
    assert!(publish_switch_off(Some("false")));
    assert!(publish_switch_off(Some(" OFF ")));
    assert!(publish_switch_off(Some("no")));
}

/// A refused publish filed as `PUBLISH_CONFLICT` reads like "remote main advanced" — so the operator goes
/// looking for a merge conflict that never existed while the candidate sits on a branch House Rule 1 will not
/// let anyone push. It is filed under its own name now, so the cause is greppable instead of inferred.
#[test]
fn a_publish_refused_by_the_switch_is_not_filed_as_a_git_conflict() {
    use release::{
        DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope, ForgeOperationResult,
        ForgeReleaseOperations, PublishOutcome,
    };
    use std::sync::Mutex;

    struct SwitchOff;
    impl ForgeReleaseOperations for SwitchOff {
        fn apply_migrations(&self, _: &str, _: &[String], _: &str) -> ForgeOperationResult {
            ForgeOperationResult {
                success: false,
                detail: "not this test's subject".into(),
            }
        }
        fn verify_migrations(&self, _: &str, _: &[String]) -> ForgeOperationResult {
            self.apply_migrations("", &[], "")
        }
        fn refresh_derived(&self, _: &[String], _: &str) -> ForgeOperationResult {
            self.apply_migrations("", &[], "")
        }
        fn verify_derived(&self, _: &[String], _: &str) -> ForgeOperationResult {
            self.apply_migrations("", &[], "")
        }
        fn publish(&self, _: Option<&str>, _: &[String]) -> PublishOutcome {
            PublishOutcome::PublishDisabled {
                reason: "publication disabled by FORGE_ALLOW_PUBLISH".into(),
            }
        }
    }
    /// Records the `failure_class` the publisher filed, which is the whole claim under test.
    struct Spy(Arc<Mutex<Option<String>>>);
    impl EvidenceStore for Spy {
        fn read(&self, _: &str) -> ForgeGateEvidence {
            ForgeGateEvidence {
                candidate_sha: Some("a".repeat(40)),
                qa_passed: Some(true),
                ..Default::default()
            }
        }
        fn merge(&self, _: &str, _: &str, patch: ForgeGateEvidence) {
            *self.0.lock().expect("spy lock") = patch.failure_class.clone();
        }
        fn latest_refresh_command_id(&self, _: &str) -> Option<String> {
            None
        }
        fn frozen_proofs(&self, _: &str) -> Vec<String> {
            Vec::new()
        }
    }

    let seen = Arc::new(Mutex::new(None));
    let exec = DbForgeReleaseExecutor {
        operations: SwitchOff,
        evidence: Spy(seen.clone()),
        pending: None,
    };
    let result = exec.execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: "c1".into(),
        process_instance_id: "p".into(),
        story_id: "s".into(),
    });
    assert_eq!(result.outcome, ApplicationCommandOutcome::Success);
    let filed = seen.lock().expect("spy lock").clone();
    assert_eq!(
        filed.as_deref(),
        Some("PUBLISH_DISABLED"),
        "the switch's refusal must be filed under its own name"
    );
    assert_ne!(
        filed.as_deref(),
        Some("PUBLISH_CONFLICT"),
        "a configured refusal is not a git conflict, and filing it as one is what hid the cause"
    );
}

/// The publish path against real git and a real remote.
///
/// House Rule 1 refuses every ref but `main`, so `preview_publish` is the only door a candidate can leave
/// through — which makes "does a candidate land?" a question that has to be answered against an actual
/// `origin`, not a mock: fetch, ancestry, merge-tree, commit-tree, push. A candidate is left on its branch
/// exactly as a Smith leaves it, published, and then read back **off the remote**, because the claim under
/// test is about `origin/main` and not about a return value.
///
/// This is the failure TST-ACCOUNTING-CORE-008 suffered on 2026-10-01: QA passed, the candidate existed, and
/// the run refused to publish it. With the switch unset, that run lands its work.
#[test]
fn the_publish_path_lands_a_candidate_on_a_real_remote() {
    use git_publish::{preview_publish, publish_switch_off};
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    fn run(dir: &Path, args: &[&str]) -> String {
        let out = Command::new(worktree::git_binary())
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    let switch = std::env::var("FORGE_ALLOW_PUBLISH").ok();
    assert!(
        !publish_switch_off(switch.as_deref()),
        "this test proves a candidate lands, so it needs the switch unset or on; it is {switch:?}"
    );

    let root = std::env::temp_dir().join(format!("forge-publish-e2e-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let remote = root.join("origin.git");
    let work = root.join("work");
    fs::create_dir_all(&work).expect("temp work");

    // The working checkout that stands in for the engine's, and a bare remote whose `main` is its one commit.
    // The remote is seeded by a bare CLONE, not a push: `preview_publish` is the only code in the workspace
    // allowed to name a mutation verb (arch_boundary__011), and a test fixture is not an exception to that.
    run(&work, &["init", "--initial-branch=main", "."]);
    run(&work, &["config", "user.email", "forge@test.invalid"]);
    run(&work, &["config", "user.name", "forge test"]);
    run(&work, &["config", "commit.gpgsign", "false"]);
    fs::write(work.join("README.md"), "base\n").expect("seed");
    run(&work, &["add", "."]);
    run(&work, &["commit", "-m", "base"]);
    let work_path = work.to_string_lossy().to_string();
    let remote_url = remote.to_string_lossy().to_string();
    run(&root, &["clone", "--bare", "-q", &work_path, &remote_url]);
    run(&work, &["remote", "add", "origin", &remote_url]);
    run(&work, &["fetch", "-q", "origin", "main"]);
    let base = run(&work, &["rev-parse", "HEAD"]);

    // The candidate: a commit on the lane's own branch, the way the Smith leaves it.
    run(&work, &["checkout", "-b", "agent/tst-story/run-1"]);
    fs::write(work.join("candidate.rs"), "// the smith's work\n").expect("candidate");
    run(&work, &["add", "."]);
    run(&work, &["commit", "-m", "smith: candidate"]);
    let candidate = run(&work, &["rev-parse", "HEAD"]);
    assert_ne!(candidate, base, "the candidate has to be its own commit");

    match preview_publish(&work, &candidate) {
        release::PublishOutcome::Published {
            published_main_hash,
        } => {
            assert_eq!(published_main_hash, candidate);
        }
        other => panic!("the candidate did not land on main: {other:?}"),
    }

    // Read it back off the remote: the deliverable is what `origin/main` now contains.
    assert_eq!(
        run(&remote, &["rev-parse", "main"]),
        candidate,
        "origin/main must BE the candidate"
    );
    assert_eq!(
        run(&remote, &["show", "main:candidate.rs"]),
        "// the smith's work",
        "the smith's file must be readable on the remote's main"
    );

    // Publishing it again must be a no-op: the candidate is already on main, so the answer says it is there
    // and main does not move. This is the branch that keeps two Smiths from fighting over the same base.
    run(&work, &["fetch", "origin", "main"]);
    match preview_publish(&work, &candidate) {
        release::PublishOutcome::Published {
            published_main_hash,
        }
        | release::PublishOutcome::IntegratedAndPublished {
            published_main_hash,
        } => {
            assert_eq!(published_main_hash, candidate);
        }
        other => panic!("an already-landed candidate must read as landed, not retried: {other:?}"),
    }
    assert_eq!(
        run(&remote, &["rev-parse", "main"]),
        candidate,
        "an already-integrated candidate must not move main"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn opencode_argv_pins_model_and_auto() {
    let args = opencode_client::build_opencode_run_args(
        opencode::OPENCODE_PINNED_MODEL,
        "do the smith work",
        true,
        None,
        false,
        None,
    );
    assert_eq!(
        args,
        vec![
            "run",
            "--standalone",
            "--format",
            "json",
            "--model",
            "deepseek/deepseek-flash",
            "--auto",
            "do the smith work"
        ]
    );
}

#[test]
fn opencode_argv_session_pins_id() {
    let args = opencode_client::build_opencode_run_args(
        opencode::OPENCODE_PINNED_MODEL,
        "resume",
        true,
        Some("sess-1"),
        false,
        None,
    );
    assert!(args.windows(2).any(|w| w == ["--session", "sess-1"]));
    assert!(!args.iter().any(|a| a == "--continue"));
}

#[test]
fn empty_explicit_model_refuses_opencode_default() {
    let err = opencode::resolve_opencode_model(Some("  ")).unwrap_err();
    assert!(err.to_string().contains("got empty"));
}

#[test]
fn packet_includes_isolation_and_architect_brief() {
    let text = packet::build_task_text(
        "architect",
        "t1",
        &packet::StoryPacket {
            id: "s1".into(),
            title: "Wire neon".into(),
            goal: Some("connect".into()),
            architect_brief: Some("use existing URL".into()),
            ..Default::default()
        },
        Some(&packet::ExecutionWorkspace {
            worktree_path: "/tmp/wt".into(),
            branch_name: "forge/s1".into(),
            base_ref: "main".into(),
            base_commit: "abc1234def".into(),
        }),
    );
    assert!(text.contains("Architect brief: use existing URL"));
    assert!(text.contains("isolated Git worktree on branch forge/s1"));
    assert!(text.contains("Do NOT push"));
}

#[test]
fn publish_preview_no_candidate() {
    let out = git_publish::preview_publish(std::path::Path::new("."), "");
    match out {
        release::PublishOutcome::NoCandidate { .. } => {}
        other => panic!("{other:?}"),
    }
}
