//! ARCH.BOUNDARY — the QA surface owns no Git door (TST-ARCH-BOUNDARY-011).
//!
//! CONTRACT. The QA surface — the modules whose name says QA, plus the lanes the engine's QA nodes resolve
//! to — owns **no way to mutate git**: it runs no git command, resolves no lineage, and holds no handle to
//! the release path. The handbook states the rule twice and names a guard for each half: `AGENTS.md` forbids
//! pushing, merging or rebasing from a worker, and forbids a git commit from Scout, Assay and Inspector.
//! This test is the harness's own reading of what those guards protect — deliberately not a copy of them:
//! the guards scan `rust/forge/src`, this one scans the whole workspace, the lane map that assigns those
//! roles, and the agent-facing instruction door as well.
//!
//! THREE FACTS, each pinned in **both directions** — a new hole fails here (that is the point) and a pin the
//! tree no longer matches also fails, so the surface can only move by a deliberate edit of this file:
//!
//!   1. THE QA SURFACE NAMES NO GIT. Seven `.rs` files have a QA stem (pinned below). Their code contains
//!      none of the git tokens, none of the lineage tokens, and none of the release-surface identifiers —
//!      so there is nothing in the QA surface to commit, push, rebase or "check the sha" *with*. QA's own
//!      work runs through the engine's command port (`rust/forge/src/engine/runner.rs`, `run_command`), not
//!      through a process a QA module spawns itself.
//!   2. THE LANES. `rust/forge/src/engine/role_mapping.rs` is the one place that says which engine node is
//!      which lane. The three QA nodes (`qa_review`, `qa_verify`, `fast_qa_verify`) resolve to the QA lanes
//!      (Inspector, Assay) and never to DevOps. The entire node → lane map is pinned (twenty-two nodes), so
//!      re-pointing a QA node at the release lane, or adding one, is a deliberate edit. What the release lane
//!      owns is TST-ARCH-BOUNDARY-012's subject; what this test pins is that QA is not in it.
//!   3. ONE DOOR IN THE WHOLE WORKSPACE. Across every `.rs` under `rust/`, the three mutation verbs of
//!      `AGENTS.md` appear **once**, in one file: `rust/forge/src/engine/git_publish.rs`, behind
//!      `FORGE_ALLOW_PUBLISH`. The release handle is constructed at exactly one site — the composition root
//!      (`rust/forge/src/bin/forge.rs`) — and exported by the DevOps lane module, while the QA lane module
//!      exports verification and nothing else. The instruction door matches: a harness file that tells
//!      Scout, Assay or Inspector to commit is refused by rule `non-builder-commit-instruction`, whose role
//!      pattern is those same three names, and the vendor guardrail block quotes the handbook sentence it is
//!      anchored to.
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden:
//!
//!   * The QA-surface scan is by **name**: a file is QA when its stem starts with `qa` or `assay`, and the
//!     seven that exist are pinned as a set. A QA module hiding under a name that says nothing (`review.rs`)
//!     is caught by fact 3 instead, which counts the mutation verbs wherever they live.
//!   * The sweep matches the **argument literal** (`"push"`), not the English word: `Vec::push` is not a push
//!     and `"merge-base"` is not a merge (a lineage *read* is a different verb, banned separately by the
//!     `"merge-base"` token in the QA scan). Both readings are self-checked against planted samples below.
//!   * The QA surface may name a sha it was **handed** (`qa_classify.rs` carries `evaluated_sha` into the
//!     failure classifier). Naming a sha is not gating on one — the class `ENG-FORGE-QA-NO-GIT-GUARD-01`
//!     deleted a guard that did gate on it, because a git fact may never void paid work. The decidable defect
//!     is the git call, which is what is scanned.
//!   * `rust/forge/tests/handbook_engine_guards.rs` already proves the runtime half for `rust/forge/src`
//!     (the QA release path runs no git command and checks no lineage; only the publish path may push, merge
//!     or rebase) and `forge guard-lint` already proves every `guard:` path in the handbook resolves to a
//!     file holding a test. This test repeats neither: it pins *which* test, and it covers the tree those two
//!     do not read.
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: it reads sources, not behaviour.
//! It does not run the engine, does not prove which commands a lane is dispatched to run, and does not prove
//! a packet's assay commands are safe — only that the QA surface has no git door to walk through.
//!
//! Level: L0 Pure — filesystem reads only. No database, no network, no process spawned.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test arch_boundary__011__qa_cannot_own_git_mutations

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use test_harness::source;

