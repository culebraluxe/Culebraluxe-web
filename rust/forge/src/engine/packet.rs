//! Canonical OpenCode task text. Same contract as `buildTaskText` in
//! `agent-runtime/deepseek/deepseek-harness-adapter.ts` (OpenCode reuses it).

#[derive(Debug, Clone, Default)]
pub struct StoryPacket {
    pub id: String,
    pub title: String,
    pub goal: Option<String>,
    pub special_instructions: Option<String>,
    pub architect_brief: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub assay_commands: Vec<String>,
    pub branch_name: Option<String>,
    pub base_ref: Option<String>,
    pub base_commit: Option<String>,
}

impl StoryPacket {
    pub fn load_from_neon(story_id: &str) -> Result<Self, String> {
        use crate::engine::vendor_session::with_shared;
        use db::ForgeEngineDao;

        let row = with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.story_packet(story_id)
                    .await
                    .map_err(|error| error.to_string())
            })
        })??;

        let Some(row) = row else {
            return Err(format!("storyboard_story {story_id} not found"));
        };

        Ok(Self {
            id: row.id,
            title: row.title,
            goal: row.goal.filter(|value| !value.trim().is_empty()),
            special_instructions: None,
            architect_brief: row.architect_brief.filter(|value| !value.trim().is_empty()),
            acceptance_criteria: row.acceptance_criteria.filter(|value| !value.trim().is_empty()),
            assay_commands: row
                .assay_commands
                .map(|value| {
                    value
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            branch_name: None,
            base_ref: None,
            base_commit: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionWorkspace {
    pub worktree_path: String,
    pub branch_name: String,
    pub base_ref: String,
    pub base_commit: String,
}

pub fn build_task_text(
    node_id: &str,
    task_id: &str,
    packet: &StoryPacket,
    workspace: Option<&ExecutionWorkspace>,
) -> String {
    build_task_text_with_context(node_id, task_id, packet, workspace, None)
}

pub fn build_task_text_with_context(
    node_id: &str,
    task_id: &str,
    packet: &StoryPacket,
    workspace: Option<&ExecutionWorkspace>,
    decision_block: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!(
        "Execute SDLC story {}: {}.",
        if packet.id.is_empty() {
            task_id
        } else {
            &packet.id
        },
        if packet.title.is_empty() {
            node_id
        } else {
            &packet.title
        }
    ));
    parts.push(format!("Engine node {node_id} task {task_id}."));
    if let Some(g) = packet.goal.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("Goal: {g}"));
    }
    if let Some(i) = packet
        .special_instructions
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        parts.push(format!(
            "Special instructions (additive, do not replace the architect brief): {i}"
        ));
    }
    if let Some(b) = packet.architect_brief.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("Architect brief: {b}"));
    }
    if let Some(a) = packet
        .acceptance_criteria
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        parts.push(format!(
            "Acceptance criteria (do not mark Complete unless these are satisfied): {a}"
        ));
    }
    if let Some(block) = decision_block.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(block.to_string());
    }
    if let Some(w) = workspace {
        parts.push(format!(
            "Execution isolation (ENG-21): you are working in an isolated Git worktree on branch {}, created from approved base {}@{}. Commit your changes on this branch only; never push, merge, rebase, or touch files outside this checkout.",
            w.branch_name, w.base_ref, w.base_commit
        ));
    }
    parts.push(
        "Work in the current repository. Verify your work by running tests/typecheck/build within the runtime policy above. Create a local git commit with the intended changes when the story requires it. Do NOT push. Do NOT mutate production data or schema. Report what you did."
            .into(),
    );
    parts.push(
        "End your final report with one concise \"Tests: <summary>\" line so the harness can record a concrete tests/checks summary against this story."
            .into(),
    );
    parts.push(
        "End with the required structured evidence for this node (FORGE_EVIDENCE_JSON / FORGE_ARCHITECT_HANDOFF as the node demands)."
            .into(),
    );
    parts.join("\n")
}

pub const TESTS_SUMMARY_MARKER: &str = "Tests:";
pub const TESTS_SUMMARY_MAX_LENGTH: usize = 300;

pub fn extract_tests_summary(output: &str, fallback: &str) -> String {
    let mut summary: Option<String> = None;
    for line in output.lines() {
        if let Some(rest) = line.split(TESTS_SUMMARY_MARKER).nth(1) {
            let v = rest.trim();
            if !v.is_empty() {
                summary = Some(v.to_string());
            }
        }
    }
    match summary {
        None => fallback.to_string(),
        Some(s) if s.len() <= TESTS_SUMMARY_MAX_LENGTH => s,
        Some(s) => format!("{}…", &s[..TESTS_SUMMARY_MAX_LENGTH - 1]),
    }
}
