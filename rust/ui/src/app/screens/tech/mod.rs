//! TECH — Forge Live Operations Cockpit (`/portal/tech`).
//!
//! This pass intentionally runs as a presentation-only MVI prototype while the OpenCode V2
//! harness contract is being completed. The model below is deterministic fake operational
//! state: no API, Neon, OpenCode, timer, or Forge-engine command is touched from this screen.
//!
//! The future live implementation should replace only the model source. The view contract is
//! deliberately shaped around Forge-owned facts: workflow roles, execution events, budgets,
//! candidate state, and engine results.

mod view;

use yew::prelude::*;

use crate::app::cmd::Cmd;
use crate::app::screen::{Link, Screen, ScreenCtx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Complete,
    Running,
    Waiting,
    Hold,
}

impl AgentState {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Complete => "✓",
            Self::Running => "●",
            Self::Waiting => "○",
            Self::Hold => "!",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Complete => "Complete",
            Self::Running => "Running",
            Self::Waiting => "Waiting",
            Self::Hold => "Hold",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentNode {
    pub id: &'static str,
    pub label: &'static str,
    pub state: AgentState,
    pub detail: &'static str,
    pub model: &'static str,
    pub session: &'static str,
    pub steps_used: u32,
    pub step_cap: u32,
    pub tokens: u64,
    pub cost_usd: f64,
    pub children: Vec<AgentNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Read,
    Search,
    Edit,
    Test,
    Subagent,
    Review,
    Decision,
}

impl EventKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Read => "READ",
            Self::Search => "SEARCH",
            Self::Edit => "EDIT",
            Self::Test => "TEST",
            Self::Subagent => "SUBAGENT",
            Self::Review => "REVIEW",
            Self::Decision => "DECISION",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityEvent {
    pub at: &'static str,
    pub kind: EventKind,
    pub title: &'static str,
    pub detail: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultState {
    ReadyQa,
    QaPassed,
    Hold,
    Failed,
    ReadyPublish,
}

impl ResultState {
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadyQa => "Ready for QA",
            Self::QaPassed => "QA passed",
            Self::Hold => "Hold",
            Self::Failed => "Failed",
            Self::ReadyPublish => "Ready to publish",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineResult {
    pub id: &'static str,
    pub title: &'static str,
    pub state: ResultState,
    pub detail: &'static str,
    pub candidate: Option<&'static str>,
    pub tests: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub status: &'static str,
    pub story_id: &'static str,
    pub story_title: &'static str,
    pub run_id: &'static str,
    pub work_item: &'static str,
    pub role: &'static str,
    pub model_name: &'static str,
    pub runtime: &'static str,
    pub worktree: &'static str,
    pub base_sha: &'static str,
    pub candidate_sha: &'static str,
    pub story_spend_usd: f64,
    pub story_budget_usd: f64,
    pub turns_used: u32,
    pub turn_cap: u32,
    pub agents: Vec<AgentNode>,
    pub selected_agent: String,
    pub activity: Vec<ActivityEvent>,
    pub results: Vec<EngineResult>,
    pub selected_result: String,
    pub notice: Option<String>,
}

impl Default for Model {
    fn default() -> Self {
        demo_model()
    }
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    AgentSelected(String),
    ResultSelected(String),
    DemoAction(&'static str),
    ClearNotice,
}

pub struct TechCockpit;

impl Screen for TechCockpit {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (demo_model(), Cmd::none())
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::AgentSelected(id) => model.selected_agent = id,
            Msg::ResultSelected(id) => model.selected_result = id,
            Msg::DemoAction(action) => {
                model.notice = Some(format!(
                    "{action} is preview-only in the MVI prototype. Live control will be wired through the Forge service after the V2 runtime contract lands."
                ));
            }
            Msg::ClearNotice => model.notice = None,
        }
        Cmd::none()
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        view::cockpit(model, &on_msg)
    }
}

pub fn find_agent<'a>(agents: &'a [AgentNode], id: &str) -> Option<&'a AgentNode> {
    for agent in agents {
        if agent.id == id {
            return Some(agent);
        }
        if let Some(found) = find_agent(&agent.children, id) {
            return Some(found);
        }
    }
    None
}