/// The file doing the scanning. It names the mutation verbs it refuses, so the workspace sweep must not count
/// it as a site — the same self-reference TST-ARCH-BOUNDARY-005 records for its own detector.
const SELF: &str = "rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs";

/// Every `.rs` file in the tree whose name says QA — the surface that may own no git door. Pinned as a set:
/// a new QA module fails here until it is added, and once added it is scanned.
const QA_SURFACE: [&str; 7] = [
    "rust/forge/src/engine/assay.rs",
    "rust/forge/src/engine/qa_adjudicate.rs",
    "rust/forge/src/engine/qa_assert.rs",
    "rust/forge/src/engine/qa_classify.rs",
    "rust/forge/src/engine/qa_repair.rs",
    "rust/forge/src/qa_consistency.rs",
    "rust/forge/src/roles/qa.rs",
];

/// What the QA surface may not name in its code: the git binary and the mutating subcommands, the read-only
/// lineage subcommands (a QA lane that resolves ancestry is the guard `ENG-FORGE-QA-NO-GIT-GUARD-01` deleted),
/// a process spawn of its own, and the release surface — the handle type, its ops, its preview, its SQL
/// sibling, and the switch that permits a publish.
const QA_GIT_TOKENS: [&str; 22] = [
    "\"git\"",
    "\"push\"",
    "\"merge\"",
    "\"rebase\"",
    "\"commit\"",
    "\"checkout\"",
    "\"worktree\"",
    "\"rev-parse\"",
    "\"rev-list\"",
    "\"merge-base\"",
    "\"is-ancestor\"",
    "\"cat-file\"",
    "\"ls-remote\"",
    "Command::new",
    "HostReleaseExecutor",
    "GitReleaseOps",
    "preview_publish",
    "PublishOutcome",
    "ForgeReleaseExecutor",
    "DbForgeReleaseExecutor",
    "ForgeReleaseOperations",
    "FORGE_ALLOW_PUBLISH",
];

/// Two QA files that must still say the thing they are for. Without these the scan could pass by reading the
/// wrong file, or an empty one — the failure mode `source.rs` warns about.
const QA_READ_PROOF: [(&str, &str); 2] = [
    ("rust/forge/src/engine/qa_repair.rs", "route_qa_result"),
    ("rust/forge/src/engine/qa_classify.rs", "evaluated_sha"),
];
/// The three mutation verbs the handbook refuses from a worker, as an author writes them in code.
const MUTATION_VERBS: [&str; 3] = ["\"push\"", "\"merge\"", "\"rebase\""];

/// The one file in the workspace permitted to name a mutation verb.
const THE_ONE_PUSHER: &str = "rust/forge/src/engine/git_publish.rs";

/// The switch that must sit between the tree and that push.
const PUBLISH_SWITCH: &str = "FORGE_ALLOW_PUBLISH";

/// How close the push must be to its switch: the argument and its gate are one statement, not a flag read
/// somewhere else in the file.
const PUBLISH_GATE_WINDOW: usize = 12;

/// How far under a `LaneId::` its `LeadPhase::` may stand — the Lead's phase is the line below its lane.
const LEAD_PHASE_WINDOW: usize = 4;

/// The only site that builds the release handle — the composition root, not a lane.
const RELEASE_HANDLE_WIRED: &str = "rust/forge/src/bin/forge.rs";

/// The QA lane module's entire export surface: four verification items, no handle.
const QA_LANE_EXPORTS: [&str; 4] = [
    "crate::engine::assay::adjudicate_assay",
    "crate::engine::assay::collect_assay_evidence",
    "crate::engine::qa_assert::assertion_resolution",
    "crate::engine::qa_repair::route_qa_result",
];

/// The lane identifiers the engine declares, pinned: the release lane is a lane, and QA is not it.
const LANE_IDS: [&str; 7] = [
    "Scout",
    "Architect",
    "Lead",
    "Smith",
    "Assay",
    "Inspector",
    "DevOps",
];

/// The whole engine node → lane map, as `node -> Lane` (or `node -> Lane:Phase` for the Lead's three phases).
const ENGINE_NODES: [&str; 22] = [
    "architect -> Architect",
    "deploy -> DevOps",
    "diagnose_scout -> Scout",
    "failure_classifier -> Lead:Pre",
    "fast_qa_verify -> Assay",
    "fast_repair_smith -> Smith",
    "fast_smith -> Smith",
    "feature_scout -> Scout",
    "lead_post -> Lead:Post",
    "lead_pre -> Lead:Pre",
    "lead_solo_implement -> Lead:Implement",
    "production_smoke -> DevOps",
    "qa_review -> Inspector",
    "qa_verify -> Assay",
    "repair_architect -> Architect",
    "repair_devops -> DevOps",
    "repair_scout -> Scout",
    "repair_smith -> Smith",
    "research_architect -> Architect",
    "research_scout -> Scout",
    "smith -> Smith",
    "smith_split_work -> Smith",
];

