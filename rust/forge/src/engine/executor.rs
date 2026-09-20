//! Port of `workflow_app/forge/forge-executor.ts`.
//! Production drive refuses the synthetic runner.

use std::collections::BTreeSet;

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::path::shared_path;
use crate::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use crate::engine::writer::ForgeStateWriter;
use workflow::{Result, TaskStatus, TxStore, WorkflowError};

pub struct ForgeRoleOutcome {
    pub transition_name: Option<String>,
    pub evidence: ForgeGateEvidence,
}

pub trait ForgeRoleRunner: Send + Sync {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome>;
}

pub fn default_evidence_for(node_id: &str) -> ForgeGateEvidence {
    match node_id {
        "lead_pre" => ForgeGateEvidence {
            lead_decision: Some("SOLO".into()),
            ..Default::default()
        },
        "feature_scout" | "research_scout" | "diagnose_scout" | "repair_scout" => {
            ForgeGateEvidence {
                scout_required: Some(false),
                extra: Default::default(),
                ..Default::default()
            }
        }
        "qa_review" => ForgeGateEvidence {
            qa_review_required: Some(false),
            qa_review_passed: Some(true),
            ..Default::default()
        },
        "qa_verify" | "fast_qa_verify" => ForgeGateEvidence {
            qa_passed: Some(true),
            publish_succeeded: Some(true),
            migration_required: Some(false),
            derived_refresh_required: Some(false),
            deployment_required: Some(false),
            ..Default::default()
        },
        "production_smoke" => ForgeGateEvidence {
            production_verified: Some(true),
            ..Default::default()
        },
        _ => ForgeGateEvidence::default(),
    }
}

/// Test-only synthetic runner. Production drive must pass a real runner.
pub struct DefaultForgeRoleRunner;

impl ForgeRoleRunner for DefaultForgeRoleRunner {
    fn run(&self, node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence: default_evidence_for(node_id),
        })
    }
}

pub static FORGE_HUMAN_GATE_NODES: &[&str] = &["hold", "repair_requirements", "fast_confirmation"];

