//! ENG-FORGE-V2-CAPABILITY-01 — Forge role → OpenCode V2 execution profile.
//!
//! Forge owns the macro orchestration; OpenCode V2 is the execution substrate. This module is the ONE place that
//! names a V2 execution agent, its authority, its child allowlist, its step ceiling and the skills it may load.
//! Every profile is **derived from the node's lane as the workflow definition binds it**
//! (`service_binding::lane_for_node`), so there is no competing role table: an unknown Forge role is a hard error,
//! never a silent default (FAIL CLOSED).
//!
//! The rendered JSON reaches the vendor two ways, both rendered from this file — there is no second
//! hand-maintained copy:
//!   1. `OPENCODE_CONFIG_CONTENT` on the model subprocess. Measured on the live 2.x build (2026-10-01): the
//!      environment content is honoured by `opencode run --standalone` — the private `serve --stdio` that
//!      `--standalone` spawns inherits the subprocess environment — and it applies from ANY cwd. That matters
//!      because Forge executes inside a linked git worktree, where a repo-root `opencode.json` is NOT discovered
//!      (`debug config` from a worktree lists only the global config): a project-file-only design would have
//!      silently enforced nothing. The same measurement names two traps. `debug config` and `debug agents` talk to
//!      the operator's BACKGROUND SERVICE, which never sees this variable (in a directory with no project file
//!      and the payload exported, `debug agents` lists no `forge-*` agent at all), so they must not be used to
//!      decide whether delivery works. And `--standalone` must never be dropped from the run args: it is the
//!      private server that makes delivery possible, not a performance preference. The probe that does settle it,
//!      free of tokens because `--agent` resolves before any model call, is
//!      `v2_the_env_payload_is_what_delivers_the_agents_to_a_standalone_turn`.
//!   2. a checked-in repo-root `opencode.json`, operator-visible and drift-checked by `pnpm forge:harness`.
//!
//! Authority is execution enforcement, not documentation. Because the harness runs `--auto`, an `ask` verdict
//! becomes an allow, so **only an explicit `deny` is security** (story §3). Measured: with
//! `permission.edit = {"*":"allow","blocked.txt":"deny"}` the specific deny held and the write was refused,
//! which is what makes "broad allow + specific deny" a control rather than a comment. `ask` is never used here
//! to mean "stop".
//!
//! WHY THE PERMISSION BLOCK IS AN ALLOWLIST (`"*": "deny"` FIRST), NOT A SET OF DENIES (2026-10-01).
//!
//! The vendor documents three facts that only matter together, and the earlier rendering got one of them wrong:
//!   1. "OpenCode permissions are keyed by tool name" — the built-ins (`read`, `edit`, `bash`, `task`, …) AND
//!      every MCP tool, because "MCP server tools are registered with server name as prefix".
//!   2. "Rules are evaluated by pattern match, with the last matching rule winning", and the recommended shape is
//!      the catch-all first with the specific rules after it.
//!   3. Defaults are PERMISSIVE: "Most permissions default to `allow`".
//!
//! A deny-list therefore only covers the tool names it happens to mention. Everything else — every tool an MCP
//! server contributes, and every tool a future vendor build adds — inherits `allow`. That is not a theoretical
//! hole: this repository's own operator config contributes `mcp.serena` (a filesystem-capable stdio server), and
//! `mcp` config MERGES with `OPENCODE_CONFIG_CONTENT`, so a Forge turn ran with a second tool surface on which
//! `serena_*` writes were reachable while `edit` was denied. A denied `edit` that is still reachable through
//! another tool name is not a deny.
//!
//! So this block renders the catch-all `deny` FIRST — it also sorts first in the emitted JSON, since
//! `serde_json`'s object is a `BTreeMap` and `*` precedes every tool name — and then names, one by one, the tools
//! a profile may actually use. Every unnamed tool — every MCP tool, `doom_loop`, anything a later build adds —
//! is denied by construction rather than by remembering to list it. `READ_TOOLS` is granted to every profile
//! because every Forge role must be able to look at its own worktree; a role that cannot read cannot work.
//!
//! The allowlist changes "the model was told" into "the tool was absent", which is the only form of authority a
//! prompt cannot talk its way out of.
//!
//! SKILLS: ONE AUTHORED COPY, ONE GENERATED MIRROR (2026-10-02). A `skill` grant is a CAPABILITY only when the
//! vendor can load the named skill, and the live build settles that this is a filesystem question rather than a
//! permissions question (measured: `--agent forge-assay` + `skill rust-testing` returned the skill's own body,
//! while `--agent forge-scout` + `skill planner` returned `Unable to load skill planner`). The canonical prose
//! lives in `docs/agent/skills/<id>.md` — that is where AGENTS.md says skills live, and it is the copy a packet's
//! `## Skills` list names — and `render_skill_markdown`/`render_skill_tree` generate the vendor tree
//! `.opencode/skills/<id>/SKILL.md` from it, with `forge opencode-skills --check` (in `pnpm forge:harness`)
//! refusing drift. So there is exactly one hand-written copy of any skill body and exactly one generator for the
//! vendor-facing frontmatter: the state this module is named after.
//!
//! SUBAGENTS ARE OFF BY DEFAULT AND ARMED EXPLICITLY (2026-10-02). §9's micro-subagents, their child allowlists
//! and `subagent_depth = 1` remain the shipped design and are rendered unchanged — but only when the operator
//! arms them with `FORGE_SUBAGENTS=1`. The reason is measured, not stylistic: a child session's spend is NOT in
//! its parent's export (`harness_usage.rs`, one real parent/child pair from the live 2.0.21 store), and the
//! vendor's HTTP session APIs are unreachable from a `--standalone` turn, so a child's cost is invisible to every
//! Forge-side reading. With the default OFF, `task` denies every name for every agent, no profile holds the
//! subagent tool, and `subagent_depth` is 0 — so the unmeasurable spend cannot occur at all. When it is armed,
//! the turn SAYS SO (`render_subagent_declaration`) instead of leaving the undercount to be rediscovered.