fn demo_model() -> Model {
    Model {
        status: "RUNNING",
        story_id: "ENG-FORGE-V2-042",
        story_title: "Forge V2 hardened execution profiles",
        run_id: "run_01JQ9Y7V2",
        work_item: "work_004218",
        role: "SMITH",
        model_name: "deepseek/deepseek-flash",
        runtime: "08:42",
        worktree: "/tmp/forge/ENG-FORGE-V2-042/smith",
        base_sha: "76f1c923",
        candidate_sha: "a83c12f4",
        story_spend_usd: 0.084,
        story_budget_usd: 0.250,
        turns_used: 5,
        turn_cap: 10,
        agents: vec![
            AgentNode {
                id: "scout",
                label: "Scout",
                state: AgentState::Complete,
                detail: "Mapped the existing harness boundary and V2 seams.",
                model: "deepseek-flash",
                session: "ses_scout_01",
                steps_used: 2,
                step_cap: 6,
                tokens: 8_240,
                cost_usd: 0.009,
                children: vec![],
            },
            AgentNode {
                id: "architect",
                label: "Architect",
                state: AgentState::Complete,
                detail: "Defined the role-permission and session-continuity contract.",
                model: "deepseek-flash",
                session: "ses_arch_01",
                steps_used: 3,
                step_cap: 8,
                tokens: 12_118,
                cost_usd: 0.014,
                children: vec![],
            },
            AgentNode {
                id: "lead",
                label: "Lead",
                state: AgentState::Complete,
                detail: "Decision: SMITH. One implementation lane, QA required.",
                model: "deepseek-flash",
                session: "ses_lead_01",
                steps_used: 1,
                step_cap: 4,
                tokens: 4_670,
                cost_usd: 0.006,
                children: vec![],
            },
            AgentNode {
                id: "smith",
                label: "Smith",
                state: AgentState::Running,
                detail: "Implementing V2 execution policy and integration coverage.",
                model: "deepseek-flash",
                session: "ses_smith_04",
                steps_used: 5,
                step_cap: 10,
                tokens: 31_482,
                cost_usd: 0.041,
                children: vec![
                    AgentNode {
                        id: "explore",
                        label: "Explore",
                        state: AgentState::Complete,
                        detail: "Traced RoleHarness callers and permission-sensitive commands.",
                        model: "deepseek-flash",
                        session: "ses_child_explore",
                        steps_used: 2,
                        step_cap: 4,
                        tokens: 5_180,
                        cost_usd: 0.006,
                        children: vec![],
                    },
                    AgentNode {
                        id: "reviewer",
                        label: "Reviewer",
                        state: AgentState::Running,
                        detail: "Reviewing the Smith diff for boundary violations.",
                        model: "deepseek-flash",
                        session: "ses_child_review",
                        steps_used: 2,
                        step_cap: 4,
                        tokens: 4_311,
                        cost_usd: 0.005,
                        children: vec![],
                    },
                    AgentNode {
                        id: "test-analyst",
                        label: "Test Analyst",
                        state: AgentState::Complete,
                        detail: "Mapped regression coverage and negative permission tests.",
                        model: "deepseek-flash",
                        session: "ses_child_test",
                        steps_used: 2,
                        step_cap: 4,
                        tokens: 3_764,
                        cost_usd: 0.003,
                        children: vec![],
                    },
                ],
            },
            AgentNode {
                id: "qa",
                label: "QA / Assay",
                state: AgentState::Waiting,
                detail: "Waiting for Smith candidate.",
                model: "deepseek-flash",
                session: "—",
                steps_used: 0,
                step_cap: 6,
                tokens: 0,
                cost_usd: 0.0,
                children: vec![],
            },
            AgentNode {
                id: "devops",
                label: "DEV_OPS",
                state: AgentState::Waiting,
                detail: "Publication gate has not been reached.",
                model: "Forge deterministic",
                session: "—",
                steps_used: 0,
                step_cap: 0,
                tokens: 0,
                cost_usd: 0.0,
                children: vec![],
            },
        ],
        selected_agent: "smith".into(),
        activity: vec![
            ActivityEvent {
                at: "16:22:04",
                kind: EventKind::Read,
                title: "rust/forge/src/engine/runner.rs",
                detail: "Loaded RoleHarness and runtime budget boundaries.",
            },
            ActivityEvent {
                at: "16:22:07",
                kind: EventKind::Search,
                title: "sanitize_model_env",
                detail: "Found production credential and Git-push protections.",
            },
            ActivityEvent {
                at: "16:22:12",
                kind: EventKind::Edit,
                title: "rust/forge/src/engine/opencode.rs",
                detail: "Added V2 execution policy mapping.",
            },
            ActivityEvent {
                at: "16:22:28",
                kind: EventKind::Test,
                title: "cargo test -p forge",
                detail: "148 passed · 0 failed",
            },
            ActivityEvent {
                at: "16:22:41",
                kind: EventKind::Subagent,
                title: "forge-reviewer",
                detail: "Read-only child session started.",
            },
            ActivityEvent {
                at: "16:23:10",
                kind: EventKind::Review,
                title: "Boundary review",
                detail: "No publication authority leaked into Smith.",
            },
            ActivityEvent {
                at: "16:23:18",
                kind: EventKind::Decision,
                title: "Candidate pending",
                detail: "QA remains gated until Smith produces a clean commit.",
            },
        ],
        results: vec![
            EngineResult {
                id: "ENG-FORGE-V2-041",
                title: "OpenCode V2 structured session adapter",
                state: ResultState::QaPassed,
                detail: "Structured events and explicit session continuity verified.",
                candidate: Some("44bc892e"),
                tests: "142 / 142",
            },
            EngineResult {
                id: "ENG-FORGE-V2-040",
                title: "Remove V1 SQLite telemetry coupling",
                state: ResultState::ReadyPublish,
                detail: "Usage now comes from the supported V2 execution contract.",
                candidate: Some("c921adc7"),
                tests: "139 / 139",
            },
            EngineResult {
                id: "ENG-VAULT-091",
                title: "Guest document authorization regression",
                state: ResultState::Hold,
                detail: "ARCHITECTURE_GAP · requires Vault entitlement decision.",
                candidate: None,
                tests: "—",
            },
            EngineResult {
                id: "ENG-OPS-034",
                title: "Property media reconciliation",
                state: ResultState::Failed,
                detail: "MODEL_TURN_CAP · stopped at 10 / 10 turns.",
                candidate: Some("91d61b0a"),
                tests: "27 / 31",
            },
            EngineResult {
                id: "ENG-SIGN-042",
                title: "Signature envelope idempotency",
                state: ResultState::ReadyQa,
                detail: "Candidate committed; deterministic assay requested.",
                candidate: Some("a83c12f4"),
                tests: "37 / 37",
            },
        ],
        selected_result: "ENG-SIGN-042".into(),
        notice: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cockpit_boots_from_fake_mvi_without_requests() {
        let ctx = ScreenCtx::default();
        let (model, cmd) = TechCockpit::init(&ctx);
        assert!(cmd.into_requests().is_empty());
        assert_eq!(model.status, "RUNNING");
        assert_eq!(model.selected_agent, "smith");
        assert!(find_agent(&model.agents, "reviewer").is_some());
    }

    #[test]
    fn selections_and_demo_controls_are_local_mvi_only() {
        let ctx = ScreenCtx::default();
        let (mut model, _) = TechCockpit::init(&ctx);

        assert!(TechCockpit::update(
            &mut model,
            Msg::AgentSelected("reviewer".into()),
            &ctx
        )
        .into_requests()
        .is_empty());
        assert_eq!(model.selected_agent, "reviewer");

        TechCockpit::update(&mut model, Msg::DemoAction("HOLD"), &ctx);
        assert!(model.notice.as_deref().is_some_and(|n| n.contains("preview-only")));
    }
}