/// The QA nodes, and the QA lanes they must land in. The other nineteen nodes are 012's subject.
const QA_NODES: [&str; 3] = [
    "fast_qa_verify -> Assay",
    "qa_review -> Inspector",
    "qa_verify -> Assay",
];

/// The lint rule that refuses a packet telling a non-Builder role to commit, the message it carries, and the
/// role pattern behind it — the same three names the handbook's `Never` line names.
const NON_BUILDER_COMMIT_RULE: &str = "non-builder-commit-instruction";
const NON_BUILDER_COMMIT_MESSAGE: &str =
    "instructs Scout/Assay/Inspector to commit; only the Builder role commits";
const ROLES_THAT_MAY_NOT_COMMIT: &str =
    r#"pattern!(roles_that_may_not_commit, r"(?i)\b(scout|assay|inspector)\b");"#;

/// The two directive shapes the rule refuses («a role, commit» and «give a role the commit»), and the tests
/// that hold the rule — including the one for the `Never` heading the rule statement itself used to trip.
const COMMIT_DIRECTIVE_DETECTORS: [&str; 2] = ["role_opens_instruction", "role_is_handed_the_act"];
const LINT_TESTS: [&str; 2] = [
    "telling_inspector_to_commit_fails_and_forbidding_it_does_not",
    "a_never_heading_protects_its_list_items",
];

/// The vendor guardrail that restates the rule, and the handbook sentence it is anchored to.
const VENDOR_GUARDRAIL: &str =
    "Only the Builder role commits. Scout, Assay and Inspector never do.";
const VENDOR_GUARDRAIL_ANCHOR: &str = "Commit on the worker branch only when the role is Builder";

/// The handbook's two `Never` lines on this subject, verbatim, each with the guard it declares. The guard
/// path is parsed back out of the line and must resolve to a file naming the test that holds the rule.
const AGENTS_GUARD_LINES: [&str; 2] = [
    "- Push, merge, or rebase from a worker. guard: rust/forge/tests/handbook_engine_guards.rs",
    "- Keep a git commit as Scout, Assay, or Inspector. guard: rust/cli/src/forge/lint.rs",
];

/// Each declared guard path, and the test inside it that is the guard. `forge guard-lint` already proves the
/// path resolves to a file that holds a test; this pins *which* test, so renaming the guard is an edit here.
const GUARD_TESTS: [(&str, &str); 3] = [
    (
        "rust/forge/tests/handbook_engine_guards.rs",
        "the_qa_release_path_runs_no_git_command_and_checks_no_lineage",
    ),
    (
        "rust/forge/tests/handbook_engine_guards.rs",
        "only_the_publish_path_may_push_merge_or_rebase",
    ),
    (
        "rust/cli/src/forge/lint.rs",
        "non-builder-commit-instruction",
    ),
];

/// Floors. A walker that found nothing would report a clean tree and pass; these make that a failure.
const RUST_SOURCE_FLOOR: usize = 500;
const QA_SURFACE_FLOOR: usize = 5;
const ENGINE_NODE_FLOOR: usize = 20;
const TOKEN_FLOOR: usize = 20;

fn in_repo(relative: &str) -> PathBuf {
    source::repo_root().join(relative)
}