use crate::engine::role_mapping::LaneId;
use crate::engine::service_binding::lane_for_node;
use crate::roles::lead;
use serde_json::{json, Map, Value};

/// The V2 execution agents Forge may select. Forge chooses the agent for a node; the model never does.
pub const AGENT_SCOUT: &str = "forge-scout";
pub const AGENT_ARCHITECT: &str = "forge-architect";
pub const AGENT_LEAD: &str = "forge-lead";
pub const AGENT_LEAD_IMPLEMENT: &str = "forge-lead-implement";
pub const AGENT_SMITH: &str = "forge-smith";
pub const AGENT_INSPECTOR: &str = "forge-inspector";
pub const AGENT_ASSAY: &str = "forge-assay";
pub const AGENT_DEVOPS: &str = "forge-devops";

/// The only micro-subagents Forge permits (§8). Small, read-only, and unable to spawn children.
pub const AGENT_EXPLORE: &str = "forge-explore";
pub const AGENT_REVIEWER: &str = "forge-reviewer";
pub const AGENT_TEST_ANALYST: &str = "forge-test-analyst";

// -----------------------------------------------------------------------------------------------------------
// Forge-side runtime limits (§10, §17). Constants with tests, never prompt prose.
// -----------------------------------------------------------------------------------------------------------

/// Max V2 child sessions a single role turn may create.
pub const MAX_CHILD_SESSIONS_PER_TURN: u32 = 3;
/// Max V2 child depth: 1 means a child may not spawn a grandchild.
pub const MAX_CHILD_DEPTH: u32 = 1;
/// Step ceiling for a micro-subagent. Deliberately small: a child is a question, not a project.
pub const CHILD_MAX_STEPS: u32 = 8;
/// Step ceiling for a primary role turn.
pub const ROLE_MAX_STEPS: u32 = 60;
/// Background children require every child to be joined before the role turn completes, which the current vendor
/// event stream cannot prove, so §10's fallback applies: background mode is DISABLED and Forge uses foreground
/// children. Correctness before concurrency.
pub const BACKGROUND_CHILDREN_ENABLED: bool = false;

/// The environment variable that ARMS subagents. Absent, empty, `0`, `no` and `false` all mean OFF: the switch is
/// deliberately a switch, so "I did not think about it" and "I turned it off" render the same config.
pub const SUBAGENTS_ENV: &str = "FORGE_SUBAGENTS";

/// Subagents are OFF unless `FORGE_SUBAGENTS` arms them. See the module note: with them off, `task` denies every
/// name, no profile holds the subagent tool, and `subagent_depth` is 0, so the child spend Forge cannot measure
/// (a child's cost is absent from its parent's export) cannot be created in the first place.
pub const SUBAGENTS_DEFAULT: bool = false;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Primary,
    Subagent,
}

/// What a profile may do to the workspace. Publication is NOT a variant: no V2 execution agent ever publishes,
/// so there is no value that could express it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    /// May read, search and inspect. May not modify the workspace.
    ReadOnly,
    /// May modify the workspace (still cannot publish).
    Implement,
}

pub struct V2AgentProfile {
    pub id: &'static str,
    pub mode: AgentMode,
    pub authority: Authority,
    pub description: &'static str,
    pub steps: u32,
    /// Skill ids this agent may load. A skill gives knowledge; it never grants authority (§7).
    pub skills: &'static [&'static str],
    /// Child agents this agent may spawn. Empty means no subagents at all.
    pub children: &'static [&'static str],
    /// Read-only shell command prefixes granted to an otherwise-shell-less agent (e.g. Assay running tests).
    /// Empty means `bash` is denied outright.
    pub shell_allow: &'static [&'static str],
}

// -----------------------------------------------------------------------------------------------------------
// Skill catalogue
// -----------------------------------------------------------------------------------------------------------

/// Every skill id Forge is willing to expose to V2. Each id must satisfy the V2 name rule
/// `^[a-z0-9]+(-[a-z0-9]+)*$` and match its skill directory name, so a typo here is a test failure rather than a
/// skill that silently never loads.
///
/// The first group is the canonical Forge skill library under `docs/agent/skills/` and is NOT re-authored here.
/// The second group is the architecture skills added by this story (§6). Both groups are also the set the vendor
/// tree is GENERATED from, so this list and `docs/agent/skills/` are the only two places a skill can be added.
pub const CANONICAL_FORGE_SKILLS: [&str; 11] = [
    "cruiser", "forms", "knip", "neon", "planner", "ripwire", "rtk", "semgrep", "serena", "ui",
    "workflow",
];

/// Architecture skills added by this story (§6). Operational knowledge, not a copy of the handbook.
pub const ADDED_ARCHITECTURE_SKILLS: [&str; 6] = [
    "abstract-service",
    "database-migration",
    "rust-testing",
    "rust-yew-mvi",
    "security-entitlements",
    "vault-service",
];

