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
    assert_eq!(after.name, before.name, "and the registered row is untouched");
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
    assert!(ledger.has_final(&completion_receipt_id(&task.task_id)));
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
    assert!(!ledger.has_final(&completion_receipt_id(&task.task_id)));
    assert_eq!(rt.reconcile_completions("story-1").unwrap(), 1);
    assert!(ledger.has_final(&completion_receipt_id(&task.task_id)));
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
        },
    )
    .unwrap();
    assert_eq!(out.stopped_after.as_deref(), Some("architect"));
    assert!(out.steps.iter().any(|s| s == "architect"));
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
    ) -> workflow::Result<runner::HarnessOutput> {
        Ok(runner::HarnessOutput {
            raw: self.raw.clone(),
            candidate_sha: self.sha.clone(),
            assay_commands: self.commands.clone(),
            acceptance_mapped: self.mapped,
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
        fn append_run_detail(&self, _r: &str, _d: &str) -> Result<(), String> {
            Ok(())
        }
        fn open_hold(&self, _i: &forge::engine::hold::OpenHold) -> Result<String, String> {
            Err("no such story".into())
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
    assert!(error.to_string().contains("ENG-GUARD-REPO-RUST-01"), "{error}");
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

#[test]
fn opencode_argv_pins_model_and_auto() {
    let args = opencode_client::build_opencode_run_args(
        opencode::OPENCODE_PINNED_MODEL,
        "do the smith work",
        true,
        None,
        false,
    );
    assert_eq!(
        args,
        vec![
            "run",
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