/// The CODE in `source_text`: `//` and `/* … */` comments removed, string and char literals copied whole.
///
/// Written by hand rather than reusing `source::code_of` because that reads a `//` inside a string literal as
/// a comment — `"https://…"` would swallow the rest of the line, and a quote inside a char literal would
/// swallow the rest of the file. A scan whose subject is `"push"` cannot afford either, so the stripper must
/// know where a literal begins. Planted samples check all four behaviours below.
fn code_of_file(source_text: &str) -> String {
    let mut out = String::with_capacity(source_text.len());
    let mut characters = source_text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '/' if characters.peek() == Some(&'/') => {
                for next in characters.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if characters.peek() == Some(&'*') => {
                characters.next();
                let mut previous = ' ';
                for next in characters.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    if next == '\n' {
                        out.push('\n');
                    }
                    previous = next;
                }
                out.push(' ');
            }
            '"' => {
                out.push(character);
                copy_string(&mut characters, &mut out);
            }
            '\'' => {
                out.push(character);
                copy_char_literal(&mut characters, &mut out);
            }
            'r' if matches!(characters.peek(), Some('"') | Some('#')) => {
                // A raw string is copied whole, so its clutter can never hide the code behind it.
                out.push(character);
                let mut hashes = 0usize;
                while characters.peek() == Some(&'#') {
                    characters.next();
                    out.push('#');
                    hashes += 1;
                }
                if characters.peek() == Some(&'"') {
                    characters.next();
                    out.push('"');
                    let terminator = format!("\"{}", "#".repeat(hashes));
                    let mut window = String::new();
                    for next in characters.by_ref() {
                        window.push(next);
                        out.push(next);
                        if window.ends_with(&terminator) {
                            break;
                        }
                        if window.len() > terminator.len() {
                            window.remove(0);
                        }
                    }
                }
            }
            _ => out.push(character),
        }
    }
    out
}

/// Copy a `"…"` literal whole, escapes included.
fn copy_string(characters: &mut std::iter::Peekable<std::str::Chars<'_>>, out: &mut String) {
    while let Some(character) = characters.next() {
        out.push(character);
        if character == '\\' {
            if let Some(escaped) = characters.next() {
                out.push(escaped);
            }
        } else if character == '"' {
            return;
        }
    }
}

/// Copy a `'…'` char literal whole. A lifetime (`'static`) is left alone: it closes nothing, and treating its
/// apostrophe as an opening quote would eat the file.
fn copy_char_literal(characters: &mut std::iter::Peekable<std::str::Chars<'_>>, out: &mut String) {
    let lookahead: String = characters.clone().take(4).collect();
    let Some(closing) = lookahead.find('\'') else {
        return;
    };
    if closing > 2 {
        return;
    }
    for _ in 0..=closing {
        if let Some(character) = characters.next() {
            out.push(character);
        }
    }
}

/// The double-quoted literals in `text`, in order.
fn literals(text: &str) -> Vec<String> {
    text.split('"')
        .skip(1)
        .step_by(2)
        .map(|literal| literal.to_string())
        .collect()
}

/// A file NAME the QA surface is read from: an `.rs` file whose stem starts with `qa` or `assay`.
fn is_qa_surface_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".rs") else {
        return false;
    };
    stem.starts_with("qa") || stem.starts_with("assay")
}

/// Every `.rs` file under `rust/`, excluding the file that runs this scan, as (relative path, code).
fn workspace_code() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in source::sources_under(&source::rust_root()) {
        let relative = source::relative(&path);
        if relative == SELF {
            continue;
        }
        out.push((relative, code_of_file(&source::read(&path))));
    }
    out
}

/// The QA surface as the tree declares it: every `.rs` file whose name says QA, sorted.
fn qa_surface() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (path, _) in workspace_code() {
        let name = Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if is_qa_surface_name(&name) {
            out.insert(path);
        }
    }
    out
}

/// The `pub use …;` exports of a module, as the full paths they name: `crate::engine::assay::{a, b}` yields
/// `crate::engine::assay::a` and `…::b`.
fn role_exports(relative: &str) -> BTreeSet<String> {
    let text = code_of_file(&source::read(&in_repo(relative)));
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub use ") else {
            continue;
        };
        let rest = rest.strip_suffix(';').unwrap_or(rest);
        match rest.split_once('{') {
            Some((module, items)) => {
                let module = module.trim_end_matches("::");
                for item in items.trim_end_matches('}').split(',') {
                    let item = item.trim();
                    if !item.is_empty() {
                        out.insert(format!("{module}::{item}"));
                    }
                }
            }
            None => {
                out.insert(rest.trim().to_string());
            }
        }
    }
    out
}

/// The `LaneId` variants the engine declares, in the order it declares them.
fn lane_ids(text: &str) -> Vec<String> {
    let body = text
        .split_once("pub enum LaneId")
        .expect("role_mapping.rs declares the LaneId enum")
        .1
        .split_once('{')
        .expect("the LaneId enum has a body")
        .1
        .split_once('}')
        .expect("the LaneId enum body closes")
        .0;
    body.lines()
        .map(|line| {
            source::code_of(line)
                .trim()
                .trim_end_matches(',')
                .to_string()
        })
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect()
}