/// The `description` every skill is published with, in the voice the vendor's own skills use ("Use when …").
///
/// It is authored here rather than scraped from the prose because it is the hint the MODEL selects on — a
/// rendering decision — and because the vendor requires it in the frontmatter of every loadable skill. One entry
/// per id in `all_skill_ids()`, asserted in BOTH directions: a catalogued skill with no description, and a
/// description for a skill that no longer exists, are both failures rather than a published empty hint.
const SKILL_DESCRIPTIONS: [(&str, &str); 17] = [
    (
        "cruiser",
        "Use when enforcing architecture boundaries, dependency rules or cycles - the dependency-cruiser hard gate",
    ),
    (
        "forms",
        "Use when changing listing agreements, signed documents or the form pipeline that renders them to PDF",
    ),
    (
        "knip",
        "Use when removing dead files, exports or dependencies, or before declaring a refactor complete",
    ),
    (
        "neon",
        "Use when touching Postgres on Neon - migrations, repository modules, branches and the canonical data rules",
    ),
    (
        "planner",
        "Use when you must write the next packet rather than implement it - the V3 outer loop's inputs, shape and exit",
    ),
    (
        "ripwire",
        "Use when mapping an unfamiliar codebase or ranking symbols before reading files - the deterministic repo map",
    ),
    (
        "rtk",
        "Use when a command's raw output would flood the context and should be filtered or summarized before it arrives",
    ),
    (
        "semgrep",
        "Use when you need static analysis grep cannot do - taint and dataflow, injection, hardcoded secrets",
    ),
    (
        "serena",
        "Use when symbol identity matters - references, implementations, definitions or a precise rename",
    ),
    (
        "ui",
        "Use when changing UI under web/ui - brand tokens, the editorial house style, iPad targets, data-derived screens",
    ),
    (
        "workflow",
        "Use when a story needs the Forge workflow vocabulary - states, lanes, turns, candidates and the packet shape",
    ),
    (
        "abstract-service",
        "Use when adding or changing a service operation in the Rust service layer - capabilities, envelopes, authorization, idempotency and execution policy",
    ),
    (
        "database-migration",
        "Use when adding, editing or applying a SQL migration - numbering, the two migration roots, and the apply, status and scan gates",
    ),
    (
        "rust-testing",
        "Use when adding, running or fixing Rust tests - nextest in CI, cargo test locally, the format gate and the deferred trees",
    ),
    (
        "rust-yew-mvi",
        "Use when working on a web/ui screen - the reducer shape, Cmd::request I/O, registry entitlement codes and the wasm feature",
    ),
    (
        "security-entitlements",
        "Use when adding or changing an authorization action, a role grant, or the entitlement a screen requires",
    ),
    (
        "vault-service",
        "Use when working on issued transaction documents - the vault domain, its database binding, the version lineage and the vault.read entitlement",
    ),
];

const SCOUT_SKILLS: [&str; 4] = ["planner", "ripwire", "semgrep", "serena"];
const ARCHITECT_SKILLS: [&str; 7] = [
    "abstract-service",
    "planner",
    "ripwire",
    "rust-yew-mvi",
    "security-entitlements",
    "ui",
    "workflow",
];
const LEAD_SKILLS: [&str; 2] = ["planner", "workflow"];
const IMPLEMENT_SKILLS: [&str; 8] = [
    "abstract-service",
    "forms",
    "neon",
    "planner",
    "rust-testing",
    "rust-yew-mvi",
    "ui",
    "workflow",
];
const SMITH_SKILLS: [&str; 8] = [
    "abstract-service",
    "forms",
    "neon",
    "rtk",
    "rust-testing",
    "semgrep",
    "ui",
    "workflow",
];
const INSPECTOR_SKILLS: [&str; 5] = [
    "abstract-service",
    "ripwire",
    "rust-testing",
    "semgrep",
    "ui",
];
const ASSAY_SKILLS: [&str; 3] = ["rtk", "rust-testing", "semgrep"];
const DEVOPS_SKILLS: [&str; 4] = [
    "database-migration",
    "neon",
    "security-entitlements",
    "vault-service",
];
const EXPLORE_SKILLS: [&str; 3] = ["ripwire", "semgrep", "serena"];
const REVIEWER_SKILLS: [&str; 3] = ["abstract-service", "rust-testing", "semgrep"];
const TEST_ANALYST_SKILLS: [&str; 2] = ["rust-testing", "semgrep"];

/// Inspection commands a read-only role may still run. Deliberately tiny.
const INSPECT_SHELL: [&str; 6] = [
    "git status",
    "git diff",
    "git log",
    "git show",
    "rg ",
    "rg --files",
];

/// Test commands. Assay and Inspector may execute tests; neither may edit the application to make one pass.
const TEST_SHELL: [&str; 6] = [
    "cargo test",
    "cargo nextest",
    "cargo check",
    "pnpm test",
    "pnpm forge:",
    "git status",
];

/// The micro-subagent allowlists (§9). Child agents have empty `children`, so a child cannot spawn a grandchild.
const ARCHITECT_CHILDREN: [&str; 1] = [AGENT_EXPLORE];
const SMITH_CHILDREN: [&str; 3] = [AGENT_EXPLORE, AGENT_REVIEWER, AGENT_TEST_ANALYST];
const NO_CHILDREN: [&str; 0] = [];