#[derive(Debug, Clone)]
pub enum ForgeStopTarget {
    Role(&'static str),
    Node(String),
}

pub fn resolve_forge_stop_target(stop: Option<&ForgeStopTarget>) -> Option<BTreeSet<String>> {
    match stop {
        None => None,
        Some(ForgeStopTarget::Node(n)) => Some(BTreeSet::from([n.clone()])),
        Some(ForgeStopTarget::Role("scout")) => Some(
            ["feature_scout", "research_scout", "diagnose_scout", "repair_scout"]
                .into_iter()
                .map(String::from)
                .collect(),
        ),
        Some(ForgeStopTarget::Role("architect")) => Some(
            ["architect", "research_architect", "repair_architect"]
                .into_iter()
                .map(String::from)
                .collect(),
        ),
        Some(ForgeStopTarget::Role("lead")) => Some(BTreeSet::from(["lead_pre".into()])),
        Some(ForgeStopTarget::Role(_)) => None,
    }
}

pub fn is_advance_conflict(err: &WorkflowError) -> bool {
    let m = err.to_string();
    let c = err.code();
    matches!(
        c,
        "STALE_TASK" | "TASK_ALREADY_COMPLETED" | "PROCESS_NOT_ACTIVE" | "TASK_NOT_CLAIMABLE"
    ) || m.to_ascii_lowercase().contains("already completed")
        || m.to_ascii_lowercase().contains("not active")
        || m.to_ascii_lowercase().contains("state changed")
}

pub fn is_completed_release_conflict(err: &WorkflowError) -> bool {
    err.code() == "TASK_ALREADY_COMPLETED"
        || err
            .to_string()
            .to_ascii_lowercase()
            .contains("cannot be released in status: completed")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneFailureSettlement {
    Released,
    AlreadyCompleted,
}

pub fn settle_forge_lane_failure<S: TxStore>(
    rt: &ForgeRuntime<S>,
    task_id: &str,
    actor: &str,
    lane_err: &WorkflowError,
) -> Result<LaneFailureSettlement> {
    match rt.release_role_task(task_id, actor) {
        Ok(()) => Ok(LaneFailureSettlement::Released),
        Err(e) if is_completed_release_conflict(&e) => {
            Ok(LaneFailureSettlement::AlreadyCompleted)
        }
        Err(e) if is_advance_conflict(&e) => Ok(LaneFailureSettlement::Released),
        Err(release_err) => Err(WorkflowError::generic(format!(
            "Forge role failed and its task could not be released: lane={lane_err}; release={release_err}"
        ))),
    }
}

#[derive(Debug, Clone)]
pub struct WaveLane<T> {
    pub lane: String,
    pub surface: Option<Vec<String>>,
    pub fanout: bool,
    pub task: T,
}

#[derive(Debug, Clone)]
pub struct WaveRefusal {
    pub lanes: (String, String),
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct WavePlan<T> {
    pub batches: Vec<Vec<WaveLane<T>>>,
    pub refusals: Vec<WaveRefusal>,
}

fn has_surface<T>(lane: &WaveLane<T>) -> bool {
    lane.surface.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
}

pub fn plan_wave<T: Clone>(lanes: &[WaveLane<T>], cap: usize) -> WavePlan<T> {
    let limit = cap.max(1);
    let mut refusals = Vec::new();
    if limit > 1 {
        let known: Vec<&WaveLane<T>> = lanes.iter().filter(|l| !l.fanout && has_surface(l)).collect();
        for i in 0..known.len() {
            for j in i + 1..known.len() {
                if let (Some(a), Some(b)) = (&known[i].surface, &known[j].surface) {
                    if let Some(path) = shared_path(a, b) {
                        refusals.push(WaveRefusal {
                            lanes: (known[i].lane.clone(), known[j].lane.clone()),
                            path,
                        });
                    }
                }
            }
        }
    }
    let mut batches: Vec<Vec<WaveLane<T>>> = Vec::new();
    for lane in lanes {
        if !lane.fanout && !has_surface(lane) {
            batches.push(vec![lane.clone()]);
            continue;
        }
        let mut placed = false;
        for batch in batches.iter_mut() {
            if batch.len() >= limit {
                continue;
            }
            let compatible = batch.iter().all(|member| {
                if lane.fanout {
                    return member.fanout;
                }
                if member.fanout || !has_surface(member) {
                    return false;
                }
                shared_path(
                    lane.surface.as_deref().unwrap_or(&[]),
                    member.surface.as_deref().unwrap_or(&[]),
                )
                .is_none()
            });
            if !compatible {
                continue;
            }
            batch.push(lane.clone());
            placed = true;
            break;
        }
        if !placed {
            batches.push(vec![lane.clone()]);
        }
    }
    WavePlan { batches, refusals }
}

#[derive(Debug, Clone)]
pub struct DriveForgeStoryResult {
    pub instance_id: String,
    pub status: String,
    pub steps: Vec<String>,
    pub exhausted: bool,
    pub blocked_reason: Option<String>,
    pub needs_human: bool,
    pub stopped_after: Option<String>,
}

pub struct DriveForgeStoryOptions<'a> {
    pub work_type: &'a str,
    pub evidence: ForgeGateEvidence,
    pub runner: Option<&'a dyn ForgeRoleRunner>,
    pub allow_synthetic_runner: bool,
    pub max_steps: usize,
    pub worker_id: &'a str,
    pub split_concurrency: usize,
    pub stop_after: Option<ForgeStopTarget>,
}

impl<'a> DriveForgeStoryOptions<'a> {
    pub fn production(work_type: &'a str, runner: &'a dyn ForgeRoleRunner) -> Self {
        Self {
            work_type,
            evidence: ForgeGateEvidence {
                work_type: Some(work_type.into()),
                ..Default::default()
            },
            runner: Some(runner),
            allow_synthetic_runner: false,
            max_steps: 40,
            worker_id: "forge",
            split_concurrency: 1,
            stop_after: None,
        }
    }
}

pub fn drive_forge_story<S: TxStore>(
    rt: &ForgeRuntime<S>,
    story_id: &str,
    opts: DriveForgeStoryOptions<'_>,
) -> Result<DriveForgeStoryResult> {
    if opts.runner.is_none() && !opts.allow_synthetic_runner {
        return Err(WorkflowError::generic(
            "Forge production execution requires an explicit real role runner; the synthetic runner is test-only.",
        ));
    }
    let synthetic = DefaultForgeRoleRunner;
    let runner: &dyn ForgeRoleRunner = opts.runner.unwrap_or(&synthetic);
    let stop_target = resolve_forge_stop_target(opts.stop_after.as_ref());
    let mut stopped_after = None;
    let mut steps = Vec::new();

    let wake = rt.wake_story(story_id, opts.work_type, opts.evidence.clone())?;
    let instance_id = wake.instance_id;
    let _ = rt.reconcile_completions(story_id)?;

    for _ in 0..opts.max_steps {
        let tasks = rt.list_role_tasks(story_id)?;
        if tasks.is_empty() {
            break;
        }
        if let Some(human) = tasks.iter().find(|t| {
            t.node_id
                .as_deref()
                .map(|n| FORGE_HUMAN_GATE_NODES.contains(&n))
                .unwrap_or(false)
        }) {
            let _ = rt.writer().mark_story_human_hold(
                story_id,
                "Forge engine entered a human decision gate.",
            );
            let _ = human;
            return Ok(DriveForgeStoryResult {
                instance_id: instance_id.clone(),
                status: instance_status(rt, &instance_id)?,
                steps,
                exhausted: false,
                blocked_reason: None,
                needs_human: true,
                stopped_after,
            });
        }
        if !tasks.iter().any(|t| t.status == TaskStatus::Ready) {
            let blocked = tasks
                .iter()
                .map(|t| {
                    format!(
                        "{}={:?}",
                        t.node_id.as_deref().unwrap_or("?"),
                        t.status
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(DriveForgeStoryResult {
                instance_id: instance_id.clone(),
                status: instance_status(rt, &instance_id)?,
                steps,
                exhausted: true,
                blocked_reason: Some(format!("no ready task; active: {blocked}")),
                needs_human: false,
                stopped_after,
            });
        }

        let ready: Vec<_> = tasks
            .into_iter()
            .filter(|t| t.status == TaskStatus::Ready)
            .collect();
        let cap = opts.split_concurrency.max(1);
        let lanes: Vec<WaveLane<ActiveForgeRoleTask>> = ready
            .into_iter()
            .map(|task| {
                let node = task.node_id.clone().unwrap_or_default();
                WaveLane {
                    fanout: node == "smith_split_work",
                    surface: if node == "smith_split_work" {
                        None
                    } else {
                        None
                    },
                    lane: node,
                    task,
                }
            })
            .collect();
        let plan = plan_wave(&lanes, cap);
        for batch in plan.batches {
            for lane in batch {
                let task = lane.task;
                let node = task.node_id.clone().unwrap_or_default();
                let actor = task
                    .candidates
                    .first()
                    .cloned()
                    .unwrap_or_else(|| opts.worker_id.to_string());
                if let Err(err) = rt.claim_role_task(&task.task_id, &actor) {
                    if is_advance_conflict(&err) {
                        continue;
                    }
                    return Err(err);
                }
                let outcome = match runner.run(&node, &task) {
                    Ok(o) => o,
                    Err(err) => {
                        let settle = settle_forge_lane_failure(rt, &task.task_id, &actor, &err)?;
                        if settle == LaneFailureSettlement::AlreadyCompleted {
                            continue;
                        }
                        return Err(err);
                    }
                };
                if let Err(err) = rt.complete_role_task(
                    &task.task_id,
                    &actor,
                    outcome.transition_name.as_deref(),
                    outcome.evidence,
                ) {
                    if is_advance_conflict(&err) {
                        continue;
                    }
                    let settle = settle_forge_lane_failure(rt, &task.task_id, &actor, &err)?;
                    if settle == LaneFailureSettlement::AlreadyCompleted {
                        continue;
                    }
                    return Err(err);
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
    Ok(DriveForgeStoryResult {
        instance_id: instance_id.clone(),
        status: instance_status(rt, &instance_id)?,
        steps,
        exhausted: !tasks.is_empty(),
        blocked_reason: None,
        needs_human: tasks.iter().any(|t| {
            t.node_id
                .as_deref()
                .map(|n| FORGE_HUMAN_GATE_NODES.contains(&n))
                .unwrap_or(false)
        }),
        stopped_after,
    })
}

fn instance_status<S: TxStore>(rt: &ForgeRuntime<S>, instance_id: &str) -> Result<String> {
    Ok(format!(
        "{:?}",
        rt.engine().get_process_instance(instance_id)?.status
    ))
}