/// The node → lane map a `forge_role_node_plan` body declares.
///
/// A node arm is a line with `=>` that carries quoted node names; the lane is the `LaneId::…` on a line that
/// follows it, and the Lead's `LeadPhase::…` stands on a line just under that (where rustfmt puts it). An arm
/// whose shape this reader cannot follow is a failure, never a skip: a node missing from the map is exactly
/// the hole this pin exists to show. The one arm that may carry no name is the wildcard refusal.
fn node_lanes_in(body: &str) -> BTreeMap<String, String> {
    let body = code_of_file(body);
    let lines: Vec<&str> = body.lines().collect();
    let mut lanes: BTreeMap<String, String> = BTreeMap::new();
    let mut pending: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.contains("=>") && !line.contains("LaneId::") {
            let nodes = literals(line);
            if nodes.is_empty() {
                assert!(
                    line.contains("other"),
                    "an engine-node arm this reader cannot follow, so a node would be missing from the \
                     pin: `{}`",
                    line.trim()
                );
                pending.clear();
                continue;
            }
            pending = nodes;
            continue;
        }
        let Some(position) = line.find("LaneId::") else {
            continue;
        };
        let lane: String = line[position + "LaneId::".len()..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        assert!(
            !lane.is_empty(),
            "a `LaneId::` with no lane after it: `{}`",
            line.trim()
        );
        // The phase is searched only in the lines under its own lane: three lines is room for rustfmt to
        // wrap it, and too little to reach the next node's plan, which opens with an arm head.
        let phase: Option<String> = lines[index + 1..(index + LEAD_PHASE_WINDOW).min(lines.len())]
            .iter()
            .find_map(|nearby| {
                nearby.find("LeadPhase::").map(|at| {
                    nearby[at + "LeadPhase::".len()..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect::<String>()
                })
            })
            .filter(|phase| !phase.is_empty());
        assert!(
            !pending.is_empty(),
            "a lane with no node above it: `{lane}` — the arm head was not read, which would hide a node"
        );
        let value = match phase {
            Some(phase) => format!("{lane}:{phase}"),
            None => lane,
        };
        for node in pending.drain(..) {
            lanes.insert(node, value.clone());
        }
    }
    lanes
}

/// The engine's node → lane map, read from the one match that declares it in
/// `rust/forge/src/engine/role_mapping.rs`.
fn engine_node_lanes() -> BTreeMap<String, String> {
    let text = source::read(&in_repo("rust/forge/src/engine/role_mapping.rs"));
    let body = text
        .split_once("pub fn forge_role_node_plan")
        .expect("role_mapping.rs declares forge_role_node_plan")
        .1
        .split_once("const PREFIX")
        .expect("the node plan function stands before the directive prefix")
        .0;
    assert!(
        body.contains("No Forge agent-runtime mapping for engine node"),
        "the node plan must refuse an unknown node — a map that quietly falls back to a lane is how a node \
         with no lane goes unnoticed"
    );
    let lanes = node_lanes_in(body);
    assert!(
        lanes.len() >= ENGINE_NODE_FLOOR,
        "the node plan reader found only {} nodes (floor {ENGINE_NODE_FLOOR}); the map it reads has moved",
        lanes.len()
    );
    lanes
}

/// The index of the first line containing `needle`, for the publish-gate window check.
fn first_line_of(text: &str, needle: &str) -> Option<usize> {
    text.lines().position(|line| line.contains(needle))
}

/// Whether a line CONSTRUCTS the release handle — as opposed to declaring its type (`struct`) or writing its
/// methods (`impl`), which are both the handle's own definition and not a second door to git.
fn builds_release_handle(line: &str) -> bool {
    line.contains("HostReleaseExecutor {") && !line.contains("struct ") && !line.contains("impl ")
}

#[test]
#[allow(non_snake_case)]
fn arch_boundary_011__qa_cannot_own_git_mutations() {
    // ── 1. THE DETECTORS, against planted samples. A scan that cannot fail is not a check. ────────────────

    // A comment naming a verb is not code — the handbook talks about pushing, and must not be read as one.
    let stripped = code_of_file("/// The lane never runs `git push`.\nfn f() {}\n");
    assert!(
        !stripped.contains("push") && stripped.contains("fn f"),
        "a comment was read as code, or the line after it was lost: {stripped:?}"
    );

    // A block comment is removed and the code after it survives.
    let stripped = code_of_file("/* git push */ let quorum = 1;\n");
    assert!(
        !stripped.contains("push") && stripped.contains("quorum"),
        "a block comment was read as code, or ate the statement after it: {stripped:?}"
    );

    // `//` inside a string literal must not truncate the line: a URL is not a comment.
    let stripped = code_of_file("let endpoint = \"https://example.test///\"; let kept = true;\n");
    assert!(
        stripped.contains("kept"),
        "a `//` inside a string ate the rest of the line: {stripped:?}"
    );

    // A raw string is copied whole: its clutter cannot hide the code after it.
    let stripped = code_of_file("let sample = r#\"cargo test\"#;\nlet quorum = 2;\n");
    assert!(
        stripped.contains("quorum") && stripped.contains("cargo test"),
        "a raw string swallowed the rest of the file: {stripped:?}"
    );

    // A quote inside a char literal must not open a string that eats the file.
    let stripped = code_of_file("let quote = '\"'; let quorum = 3;\n");
    assert!(
        stripped.contains("quorum"),
        "a quote inside a char literal swallowed the file: {stripped:?}"
    );

    // A lifetime is not a char literal: reading `'static` as one would eat the rest of its line.
    let stripped =
        code_of_file("fn borrow<'a>(value: &'a str) -> &'a str { value }\nlet quorum = 4;\n");
    assert!(
        stripped.contains("quorum"),
        "a lifetime was read as a char literal: {stripped:?}"
    );

    // The token is the argument literal, not the English word.
    assert!(
        code_of_file("let args = [\"push\", \"origin\", \"main\"];").contains("\"push\""),
        "a real git argument was not seen"
    );
    assert!(
        !code_of_file("blockers.push(result);").contains("\"push\""),
        "`Vec::push` was read as a git push"
    );
    assert!(
        !code_of_file("let span = [\"merge-base\", \"main\"];").contains("\"merge\""),
        "`merge-base` (a lineage read) was read as a merge"
    );

    // The QA-name detector, in both directions.
    assert!(is_qa_surface_name("qa_adjudicate.rs"));
    assert!(is_qa_surface_name("assay.rs"));
    assert!(is_qa_surface_name("qa.rs"));
    assert!(!is_qa_surface_name("assertion.rs"));
    assert!(!is_qa_surface_name("qa_surface.md"));
    assert!(
        !is_qa_surface_name("aqua.rs"),
        "a stem that merely starts with `a` is not QA"
    );

    // The node-plan reader, on a planted body: one plain node, one Lead phase, and the wildcard refusal.
    let planted = concat!(
        "        match node_id {\n",
        "            \"qa_review\" => ForgeRoleNodePlan {\n",
        "                lane: LaneId::Inspector,\n",
        "                lead_phase: None,\n",
        "            },\n",
        "            \"lead_pre\" | \"failure_classifier\" => ForgeRoleNodePlan {\n",
        "                lane: LaneId::Lead,\n",
        "                lead_phase: Some(LeadPhase::Pre),\n",
        "            },\n",
        "            other => {\n",
        "                return Err(format!(\"No Forge agent-runtime mapping for engine node '{other}'\"));\n",
        "            }\n",
        "        }\n",
    );
    let planted_lanes = node_lanes_in(planted);
    assert_eq!(
        planted_lanes.get("qa_review").map(String::as_str),
        Some("Inspector")
    );
    assert_eq!(
        planted_lanes.get("lead_pre").map(String::as_str),
        Some("Lead:Pre")
    );
    assert_eq!(
        planted_lanes.get("failure_classifier").map(String::as_str),
        Some("Lead:Pre")
    );
    assert_eq!(
        planted_lanes.len(),
        3,
        "the planted map yielded {planted_lanes:?}"
    );

    assert!(
        QA_GIT_TOKENS.len() >= TOKEN_FLOOR,
        "the token list has shrunk"
    );

    // ── 2. THE QA SURFACE NAMES NO GIT, NO LINEAGE, NO RELEASE HANDLE. ────────────────────────────────────

    let swept = workspace_code();
    assert!(
        swept.len() >= RUST_SOURCE_FLOOR,
        "the sweep read {} `.rs` files under rust/ (floor {RUST_SOURCE_FLOOR}) — a walker that finds nothing \
         reports a clean tree and passes",
        swept.len()
    );
    assert!(
        in_repo(SELF).is_file(),
        "this test does not live where it says it does: {SELF}"
    );

    let qa = qa_surface();
    assert!(
        qa.len() >= QA_SURFACE_FLOOR,
        "the QA surface came out as {} files (floor {QA_SURFACE_FLOOR}): {qa:?}",
        qa.len()
    );
    let pinned: BTreeSet<String> = QA_SURFACE.iter().map(|path| path.to_string()).collect();
    assert_eq!(
        qa, pinned,
        "the QA-named modules and the pin have drifted. A NEW ONE IS SCANNED, NOT ALLOWED: add it to \
         QA_SURFACE in this file, which is the deliberate act of saying a QA module exists. If one was \
         RENAMED away, say so here too"
    );

    for (path, proof) in QA_READ_PROOF {
        let text = source::read(&in_repo(path));
        assert!(
            text.contains(proof),
            "{path} no longer says `{proof}` — the scan below would pass by reading the wrong file"
        );
    }

    for (path, code) in &swept {
        if !qa.contains(path) {
            continue;
        }
        for token in QA_GIT_TOKENS {
            assert!(
                !code.contains(token),
                "{path} names `{token}`. The QA surface owns no git door: it does not run git, resolve \
                 lineage, or hold the release handle — it verifies what a Builder produced. Its own commands \
                 go through the engine's command port (rust/forge/src/engine/runner.rs). If a QA module \
                 genuinely needs this name, that is an architecture decision, not a test edit"
            );
        }
    }

    // ── 3. THE LANES. QA nodes resolve to QA lanes, and never to the release lane. ─────────────────────────

    let role_mapping = source::read(&in_repo("rust/forge/src/engine/role_mapping.rs"));
    assert_eq!(
        lane_ids(&role_mapping),
        LANE_IDS
            .iter()
            .map(|lane| lane.to_string())
            .collect::<Vec<String>>(),
        "the lane inventory changed"
    );

    let lanes = engine_node_lanes();
    let expected: BTreeMap<String, String> = ENGINE_NODES
        .iter()
        .map(|arm| {
            let (node, lane) = arm
                .split_once(" -> ")
                .expect("every pinned arm is `node -> lane`");
            (node.to_string(), lane.to_string())
        })
        .collect();
    assert_eq!(
        lanes, expected,
        "the engine's node → lane map drifted. Every node is pinned: adding a node (especially a QA node) \
         must be a deliberate edit here, and a QA node landing in the release lane is this test's whole point"
    );

    for arm in QA_NODES {
        let (node, lane) = arm
            .split_once(" -> ")
            .expect("every pinned QA arm is `node -> lane`");
        assert_eq!(
            lanes.get(node).map(String::as_str),
            Some(lane),
            "the QA node `{node}` must resolve to `{lane}`"
        );
        assert!(
            !lane.starts_with("DevOps"),
            "`{node}` resolves to the release lane — a QA node may not own release"
        );
    }
    let qa_lanes: BTreeSet<&str> = QA_NODES
        .iter()
        .map(|arm| arm.split_once(" -> ").expect("`node -> lane`").1)
        .collect();
    assert_eq!(
        qa_lanes,
        BTreeSet::from(["Assay", "Inspector"]),
        "the engine's QA nodes must land in the QA lanes and nowhere else"
    );

    // ── 4. NO HANDLE: the QA lane exports verification, the release handle is built in one place. ──────────

    assert_eq!(
        role_exports("rust/forge/src/roles/qa.rs"),
        QA_LANE_EXPORTS.iter().map(|path| path.to_string()).collect::<BTreeSet<String>>(),
        "the QA lane module's exports are pinned: four verification items, no release handle. An export added \
         here is the QA lane being handed a capability it may not own"
    );
    let dev_ops_exports = role_exports("rust/forge/src/roles/dev_ops.rs");
    assert!(
        dev_ops_exports.iter().any(|item| item.ends_with("HostReleaseExecutor"))
            && dev_ops_exports.iter().any(|item| item.ends_with("DbForgeReleaseExecutor")),
        "the release handle belongs to the DevOps lane module ({dev_ops_exports:?}) — which lane owns \
         release is TST-ARCH-BOUNDARY-012's subject, but it is not QA's"
    );

    let constructors: BTreeSet<String> = swept
        .iter()
        .filter(|(_, code)| code.lines().any(builds_release_handle))
        .map(|(path, _)| path.clone())
        .collect();
    assert!(
        builds_release_handle("    let release = Arc::new(HostReleaseExecutor {")
            && !builds_release_handle("pub struct HostReleaseExecutor {")
            && !builds_release_handle("impl HostReleaseExecutor {"),
        "the construction detector must see a construction and not the handle's own definition"
    );
    assert_eq!(
        constructors,
        BTreeSet::from([RELEASE_HANDLE_WIRED.to_string()]),
        "the release handle is constructed at exactly one site — the composition root — so no lane and no \
         QA module can build itself a door to git"
    );

    // ── 5. ONE DOOR: the mutation verbs appear once in the whole workspace, behind the publish switch. ─────

    let mut pushers: BTreeMap<String, usize> = BTreeMap::new();
    for (path, code) in &swept {
        let count: usize = MUTATION_VERBS
            .iter()
            .map(|verb| code.matches(*verb).count())
            .sum();
        if count > 0 {
            pushers.insert(path.clone(), count);
        }
    }
    assert_eq!(
        pushers.keys().cloned().collect::<Vec<String>>(),
        vec![THE_ONE_PUSHER.to_string()],
        "push/merge/rebase is named outside the publish path: {pushers:?}. A worker pushes and it is a \
         collision; only the release path publishes, and it is one file"
    );
    assert_eq!(
        pushers.get(THE_ONE_PUSHER).copied(),
        Some(1),
        "the publish path should name one mutation verb, in one place"
    );
    let publisher = &swept
        .iter()
        .find(|(path, _)| path == THE_ONE_PUSHER)
        .expect("git_publish.rs was read by the sweep")
        .1;
    let gate_line = first_line_of(publisher, PUBLISH_SWITCH)
        .expect("the publish path reads FORGE_ALLOW_PUBLISH before it publishes");
    let verb_line = first_line_of(publisher, "\"push\"").expect("the publish path pushes");
    assert!(
        gate_line < verb_line && verb_line - gate_line <= PUBLISH_GATE_WINDOW,
        "the push is not gated by {PUBLISH_SWITCH} at the point of use (switch line {}, push line {})",
        gate_line + 1,
        verb_line + 1
    );

    // ── 6. THE INSTRUCTION DOOR: a packet cannot tell a QA role to commit, and the handbook says so twice. ─

    let lint = source::read(&in_repo("rust/cli/src/forge/lint.rs"));
    assert!(
        lint.contains(ROLES_THAT_MAY_NOT_COMMIT),
        "the rule's role pattern changed: the lint must name Scout, Assay and Inspector — the same three \
         roles the handbook's `Never` line names"
    );
    assert!(
        lint.contains(NON_BUILDER_COMMIT_RULE),
        "the lint rule `{NON_BUILDER_COMMIT_RULE}` is the enforcement of the handbook line and is gone from \
         the lint"
    );
    for detector in COMMIT_DIRECTIVE_DETECTORS {
        assert!(
            lint.contains(detector),
            "the lint no longer refuses `{detector}`"
        );
    }
    for test_name in LINT_TESTS {
        assert!(
            lint.contains(test_name),
            "the lint test `{test_name}` is gone"
        );
    }
    let harness = source::read(&in_repo("rust/cli/src/forge/lint/harness.rs"));
    assert_eq!(
        harness.matches(NON_BUILDER_COMMIT_RULE).count(),
        1,
        "one rule, declared in one place"
    );
    assert!(
        harness.contains(NON_BUILDER_COMMIT_MESSAGE),
        "the rule's message is what a Builder reads when the gate fires, and it must name what was refused"
    );

    let agents = source::read(&in_repo("AGENTS.md"));
    assert!(
        agents.contains(VENDOR_GUARDRAIL_ANCHOR),
        "the handbook sentence the vendor guardrail is anchored to is gone; `forge sync-agents` and the \
         packet lint both fail without it"
    );
    let vendor = source::read(&in_repo("rust/cli/src/forge/vendor_block.rs"));
    assert!(
        vendor.contains(VENDOR_GUARDRAIL) && vendor.contains(VENDOR_GUARDRAIL_ANCHOR),
        "the vendor block must carry the rule and the handbook sentence that backs it"
    );
    for line in AGENTS_GUARD_LINES {
        assert!(
            agents.lines().any(|text| text.trim() == line),
            "the handbook no longer states this rule as it did: `{line}`"
        );
        let guard = line
            .split("guard: ")
            .nth(1)
            .expect("the line declares a guard path");
        assert!(
            in_repo(guard).is_file(),
            "`{guard}` is declared as the guard for this rule and is not a file: a guard path that does not \
             resolve reads as enforced while enforcing nothing"
        );
    }
    for (path, test_name) in GUARD_TESTS {
        assert!(
            source::read(&in_repo(path)).contains(test_name),
            "{path} no longer holds `{test_name}`, the test that guards this rule"
        );
    }
}