// -----------------------------------------------------------------------------------------------------------
// The profile table
// -----------------------------------------------------------------------------------------------------------

pub const V2_AGENT_PROFILES: [V2AgentProfile; 11] = [
    V2AgentProfile {
        id: AGENT_SCOUT,
        mode: AgentMode::Primary,
        authority: Authority::ReadOnly,
        description: "Forge research/diagnosis scout: read, search and inspect only.",
        steps: ROLE_MAX_STEPS,
        skills: &SCOUT_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &INSPECT_SHELL,
    },
    V2AgentProfile {
        id: AGENT_ARCHITECT,
        mode: AgentMode::Primary,
        authority: Authority::ReadOnly,
        description: "Forge architect: design, boundaries and planning. Designs; never quietly becomes Smith.",
        steps: ROLE_MAX_STEPS,
        skills: &ARCHITECT_SKILLS,
        children: &ARCHITECT_CHILDREN,
        shell_allow: &INSPECT_SHELL,
    },
    V2AgentProfile {
        id: AGENT_LEAD,
        mode: AgentMode::Primary,
        authority: Authority::ReadOnly,
        description: "Forge lead: routing, judgment, assembly. Read-only, no publication, no child agents.",
        steps: ROLE_MAX_STEPS,
        skills: &LEAD_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &INSPECT_SHELL,
    },
    V2AgentProfile {
        id: AGENT_LEAD_IMPLEMENT,
        mode: AgentMode::Primary,
        authority: Authority::Implement,
        description: "Forge solo-implement lead: the one lead phase that implements. Cannot publish.",
        steps: ROLE_MAX_STEPS,
        skills: &IMPLEMENT_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &[],
    },
    V2AgentProfile {
        id: AGENT_SMITH,
        mode: AgentMode::Primary,
        authority: Authority::Implement,
        description: "Forge smith: implements inside the worktree and commits a candidate. Never publishes.",
        steps: ROLE_MAX_STEPS,
        skills: &SMITH_SKILLS,
        children: &SMITH_CHILDREN,
        shell_allow: &[],
    },
    V2AgentProfile {
        id: AGENT_INSPECTOR,
        mode: AgentMode::Primary,
        authority: Authority::ReadOnly,
        description: "Forge QA review: inspects the candidate and runs tests. QA DOES NOT WRITE CODE.",
        steps: ROLE_MAX_STEPS,
        skills: &INSPECTOR_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &TEST_SHELL,
    },
    V2AgentProfile {
        id: AGENT_ASSAY,
        mode: AgentMode::Primary,
        authority: Authority::ReadOnly,
        description: "Forge assay: executes the authoritative tests and reports. Cannot repair the application.",
        steps: ROLE_MAX_STEPS,
        skills: &ASSAY_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &TEST_SHELL,
    },
    V2AgentProfile {
        id: AGENT_DEVOPS,
        mode: AgentMode::Primary,
        authority: Authority::ReadOnly,
        description: "Forge devops: analyses release state. Publication and deployment stay Forge-owned code.",
        steps: ROLE_MAX_STEPS,
        skills: &DEVOPS_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &INSPECT_SHELL,
    },
    V2AgentProfile {
        id: AGENT_EXPLORE,
        mode: AgentMode::Subagent,
        authority: Authority::ReadOnly,
        description: "Micro-subagent: codebase search, dependency tracing, call-site discovery. Read-only.",
        steps: CHILD_MAX_STEPS,
        skills: &EXPLORE_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &INSPECT_SHELL,
    },
    V2AgentProfile {
        id: AGENT_REVIEWER,
        mode: AgentMode::Subagent,
        authority: Authority::ReadOnly,
        description: "Micro-subagent: reviews a diff for defects, architecture violations and missing tests.",
        steps: CHILD_MAX_STEPS,
        skills: &REVIEWER_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &INSPECT_SHELL,
    },
    V2AgentProfile {
        id: AGENT_TEST_ANALYST,
        mode: AgentMode::Subagent,
        authority: Authority::ReadOnly,
        description: "Micro-subagent: finds relevant tests and missing regression coverage. Cannot edit code.",
        steps: CHILD_MAX_STEPS,
        skills: &TEST_ANALYST_SKILLS,
        children: &NO_CHILDREN,
        shell_allow: &TEST_SHELL,
    },
];

// -----------------------------------------------------------------------------------------------------------
// Role → agent resolution
// -----------------------------------------------------------------------------------------------------------

/// Resolve a Forge engine node id to its V2 execution agent.
///
/// FAIL CLOSED: an unknown node id is an error and there is no default agent. A node Forge cannot map is a node
/// Forge must not run, because "run it as Smith anyway" would hand implement authority to a role nobody granted.
pub fn v2_agent_for_node(node_id: &str) -> Result<&'static str, String> {
    Ok(match lane_for_node(node_id)? {
        LaneId::Scout => AGENT_SCOUT,
        LaneId::Architect => AGENT_ARCHITECT,
        // The lead has three phases. Only the solo-implement phase may write; pre/post are routing, judgment and
        // assembly, so they get the read-only lead rather than the implementing one.
        LaneId::Lead if lead::implements(node_id) => AGENT_LEAD_IMPLEMENT,
        LaneId::Lead => AGENT_LEAD,
        LaneId::Smith => AGENT_SMITH,
        LaneId::Assay => AGENT_ASSAY,
        LaneId::Inspector => AGENT_INSPECTOR,
        LaneId::DevOps => AGENT_DEVOPS,
    })
}

pub fn v2_profile(agent_id: &str) -> Option<&'static V2AgentProfile> {
    V2_AGENT_PROFILES
        .iter()
        .find(|profile| profile.id == agent_id)
}

/// Every skill id Forge exposes to V2, in a stable order: canonical library first, then the added architecture
/// skills. Drift checking and the generated skill tree both read this.
pub fn all_skill_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = Vec::new();
    for skill in CANONICAL_FORGE_SKILLS
        .iter()
        .chain(ADDED_ARCHITECTURE_SKILLS.iter())
    {
        if !ids.contains(skill) {
            ids.push(skill);
        }
    }
    ids
}

/// True when `name` satisfies the vendor's strict skill-name rule `^[a-z0-9]+(-[a-z0-9]+)*$`.
///
/// Hand-rolled rather than regex-based so the rule is explicit and dependency-free: lowercase alphanumerics in
/// hyphen-separated non-empty segments. This rejects the names the vendor would silently hide (`UI`, `ui_v2`,
/// `-ui`, `ui--v2`).
pub fn is_valid_v2_skill_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.split('-').all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    })
}

/// The selection hint a skill is published with. An id with no description is an ERROR rather than a skill
/// published without one: the description is what the model selects on, so a silent omission degrades the role
/// without failing anywhere.
pub fn skill_description(id: &str) -> Result<&'static str, String> {
    SKILL_DESCRIPTIONS
        .iter()
        .find(|(skill, _)| *skill == id)
        .map(|(_, description)| *description)
        .ok_or_else(|| {
            format!(
                "no description for skill '{id}': the catalogue and the description table disagree"
            )
        })
}

// -----------------------------------------------------------------------------------------------------------
// Skill tree rendering (§6, §7): the canonical prose is authored once, the vendor tree is generated from it
// -----------------------------------------------------------------------------------------------------------

/// Canonical skill prose, repo-relative. `AGENTS.md`: "Skills live in `docs/agent/skills/`" — so this is the one
/// place a skill body is written, whether it is read by a human packet or published to the vendor.
pub const CANONICAL_SKILL_DIR: &str = "docs/agent/skills";

/// The vendor-loadable skill tree, repo-relative: `.opencode/skills/<id>/SKILL.md`. OpenCode V2 discovers skills
/// here (and in the global config directory), which is why a `docs/`-only skill is a grant the vendor refuses.
pub const VENDOR_SKILL_DIR: &str = ".opencode/skills";

/// The vendor's filename for a skill body. Fixed by the vendor, not a Forge choice.
pub const VENDOR_SKILL_FILE: &str = "SKILL.md";

/// Where a skill's canonical prose lives.
pub fn canonical_skill_path(id: &str) -> String {
    format!("{CANONICAL_SKILL_DIR}/{id}.md")
}

/// Where a skill's vendor-loadable file lives. The id IS the directory name, so a mismatch between the two can
/// only be introduced by hand — and hand-maintaining this tree is exactly what the renderer replaces.
pub fn vendor_skill_path(id: &str) -> String {
    format!("{VENDOR_SKILL_DIR}/{id}/{VENDOR_SKILL_FILE}")
}

/// A loadable `SKILL.md`, rendered from a skill id and its canonical prose.
///
/// PURE: the canonical body is passed in, so the renderer has no opinion about where the prose came from and the
/// same input always produces the same bytes. Reading the prose is the caller's job — the same split as
/// `render_v2_agent_config()` and the CLI command that writes it.
///
/// Three properties are enforced here rather than trusted to the author, because each one is a way for a skill to
/// exist in the tree and still be unloadable:
///   * the id must satisfy the vendor's name rule — a directory the vendor cannot address holds a skill nobody
///     can load;
///   * the frontmatter block must be the FIRST thing in the file (`---\n` at byte 0). A frontmatter block lower
///     down is not frontmatter, and the vendor then reads the file without a name or a description;
///   * the prose must not carry frontmatter of its own, which would produce two blocks and an ambiguous name.
pub fn render_skill_markdown(id: &str, canonical_body: &str) -> Result<String, String> {
    if !is_valid_v2_skill_name(id) {
        return Err(format!(
            "'{id}' is not a valid V2 skill name; the vendor's rule is ^[a-z0-9]+(-[a-z0-9]+)*$"
        ));
    }
    let description = skill_description(id)?;
    let body = canonical_body.trim();
    if body.is_empty() {
        return Err(format!("the canonical prose for '{id}' is empty"));
    }
    if body.starts_with("---") {
        return Err(format!(
            "'{id}' carries its own frontmatter; the renderer owns it and a second block would be ambiguous"
        ));
    }
    Ok(format!(
        "---\nname: {id}\ndescription: {description}\n---\n\n{body}\n"
    ))
}

/// Every rendered skill file as `(repo-relative path, content)`, in catalogue order.
///
/// PURE given the canonical prose. A catalogued skill with no prose is an error rather than a skip: the grant
/// exists in the config, so a missing file would make it inert again, which is the whole failure being removed.
pub fn render_skill_tree(
    bodies: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<(String, String)>, String> {
    let mut files = Vec::new();
    for id in all_skill_ids() {
        let body = bodies.get(id).ok_or_else(|| {
            format!(
                "no canonical prose at {} for skill '{id}'",
                canonical_skill_path(id)
            )
        })?;
        files.push((vendor_skill_path(id), render_skill_markdown(id, body)?));
    }
    Ok(files)
}

/// The children a parent agent may spawn. An unknown parent gets the EMPTY allowlist, so a name Forge cannot
/// resolve cannot inherit spawning rights from a fallback.
pub fn v2_children_for_agent(agent_id: &str) -> &'static [&'static str] {
    match v2_profile(agent_id) {
        Some(profile) => profile.children,
        None => &NO_CHILDREN,
    }
}

/// True only when `parent` explicitly lists `child`. A child agent's `children` is empty, so this is also what
/// makes §10's "a child may not spawn a child" decidable Forge-side rather than merely configured.
pub fn v2_child_allowed(parent_agent: &str, child_agent: &str) -> bool {
    v2_children_for_agent(parent_agent).contains(&child_agent)
}

// -----------------------------------------------------------------------------------------------------------
// The subagent gate (§9, §10): declared capability, explicit arming, and a turn that says so
// -----------------------------------------------------------------------------------------------------------

/// Arm or disarm subagents from `FORGE_SUBAGENTS`.
///
/// OFF is the answer for `None`, `""` and for every value that is not one of `1`, `true`, `yes`, `on`
/// (case-insensitive). That includes a TYPO, and deliberately: the safe direction is the one to fail towards, and
/// the turn's declaration line prints the value that was read, so an ignored `FORGE_SUBAGENTS=ture` is visible in
/// the log instead of being mistaken for a considered deny.
pub fn parse_subagents_flag(raw: Option<&str>) -> bool {
    match raw {
        Some(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => SUBAGENTS_DEFAULT,
    }
}

/// The armed state read from the process environment — the only place the variable is read, so the harness, the
/// CLI and the tests cannot disagree about what is in force.
pub fn subagents_enabled_from_env() -> bool {
    parse_subagents_flag(std::env::var(SUBAGENTS_ENV).ok().as_deref())
}

/// The children an agent may spawn once the gate is applied.
///
/// With `spawn == false` this is the EMPTY allowlist for EVERY agent — including the two profiles that declare
/// children — so the gate cannot be side-stepped by a name, a fallback or an unknown agent id.
pub fn v2_children_for_agent_gated(agent_id: &str, spawn: bool) -> &'static [&'static str] {
    if !spawn {
        return &NO_CHILDREN;
    }
    v2_children_for_agent(agent_id)
}

/// True only when the gate is armed AND `parent` declares `child`.
pub fn v2_child_allowed_gated(parent_agent: &str, child_agent: &str, spawn: bool) -> bool {
    v2_children_for_agent_gated(parent_agent, spawn).contains(&child_agent)
}

/// One line, emitted once per role turn, that states in the log whether subagents were armed — and, when they
/// were, that the child spend is not fully measurable.
///
/// This is the honest half of the gate. The undercount is not hypothetical: the vendor's parent export carries
/// the parent's totals alone (measured, `harness_usage.rs`), so an armed turn's recorded cost is a lower bound.
/// A turn that can undercount should say so where the operator can see it, not only in a module comment.
pub fn render_subagent_declaration(spawn: bool, raw: Option<&str>) -> String {
    let shown = match raw {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        // `<unset>` and an empty value render the same config, so they are reported the same way.
        _ => "<unset>".to_string(),
    };
    if spawn {
        format!(
            "subagents=ARMED ({SUBAGENTS_ENV}={shown}): child sessions are possible, so this turn's recorded \
             spend is a LOWER BOUND — a child's cost is not in its parent's export"
        )
    } else {
        format!(
            "subagents=disabled ({SUBAGENTS_ENV}={shown}): every task name is denied and subagent_depth=0, so \
             no child session can be created and the recorded spend is the whole turn"
        )
    }
}

// -----------------------------------------------------------------------------------------------------------
// Permission rendering
// -----------------------------------------------------------------------------------------------------------

/// Tools EVERY Forge role may use, regardless of authority.
///
/// These are the read-only eyes: file content, name patterns, content search, directory listing, and semantic
/// queries. They are named here because the permission block is an allowlist — an unnamed tool is denied — and a
/// role that cannot inspect its own worktree cannot do its job. None of them can modify the workspace, so
/// granting them does not blur the authority line that `edit` and `bash` carry.
pub const READ_TOOLS: [&str; 5] = ["glob", "grep", "list", "lsp", "read"];

/// Commands NO Forge role may run, whatever its authority. Publication, deployment and production-data authority
/// are Forge-owned code paths, never a model's shell (§2, §20). These denies are re-applied on top of every
/// profile — including Smith, Lead-Implement and DevOps — so gaining workspace authority never gains publication
/// authority.
///
/// The vendor resolves a command pattern most-specifically-first and an explicit deny wins, which is what makes
/// "broad allow + specific deny" a control rather than a comment.
const PUBLICATION_DENY: [&str; 17] = [
    "git push",
    "git remote",
    "git fetch",
    "git send-email",
    "gh ",
    "pnpm deploy",
    "npm publish",
    "cargo publish",
    "psql",
    "neonctl ",
    "wrangler",
    "vercel",
    "flyctl",
    "kubectl",
    "terraform",
    "aws ",
    "gcloud ",
];

fn shell_rules(profile: &V2AgentProfile) -> Value {
    let mut rules = Map::new();
    for pattern in PUBLICATION_DENY {
        rules.insert(format!("{pattern}*"), json!("deny"));
    }
    match profile.authority {
        // An implementer gets the whole toolchain — build, test, `git add`/`git commit` inside its own worktree.
        // The publication denies above are still in force, so "May I use bash?" is not the same question as
        // "May I publish?".
        Authority::Implement => {
            rules.insert("*".into(), json!("allow"));
        }
        // A read-only role gets only the commands it declared, plus the publication denies. The trailing
        // catch-all deny means an undeclared command cannot inherit an allow.
        Authority::ReadOnly => {
            for pattern in profile.shell_allow {
                rules.insert(format!("{pattern}*"), json!("allow"));
            }
            rules.insert("*".into(), json!("deny"));
        }
    }
    Value::Object(rules)
}

/// Which micro-subagents this agent may spawn, once the gate is applied.
///
/// Everything undeclared is denied, so a child agent (whose `children` is empty) resolves to "deny everything"
/// and cannot spawn a grandchild. With the gate DISARMED the map is `{"*": "deny"}` for every profile, the two
/// declared child allowlists included — the config then contains no name that could be spawned.
fn task_rules(profile: &V2AgentProfile, spawn: bool) -> Value {
    let mut rules = Map::new();
    for child in v2_children_for_agent_gated(profile.id, spawn) {
        rules.insert((*child).to_string(), json!("allow"));
    }
    rules.insert("*".into(), json!("deny"));
    Value::Object(rules)
}

/// Which skills this agent may load. A `deny` HIDES the skill from the agent, so a role is not merely told not to
/// use a skill it cannot use. A skill gives knowledge; it never grants authority (§7) — `neon` being loadable
/// does not make production database credentials reachable.
fn skill_rules(profile: &V2AgentProfile) -> Value {
    let mut rules = Map::new();
    for skill in profile.skills {
        rules.insert((*skill).to_string(), json!("allow"));
    }
    rules.insert("*".into(), json!("deny"));
    Value::Object(rules)
}

/// Web access is a deliberate exception, not a default: only Scout and Architect may reach the network, because
/// only they are asked to reason about external facts. Everyone else is denied.
fn web_enabled(profile: &V2AgentProfile) -> bool {
    matches!(profile.id, AGENT_SCOUT | AGENT_ARCHITECT)
}

/// The permission block for one profile. This is execution enforcement, not documentation.
///
/// THE SHAPE IS AN ALLOWLIST. `"*": "deny"` is inserted first, so a tool name this function does not explicitly
/// grant is refused — including every tool an MCP server contributes (`serena_*` and friends) and every safety
/// guard the vendor owns. See the module note: the vendor keys permissions by tool name and lets the last
/// matching rule win, so the catch-all is only a control if the grants come after it, which is exactly what the
/// emitted JSON gives us (`*` sorts before every tool name in the `BTreeMap` behind `serde_json`'s object).
///
/// `edit` and `patch` are the workspace write tools, and for a read-only role they are an outright `deny`:
/// "the architect only designed" and "QA did not fix the code it judged" are properties of the sandbox rather
/// than of the prompt. An MCP tool that can write (for example a filesystem MCP server) is covered by the same
/// rule as `edit`: an MCP tool Forge does not grant must not be enabled while a role holds it, since a denied
/// `edit` would otherwise be reachable through a second tool surface (§13).
///
/// Two denies are deliberate and unattended-lane specific rather than authority-specific:
///   - `question` is refused because a Forge turn has no one to answer it. The subprocess runs with stdin
///     closed and `--auto`, so an agent that stops to ask would block until something killed it.
///   - `external_directory` is refused because a Forge turn owns exactly one directory: the worktree it was
///     launched in. Its production credentials are already stripped from the environment for the same reason,
///     and a tool that reached outside the worktree would be a path around that stripping rather than a task.
///
/// `spawn` is the subagent gate, applied to `task` and to the `tools` entry beside this block. It is an explicit
/// parameter rather than a read of the environment so the rendering stays a pure function of its inputs.
pub fn v2_agent_permissions(profile: &V2AgentProfile, spawn: bool) -> Value {
    let mut permissions = Map::new();
    // FIRST, and first in the emitted JSON: nothing is granted by default, so an unnamed tool — every MCP tool
    // among them — resolves here.
    permissions.insert("*".into(), json!("deny"));
    // Then the named grants. Reading is not a privilege a Forge role can be denied.
    for tool in READ_TOOLS {
        permissions.insert(tool.into(), json!("allow"));
    }
    match profile.authority {
        Authority::ReadOnly => {
            permissions.insert("edit".into(), json!("deny"));
            permissions.insert("patch".into(), json!("deny"));
        }
        Authority::Implement => {
            permissions.insert("edit".into(), json!("allow"));
            permissions.insert("patch".into(), json!("allow"));
        }
    }
    let web = if web_enabled(profile) {
        "allow"
    } else {
        "deny"
    };
    permissions.insert("webfetch".into(), json!(web));
    permissions.insert("websearch".into(), json!(web));
    // Planning scratch, not workspace state: a primary role keeps a todo list so it can be seen working through
    // one. A micro-subagent is a single question and gets neither the tool nor the bookkeeping.
    let todo = match profile.mode {
        AgentMode::Primary => "allow",
        AgentMode::Subagent => "deny",
    };
    permissions.insert("todowrite".into(), json!(todo));
    permissions.insert("question".into(), json!("deny"));
    permissions.insert("external_directory".into(), json!("deny"));
    permissions.insert("bash".into(), shell_rules(profile));
    permissions.insert("task".into(), task_rules(profile, spawn));
    permissions.insert("skill".into(), skill_rules(profile));
    Value::Object(permissions)
}

// -----------------------------------------------------------------------------------------------------------
// Config rendering
// -----------------------------------------------------------------------------------------------------------

/// The agent entry for one profile.
///
/// `spawn` reaches the `tools.task` switch as well as the `task` permission map: two independent statements that
/// this agent cannot start a subagent — the tool is absent AND every `task` name is denied — because either one
/// alone is a single point of failure.
pub fn v2_agent_block(profile: &V2AgentProfile, spawn: bool) -> Value {
    let mut block = Map::new();
    block.insert("description".into(), json!(profile.description));
    block.insert(
        "mode".into(),
        json!(match profile.mode {
            AgentMode::Primary => "primary",
            AgentMode::Subagent => "subagent",
        }),
    );
    block.insert("steps".into(), json!(profile.steps));
    block.insert("permission".into(), v2_agent_permissions(profile, spawn));
    if v2_children_for_agent_gated(profile.id, spawn).is_empty() {
        // Belt and braces: an agent with no allowlist cannot call the subagent tool at all, so even a
        // mis-rendered `task` rule could not make a child spawn a grandchild.
        block.insert("tools".into(), json!({ "task": false }));
    }
    Value::Object(block)
}

/// The complete Forge-owned OpenCode V2 configuration, with the subagent gate applied.
///
/// `subagent_depth` follows the same gate as the per-agent `task` rules, so the two can never disagree: with
/// subagents disarmed the top-level depth is 0 and no agent names a child, which means the vendor has nothing to
/// spawn even if it wanted to. (The schema sets `subagent_depth`'s `minimum` to 0, so 0 is a legal value rather
/// than a shape the vendor rejects — checked against the published config schema, not assumed.)
pub fn render_v2_agent_config_gated(spawn: bool) -> Value {
    let mut agents = Map::new();
    for profile in V2_AGENT_PROFILES.iter() {
        agents.insert(profile.id.to_string(), v2_agent_block(profile, spawn));
    }
    let mut config = Map::new();
    config.insert("$schema".into(), json!("https://opencode.ai/config.json"));
    // §4: Forge owns worktree state, rollback and recovery. A second snapshot mechanism underneath it is
    // forbidden, and disabling it is observable: with `snapshot:false` a `step_start` event carries no
    // `snapshot` key (measured on the live 2.x build), so Forge can assert the discipline held rather than
    // assume it.
    config.insert("snapshot".into(), json!(false));
    // §10: a child may not spawn a child — and with the gate disarmed, no child at all.
    config.insert(
        "subagent_depth".into(),
        json!(if spawn { MAX_CHILD_DEPTH } else { 0 }),
    );
    // §19: compaction is itself a model operation, so automatic compaction would be invisible Forge spend
    // against no Forge turn. It stays off until a later story can show usage and authority both survive it.
    config.insert("compaction".into(), json!({ "auto": false }));
    config.insert("agent".into(), Value::Object(agents));
    Value::Object(config)
}

/// The shipped default configuration. This is what the checked-in repo-root `opencode.json` holds and what a
/// reader of that file is entitled to assume: subagents disarmed, as `SUBAGENTS_DEFAULT` states.
pub fn render_v2_agent_config() -> Value {
    render_v2_agent_config_gated(SUBAGENTS_DEFAULT)
}

/// The configuration a live turn actually runs under, which is the default unless the operator armed the gate.
pub fn render_v2_agent_config_from_env() -> Value {
    render_v2_agent_config_gated(subagents_enabled_from_env())
}

/// The `OPENCODE_CONFIG_CONTENT` payload for an explicitly armed or disarmed gate. Pure, so tests can render
/// both postures without touching the process environment.
pub fn v2_agent_config_content_gated(spawn: bool) -> Result<String, String> {
    serde_json::to_string(&render_v2_agent_config_gated(spawn)).map_err(|error| error.to_string())
}

/// The `OPENCODE_CONFIG_CONTENT` payload Forge injects into the model subprocess: the DEFAULT posture.
///
/// NOTE ON SESSION WARMING: warming is deliberately NOT enabled here. A background warming request would be
/// spend Forge never asked for and cannot attribute (§4).
pub fn v2_agent_config_content() -> Result<String, String> {
    v2_agent_config_content_gated(SUBAGENTS_DEFAULT)
}

/// The `OPENCODE_CONFIG_CONTENT` payload a live turn runs under: the default, unless `FORGE_SUBAGENTS` armed the
/// gate. One function, so "what is on the subprocess" has exactly one answer.
pub fn v2_agent_config_content_from_env() -> Result<String, String> {
    v2_agent_config_content_gated(subagents_enabled_from_env())
}

/// The repo-root `opencode.json` Forge ships for humans and for the drift gate. It records the DEFAULT posture
/// (subagents disarmed); an operator who arms the gate changes what a turn is handed, not what the file says,
/// and the turn's own declaration line is what reports the difference.
pub fn render_v2_agent_config_pretty() -> Result<String, String> {
    let mut rendered = serde_json::to_string_pretty(&render_v2_agent_config())
        .map_err(|error| error.to_string())?;
    rendered.push('\n');
    Ok(rendered)
}
