//! FORGE PACKET LINT — scan the HARNESS, not the app.
//!
//! Rust replacement for the retired `scripts/forge-packet-lint.ts` (behind `pnpm forge:packet-lint`),
//! which imported five modules deleted with the TypeScript application in `4cf98110` and therefore
//! exited `ERR_MODULE_NOT_FOUND` — including inside `scripts/vercel-build-prod.sh`, where it was wired
//! as a non-fatal release step. Same rules, same rule names, same exit contract, one language.
//!
//! The useful half of the intake this came from was never its 286 skills; it was the idea that the
//! things which steer an agent (packets, skill packs, the memory file, the allowlists) can be checked
//! the same way code is. A packet that names a skill which does not exist, or that tells Inspector to
//! commit, is a defect in the harness — and harness defects are silent, because nothing runs them.
//!
//! SCOPE IS DELIBERATELY NARROW: the packet/skill/memory files, the generated artefacts and the
//! allowlists. It does not walk the application, and it does not judge prose. Findings are `fail` or
//! `warn`, and the CLI is NON-BLOCKING by default so drift that pre-dates the lint does not stop a
//! release: a doc gate must not be able to block a release. `--strict` restores blocking.
//!
//! One deliberate change from the TypeScript: an unreadable or malformed baseline is a FAILING finding
//! (`baseline-unreadable`) instead of a silently empty debt list. The TypeScript caught the parse error
//! and returned "no debt", which meant a corrupt file quietly turned recorded debt into violations —
//! a silent refusal, which `docs/agent/decisions/silent-refusal-is-a-defect.md` calls a defect.
//!
//! Usage:
//!   cargo run -p cli -- forge harness-lint [--strict] [--format json]

use crate::forge::citations::{count_lines, cited_repo_paths, line_citations, Resolution, Resolver};
use crate::forge::decision::{parse_decision_file, validate_statement, DECISION_STATUSES};
use crate::forge::secret_shapes::shapes_in_line;
use crate::forge::vendor_block::{
    block_drifted, has_block, orphaned_guardrails, MANAGED_VENDOR_FILES,
};
use crate::forge::Failure;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

pub const BASELINE_PATH: &str = "docs/agent/harness-lint-baseline.json";

/// The known skill ids. The retired `agent-runtime/skills.ts` held this list too, for the deleted
/// engine's own readers; this is now the list the live gate reads.
const KNOWN_SKILLS: [&str; 11] = [
    "neon", "forms", "workflow", "ui", "planner", "cruiser", "knip", "ripwire", "rtk", "semgrep",
    "serena",
];

const MAX_SKILLS_PER_PACKET: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Fail,
    Warn,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Fail => "fail",
            Level::Warn => "warn",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub level: Level,
    pub rule: &'static str,
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
    pub baselined: bool,
}

#[derive(Clone)]
pub struct HarnessFile {
    pub path: String,
    pub content: String,
}

/// Every pattern the lint matches with, compiled once. Declared through one helper so a bad pattern is
/// a panic at first use rather than a silent no-match — a lint that stops matching is a lint that
/// reports success.
macro_rules! pattern {
    ($name:ident, $regex:literal) => {
        fn $name() -> &'static Regex {
            static PATTERN: OnceLock<Regex> = OnceLock::new();
            PATTERN.get_or_init(|| Regex::new($regex).expect("lint pattern is a constant"))
        }
    };
}

pattern!(heading_pattern, r"^##\s+(.+?)\s*$");
pattern!(packet_path_pattern, r"docs/agent/packets/[^/]+\.md$");
pattern!(map_page_pattern, r"docs/agent/(ORIENTATION|MAP-[^/]+)\.md$");
pattern!(memory_path_pattern, r"docs/agent/MEMORY\.md$");
pattern!(skill_file_pattern, r"docs/agent/skills/[^/]+\.md$");
pattern!(manifest_path_pattern, r"docs/agent/manifest/[^/]+\.md$");
pattern!(decision_path_pattern, r"docs/agent/decisions/[^/]+\.md$");
pattern!(memory_entry_pattern, r"^[-*]\s+\*\*");
pattern!(date_pattern, r"\d{4}-\d{2}-\d{2}");
pattern!(manifest_row_pattern, r"^- `([^`]+)`( \*\*MISSING\*\*)? — ");
pattern!(roles_that_may_not_commit, r"(?i)\b(scout|assay|inspector)\b");
pattern!(
    prohibition,
    r"(?i)\b(?:never|not|no|may not|must not|do not|don't|cannot|can't|forbidden|prohibited)\b"
);
pattern!(
    role_opens_instruction,
    r"(?i)^(?:then\s+|and\s+|next,?\s+|-\s*)?(?:scout|assay|inspector)\b[^.\n]{0,60}?\bcommit"
);
pattern!(
    role_is_handed_the_act,
    r"(?i)\b(?:then|after|have|let)\s+(?:scout|assay|inspector)\s+(?:commit|commits|should commit|must commit|will commit)"
);
pattern!(list_item_pattern, r"^[-*]\s+");
pattern!(prohibition_heading_pattern, r"(?i)^#*\s*(never|do not|don't|must not|forbidden)\b");
pattern!(skills_heading_pattern, r"(?i)^skills$");

fn is_packet(path: &str) -> bool {
    packet_path_pattern().is_match(path) && !path.ends_with("README.md")
}

fn is_map_page(path: &str) -> bool {
    map_page_pattern().is_match(path)
}

/// `## <name>` headings with their line number and the lines under them, up to the next `##`.
struct Heading {
    name: String,
    line: usize,
    body: String,
}

fn headings_in(content: &str) -> Vec<Heading> {
    let mut out: Vec<Heading> = Vec::new();
    let mut current: Option<Heading> = None;
    for (index, line) in content.split('\n').enumerate() {
        if let Some(captures) = heading_pattern().captures(line) {
            if let Some(heading) = current.take() {
                out.push(heading);
            }
            current = Some(Heading {
                name: captures[1].trim().to_string(),
                line: index + 1,
                body: String::new(),
            });
            continue;
        }
        if let Some(heading) = current.as_mut() {
            heading.body.push_str(line);
            heading.body.push('\n');
        }
    }
    if let Some(heading) = current {
        out.push(heading);
    }
    out
}

/// True when a list item sits under a prohibition heading (`Never`, `Do not`, `Ask first`).
///
/// `AGENTS.md`'s Never list is the rule statement, not an instruction: its items read as imperatives
/// ("Keep a git commit as Scout, Assay, or Inspector.") and the only clue is the heading above them.
fn prohibition_for(lines: &[&str], index: usize) -> bool {
    let mut cursor = index as isize;
    while cursor >= 0 && index as isize - cursor <= 25 {
        let line = lines[cursor as usize].trim();
        if line.is_empty() || list_item_pattern().is_match(line) {
            cursor -= 1;
            continue;
        }
        return prohibition_heading_pattern().is_match(line);
    }
    false
}

/// Is this a DIRECTIVE telling a role to commit, or prose about commits?
///
/// Measured against real text on 2026-09-15, the naive "role + commit" rule flagged every sentence that
/// merely described the rule. So the role must be the SUBJECT of the instruction, not a word inside a
/// sentence about one.
fn is_commit_directive(window: &str) -> bool {
    if !roles_that_may_not_commit().is_match(window) {
        return false;
    }
    if prohibition().is_match(window) {
        return false;
    }
    let trimmed = window.trim();
    role_opens_instruction().is_match(trimmed) || role_is_handed_the_act().is_match(window)
}

fn finding(
    level: Level,
    rule: &'static str,
    file: &str,
    line: Option<usize>,
    message: String,
) -> Finding {
    Finding {
        level,
        rule,
        file: file.to_string(),
        line,
        message,
        baselined: false,
    }
}

/// The rules, in order. `baseline` holds `path::rule` keys whose findings are reported as warnings
/// instead of failures: a NEW violation blocks, pre-existing debt is reported.
pub fn lint_harness(
    files: &[HarnessFile],
    known_skills: &[&str],
    baseline: &HashSet<String>,
    resolver: &Resolver,
) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();

    for file in files {
        let lines: Vec<&str> = file.content.split('\n').collect();

        // RULE 1 + 2 — a packet's `## Skills` must name real skills, at most three of them.
        if is_packet(&file.path) {
            if let Some(skills) = headings_in(&file.content)
                .into_iter()
                .find(|heading| skills_heading_pattern().is_match(&heading.name))
            {
                let tokens: Vec<String> = skills
                    .body
                    .to_lowercase()
                    .split_whitespace()
                    .flat_map(|token| token.split(','))
                    .map(|token| token.trim_start_matches(['-', '*']).trim().to_string())
                    .filter(|token| !token.is_empty())
                    .collect();
                let unknown: Vec<&str> = tokens
                    .iter()
                    .filter(|token| !known_skills.contains(&token.as_str()))
                    .map(String::as_str)
                    .collect();
                if !unknown.is_empty() {
                    findings.push(finding(
                        Level::Fail,
                        "skills-unknown",
                        &file.path,
                        Some(skills.line),
                        format!(
                            "names skill(s) that do not exist: {} (known: {})",
                            unknown.join(", "),
                            known_skills.join(", ")
                        ),
                    ));
                }
                if tokens.len() > MAX_SKILLS_PER_PACKET {
                    findings.push(finding(
                        Level::Fail,
                        "skills-too-many",
                        &file.path,
                        Some(skills.line),
                        format!(
                            "lists {} skills; the cap is {MAX_SKILLS_PER_PACKET}",
                            tokens.len()
                        ),
                    ));
                }
            }
        }

        // RULE 3 — every dated MEMORY.md ENTRY carries a date, so an undated fact cannot hide in the
        // log. Scoped to entry-style bullets: the file also holds plain bullets that are section
        // content, and demanding a date from those is how a lint becomes noise.
        if memory_path_pattern().is_match(&file.path) {
            for (index, line) in lines.iter().enumerate() {
                if !memory_entry_pattern().is_match(line) {
                    continue;
                }
                let head: String = line.chars().take(80).collect();
                if !date_pattern().is_match(&head) {
                    findings.push(finding(
                        Level::Fail,
                        "memory-entry-undated",
                        &file.path,
                        Some(index + 1),
                        "entry has no YYYY-MM-DD prefix in its first 80 characters".to_string(),
                    ));
                }
            }
        }

        // RULE 4 — no secret-shaped token, anywhere in the harness.
        for (index, line) in lines.iter().enumerate() {
            for shape in shapes_in_line(line) {
                findings.push(finding(
                    Level::Fail,
                    "secret-shape",
                    &file.path,
                    Some(index + 1),
                    format!("looks like a {shape}"),
                ));
            }
        }

        // RULE 7 — a MAP must only point at files that exist.
        if is_map_page(&file.path) {
            for cited in cited_repo_paths(&file.content) {
                if resolver.path_exists(&cited) {
                    continue;
                }
                findings.push(finding(
                    Level::Fail,
                    "map-cites-missing-path",
                    &file.path,
                    None,
                    format!(
                        "cites `{cited}`, which does not exist — the map has drifted from the code"
                    ),
                ));
            }
        }

        // RULE 5 — a role that may not commit must not be told to (that is an AGENTS.md Never).
        //
        // The prohibition can be a SECTION HEADER rather than a word on the line: `AGENTS.md` lists
        // "Keep a git commit as Scout, Assay, or Inspector." under a `Never` heading, so the cue is up
        // to two dozen lines above the item. Reading only a +/-3 line window flagged the rule statement
        // itself — a false positive that would have taught everyone to ignore the gate.
        if !prohibition_for(&lines, 0) {
            for index in 0..lines.len() {
                if prohibition_for(&lines, index) {
                    continue;
                }
                let window = format!(
                    "{} {} {}",
                    lines[index],
                    lines.get(index + 1).unwrap_or(&""),
                    lines.get(index + 2).unwrap_or(&"")
                );
                if !is_commit_directive(&window) {
                    continue;
                }
                findings.push(finding(
                    Level::Fail,
                    "non-builder-commit-instruction",
                    &file.path,
                    Some(index + 1),
                    "instructs Scout/Assay/Inspector to commit; only the Builder role commits"
                        .to_string(),
                ));
            }
        }
    }

    findings.extend(rules_over_the_corpus(files, known_skills, resolver));

    // Debt recorded at the baseline is reported, not blocking.
    for item in findings.iter_mut() {
        if item.level != Level::Fail || !baseline.contains(&baseline_key(item)) {
            continue;
        }
        item.level = Level::Warn;
        item.baselined = true;
        item.message = format!("pre-existing (baselined): {}", item.message);
    }
    findings
}

/// `path::rule`, without line numbers, so editing a file above a known finding does not "un-baseline" it.
pub fn baseline_key(finding: &Finding) -> String {
    format!("{}::{}", finding.file, finding.rule)
}

/// Rules that read the corpus as a whole rather than one file at a time: the skill-directory drift, the
/// generated artefacts (manifests, vendor blocks), the evidence ranges and the decision mirrors.
fn rules_over_the_corpus(
    files: &[HarnessFile],
    known_skills: &[&str],
    resolver: &Resolver,
) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();

    // RULE 6 (warn) — the skill list and the skill files have drifted apart. Discovered 2026-09-15:
    // `docs/agent/skills/` holds packs that are not in KNOWN_SKILLS while some known ids have no file.
    // Reported, not failed: it pre-dates this lint, and a gate that blocks on day one gets switched off
    // on day one.
    let skill_files: Vec<String> = files
        .iter()
        .filter(|file| skill_file_pattern().is_match(&file.path))
        .map(|file| {
            file.path
                .rsplit('/')
                .next()
                .unwrap_or("")
                .trim_end_matches(".md")
                .to_string()
        })
        .filter(|name| name != "README")
        .collect();
    for name in &skill_files {
        if known_skills.contains(&name.as_str()) {
            continue;
        }
        findings.push(finding(
            Level::Warn,
            "skill-file-not-in-known",
            &format!("docs/agent/skills/{name}.md"),
            None,
            "skill file exists but is not in KNOWN_SKILLS, so no packet can load it".to_string(),
        ));
    }
    for skill in known_skills {
        if skill_files.iter().any(|name| name == skill) {
            continue;
        }
        findings.push(finding(
            Level::Warn,
            "known-skill-has-no-file",
            &format!("docs/agent/skills/{skill}.md"),
            None,
            format!("KNOWN_SKILLS lists \"{skill}\" but no pack file exists to load"),
        ));
    }

    // RULE 8 — a generated scope manifest must not point at a path that is gone. The value of a
    // generated index is that it cannot lie, and a row for a deleted file is a lie the reader cannot
    // see. Parse the rows rather than re-deriving them: the manifest CLI owns the ranking, the lint
    // owns the claim.
    for file in files {
        if !manifest_path_pattern().is_match(&file.path) {
            continue;
        }
        for line in file.content.split('\n') {
            let Some(captures) = manifest_row_pattern().captures(line) else {
                continue;
            };
            let cited = &captures[1];
            if !matches!(resolver.resolve(cited), Resolution::None) {
                continue;
            }
            findings.push(finding(
                Level::Fail,
                "manifest-cites-missing-path",
                &file.path,
                None,
                format!(
                    "row `{cited}` resolves to no file — regenerate: pnpm forge:manifest <scope>, or fix the reference in the packet"
                ),
            ));
        }
    }

    // RULE 9 — the two halves of the vendor-block mechanism: a block that drifted from a fresh render,
    // and a guardrail whose backing sentence is no longer in `AGENTS.md`. The second one is the whole
    // point of replicating a rule: a generated file must never assert something the handbook stopped
    // saying.
    if let Some(agents) = files.iter().find(|file| file.path == "AGENTS.md") {
        for guardrail in orphaned_guardrails(&agents.content, &crate::forge::vendor_block::GUARDRAILS) {
            findings.push(finding(
                Level::Fail,
                "guardrail-anchor-missing",
                &agents.path,
                None,
                format!(
                    "AGENTS.md no longer contains \"{}\", so the generated block still asserts it",
                    guardrail.anchored_by
                ),
            ));
        }
        let block = crate::forge::vendor_block::render_block();
        for file in files {
            if !MANAGED_VENDOR_FILES.contains(&file.path.as_str()) || !has_block(&file.content) {
                continue;
            }
            if !block_drifted(&file.content, &block) {
                continue;
            }
            findings.push(finding(
                Level::Fail,
                "vendor-block-drift",
                &file.path,
                None,
                "the generated block differs from a fresh render — run: pnpm forge:sync-agents"
                    .to_string(),
            ));
        }
    }

    // RULE 10 — evidence cites a file AND a range; a range past the end of the file is a stale claim
    // that reads as precision. Applied to packets and maps, the two places we ask a reader to verify a
    // statement against a specific line.
    for file in files {
        if !is_packet(&file.path) && !is_map_page(&file.path) {
            continue;
        }
        for citation in line_citations(&file.content) {
            let resolved = match resolver.resolve(&citation.path) {
                Resolution::Ambiguous => continue, // two files share the name; not this gate's call
                Resolution::None => {
                    findings.push(finding(
                        Level::Fail,
                        "evidence-cites-missing-file",
                        &file.path,
                        None,
                        format!(
                            "cites `{}` but no file named `{}` exists",
                            citation.raw, citation.path
                        ),
                    ));
                    continue;
                }
                Resolution::File(path) => path,
            };
            let Ok(content) = fs::read_to_string(resolver.root().join(&resolved)) else {
                continue;
            };
            let line_count = count_lines(&content);
            if citation.end > line_count {
                findings.push(finding(
                    Level::Fail,
                    "evidence-line-past-eof",
                    &file.path,
                    None,
                    format!(
                        "cites `{}` but `{resolved}` has {line_count} lines",
                        citation.raw
                    ),
                ));
            }
        }
    }

    // RULE 11 (warn) — a skill pack is loadable knowledge, so it should anchor to the code it
    // describes. Warning rather than failing is the same day-one decision as rule 6, and the count is
    // the number worth watching.
    for file in files {
        if !skill_file_pattern().is_match(&file.path) || file.path.ends_with("README.md") {
            continue;
        }
        if cited_repo_paths(&file.content)
            .iter()
            .any(|path| resolver.path_exists(path))
        {
            continue;
        }
        findings.push(finding(
            Level::Warn,
            "skill-not-anchored",
            &file.path,
            None,
            "names no repo path that exists — the pack describes code it does not point at"
                .to_string(),
        ));
    }

    // RULE 12 — a decision mirror must still LOOK like a decision: the filename is the key, the status
    // is one of three, and the statement is one sentence. Whether the file matches its ROW needs the
    // database, which this lint deliberately does not touch. What can be checked offline is the shape,
    // and shape is what a hand edit breaks first.
    for file in files {
        if !decision_path_pattern().is_match(&file.path) {
            continue;
        }
        let file_key = file
            .path
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md")
            .to_string();
        let parsed = parse_decision_file(&file.content, &file_key);
        if parsed.key != file_key {
            findings.push(finding(
                Level::Fail,
                "decision-file-key-mismatch",
                &file.path,
                None,
                format!(
                    "title says \"{}\" but the filename says \"{file_key}\" — the filename IS the key",
                    parsed.key
                ),
            ));
        }
        if !crate::forge::decision::is_valid_key(&parsed.key) {
            findings.push(finding(
                Level::Fail,
                "decision-file-key-mismatch",
                &file.path,
                None,
                format!("\"{}\" is not a lowercase slug", parsed.key),
            ));
        }
        if !DECISION_STATUSES.contains(&parsed.status.as_str()) {
            findings.push(finding(
                Level::Fail,
                "decision-file-status",
                &file.path,
                None,
                format!(
                    "status \"{}\" is not one of {}",
                    parsed.status,
                    DECISION_STATUSES.join(", ")
                ),
            ));
        }
        for problem in validate_statement(&parsed.statement) {
            findings.push(finding(
                Level::Fail,
                "decision-file-statement",
                &file.path,
                None,
                problem,
            ));
        }
        if !file.content.contains("GENERATED from forge_decision") {
            findings.push(finding(
                Level::Warn,
                "decision-file-not-generated",
                &file.path,
                None,
                "no generated-by marker: hand-written decisions belong in the table, not the mirror"
                    .to_string(),
            ));
        }
    }

    findings
}

/// The harness files, and only the harness files.
pub fn load_harness_files(root: &Path) -> Vec<HarnessFile> {
    fn add(root: &Path, path: &Path, out: &mut Vec<HarnessFile>) {
        let Ok(content) = fs::read_to_string(path) else {
            return; // a missing optional file is not a finding
        };
        let Ok(relative) = path.strip_prefix(root) else {
            return;
        };
        out.push(HarnessFile {
            path: relative.to_string_lossy().to_string(),
            content,
        });
    }
    fn add_dir(
        root: &Path,
        dir: &Path,
        filter: &dyn Fn(&str) -> bool,
        out: &mut Vec<HarnessFile>,
    ) {
        let Ok(entries) = fs::read_dir(dir) else {
            return; // absent directory
        };
        let mut names: Vec<String> = entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| filter(name))
            .collect();
        names.sort();
        for name in names {
            add(root, &dir.join(name), out);
        }
    }

    let mut out: Vec<HarnessFile> = Vec::new();
    add(root, &root.join("AGENTS.md"), &mut out);
    add(root, &root.join("docs/agent/MEMORY.md"), &mut out);
    add_dir(
        root,
        &root.join("docs/agent/packets"),
        &|name: &str| name.ends_with(".md"),
        &mut out,
    );
    add_dir(
        root,
        &root.join("docs/agent/skills"),
        &|name: &str| name.ends_with(".md"),
        &mut out,
    );
    // The MAP pages: they claim to point at real files, so they are scanned (rule 7 checks the claim).
    add_dir(
        root,
        &root.join("docs/agent"),
        &|name: &str| name.starts_with("ORIENTATION") || name.starts_with("MAP-"),
        &mut out,
    );
    add_dir(
        root,
        &root.join("agent-runtime"),
        &|name: &str| name.ends_with(".ts") && !name.ends_with(".test.ts"),
        &mut out,
    );
    // Generated scope manifests: each row claims a path on disk (rule 8 checks the claim).
    add_dir(
        root,
        &root.join("docs/agent/manifest"),
        &|name: &str| name.ends_with(".md"),
        &mut out,
    );
    // Decision mirrors (migration 180): scanned for STRUCTURE only — whether each file still matches
    // its row needs the database, and that check lives in `pnpm forge:decision check`.
    add_dir(
        root,
        &root.join("docs/agent/decisions"),
        &|name: &str| name.ends_with(".md"),
        &mut out,
    );
    // Vendor pointer files that carry a generated block (rule 9 checks it has not drifted).
    for name in MANAGED_VENDOR_FILES {
        add(root, &root.join(name), &mut out);
    }

    out
}

/// The recorded debt, if any. An ABSENT file means "no baseline", which fails on everything — that is a
/// stated fact, not a silent one. An UNREADABLE file is an error the caller turns into a finding: the
/// TypeScript caught the parse error and returned "no debt", so a corrupt file quietly promoted
/// recorded debt back into violations.
pub fn load_baseline(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join(BASELINE_PATH);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{BASELINE_PATH} could not be read: {error}")),
    };
    let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "{BASELINE_PATH} is not valid JSON ({error}) — the recorded debt cannot be read, so every \
             baselined finding would fail. Fix the file: it must be one JSON object with a `findings` array."
        )
    })?;
    Ok(parsed
        .get("findings")
        .and_then(serde_json::Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter_map(|key| key.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}


/// The CLI. Exit is 0 unless `--strict` is passed and something failed: a doc gate must not be able to
/// stop a release, which is why the default is "reported, not blocking".
pub fn run(args: &[String]) -> Result<u8, Failure> {
    let mut strict = false;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--strict" => strict = true,
            "--format" => {
                json = args.get(index + 1).map(String::as_str) == Some("json");
                index += 1;
            }
            other => {
                return Err(Failure::usage(format!(
                    "unknown argument `{other}`; usage: forge harness-lint [--strict] [--format json]"
                )))
            }
        }
        index += 1;
    }

    let root = crate::forge::repo_root();
    let files = load_harness_files(&root);
    let resolver = Resolver::new(root.clone());
    let mut findings: Vec<Finding> = Vec::new();
    let baseline: HashSet<String> = match load_baseline(&root) {
        Ok(keys) => keys.into_iter().collect(),
        Err(message) => {
            findings.push(finding(
                Level::Fail,
                "baseline-unreadable",
                BASELINE_PATH,
                None,
                message,
            ));
            HashSet::new()
        }
    };
    findings.extend(lint_harness(&files, &KNOWN_SKILLS, &baseline, &resolver));

    let failures = findings
        .iter()
        .filter(|finding| finding.level == Level::Fail)
        .count();
    let warnings = findings.len() - failures;
    let baselined = findings.iter().filter(|finding| finding.baselined).count();

    if json {
        let payload = serde_json::json!({
            "filesScanned": files.len(),
            "failures": failures,
            "warnings": warnings,
            "baselined": baselined,
            "findings": findings
                .iter()
                .map(|finding| serde_json::json!({
                    "level": finding.level.as_str(),
                    "rule": finding.rule,
                    "file": finding.file,
                    "line": finding.line,
                    "message": finding.message,
                    "baselined": finding.baselined,
                }))
                .collect::<Vec<_>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(exit_code(strict, failures));
    }

    for finding in &findings {
        let location = match finding.line {
            Some(line) => format!("{}:{}", finding.file, line),
            None => finding.file.clone(),
        };
        println!(
            "{}  {:<32} {}\n      {}",
            if finding.level == Level::Fail {
                "FAIL"
            } else {
                "warn"
            },
            finding.rule,
            location,
            finding.message
        );
    }
    println!(
        "\nforge:packet-lint — {failures} failure(s), {warnings} warning(s) ({baselined} baselined), {} harness file(s) scanned{}",
        files.len(),
        if strict && failures > 0 {
            ""
        } else {
            " — reported, not blocking (use --strict to block)"
        }
    );
    Ok(exit_code(strict, failures))
}

fn exit_code(strict: bool, failures: usize) -> u8 {
    if strict && failures > 0 {
        1
    } else {
        0
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::vendor_block::{render_block, upsert_block, GUARDRAILS};
    use std::path::{Path, PathBuf};

    // ---------------------------------------------------------------------------------------------
    // The lint's own tests, ported from the retired `scripts/forge-packet-lint.test.ts`. Each rule
    // gets a fixture that MUST fail and one that must pass, because a gate nobody has seen fail is
    // indistinguishable from decoration — the exact criticism that produced it.
    // ---------------------------------------------------------------------------------------------

    fn packet(body: &str) -> HarnessFile {
        HarnessFile {
            path: "docs/agent/packets/TEST-01.md".to_string(),
            content: format!("# TEST-01 — a fixture\n\n{body}\n"),
        }
    }

    fn fixture_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("forge-harness-lint-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture root");
        root
    }

    fn write_file(root: &Path, path: &str, content: &str) {
        let full = root.join(path);
        fs::create_dir_all(full.parent().expect("fixture parent")).expect("fixture parent");
        fs::write(full, content).expect("fixture file");
    }

    fn lint_with(files: &[HarnessFile], known: &[&str], baseline: &[&str]) -> Vec<Finding> {
        let root = fixture_root("no-paths");
        let keys: HashSet<String> = baseline.iter().map(|key| key.to_string()).collect();
        lint_harness(files, known, &keys, &Resolver::new(root))
    }

    fn rules(findings: &[Finding], level: Level) -> Vec<&'static str> {
        findings
            .iter()
            .filter(|finding| finding.level == level)
            .map(|finding| finding.rule)
            .collect()
    }

    /// A handbook that still contains every anchor, so rule 9's anchor half stays quiet in fixtures
    /// that are about something else.
    fn fixture_handbook() -> String {
        let mut handbook = String::from("# A handbook\n\n");
        for guardrail in GUARDRAILS {
            handbook.push_str(guardrail.anchored_by);
            handbook.push('\n');
        }
        handbook
    }

    #[test]
    fn a_packet_naming_a_skill_that_does_not_exist_fails() {
        let findings = lint_with(
            &[packet("## Skills\n\nneon, quantum-neural-wiki\n")],
            &["neon", "forms"],
            &[],
        );
        let hit = findings
            .iter()
            .find(|finding| finding.rule == "skills-unknown")
            .expect("expected an unknown-skill finding");
        assert_eq!(hit.level, Level::Fail);
        assert!(hit.message.contains("quantum-neural-wiki"), "{}", hit.message);
    }

    #[test]
    fn a_packet_listing_more_than_three_skills_fails() {
        let findings = lint_with(
            &[packet("## Skills\n\nneon, forms, workflow, ui\n")],
            &["neon", "forms", "workflow", "ui", "planner"],
            &[],
        );
        let hit = findings
            .iter()
            .find(|finding| finding.rule == "skills-too-many")
            .expect("expected a too-many-skills finding");
        assert!(
            hit.message.contains("lists 4 skills; the cap is 3"),
            "{}",
            hit.message
        );
    }

    #[test]
    fn a_packet_with_three_real_skills_passes() {
        let findings = lint_with(
            &[packet("## Skills\n\nneon, forms, planner\n")],
            &["neon", "forms", "planner"],
            &[],
        );
        assert!(rules(&findings, Level::Fail).is_empty());
    }

    #[test]
    fn a_memory_entry_with_no_date_fails_and_a_dated_one_passes() {
        let undated = lint_with(
            &[HarnessFile {
                path: "docs/agent/MEMORY.md".to_string(),
                content:
                    "- **something we learned the hard way and must never forget because it cost hours**\n"
                        .to_string(),
            }],
            &KNOWN_SKILLS,
            &[],
        );
        assert!(undated
            .iter()
            .any(|finding| finding.rule == "memory-entry-undated"));

        let dated = lint_with(
            &[HarnessFile {
                path: "docs/agent/MEMORY.md".to_string(),
                content: "- **2026-09-15 (a dated fact):** something we learned and can now find again\n"
                    .to_string(),
            }],
            &KNOWN_SKILLS,
            &[],
        );
        assert!(!dated
            .iter()
            .any(|finding| finding.rule == "memory-entry-undated"));
    }

    #[test]
    fn secret_shaped_tokens_fail_in_any_harness_file() {
        let findings = lint_with(
            &[
                HarnessFile {
                    path: "AGENTS.md".to_string(),
                    content: "token: sk-abcdefghijklmnopqrstuvwx\n".to_string(),
                },
                HarnessFile {
                    path: "docs/agent/packets/TEST-02.md".to_string(),
                    content: "db: postgres://user:pw@host/db\n".to_string(),
                },
            ],
            &KNOWN_SKILLS,
            &[],
        );
        assert_eq!(
            findings
                .iter()
                .filter(|finding| finding.rule == "secret-shape")
                .count(),
            2
        );
    }

    #[test]
    fn telling_inspector_to_commit_fails_and_forbidding_it_does_not() {
        let instruction = lint_with(
            &[packet("## Loop\n\nInspector: verify the diff, then git commit the fixes.\n")],
            &KNOWN_SKILLS,
            &[],
        );
        assert!(instruction
            .iter()
            .any(|finding| finding.rule == "non-builder-commit-instruction"));

        let prohibition = lint_with(
            &[packet("## Loop\n\nNever create a git commit as Scout, Assay or Inspector.\n")],
            &KNOWN_SKILLS,
            &[],
        );
        assert!(!prohibition
            .iter()
            .any(|finding| finding.rule == "non-builder-commit-instruction"));
    }

    #[test]
    fn a_never_heading_protects_its_list_items() {
        // `AGENTS.md` states the same rule as a `Never` list item, and the cue is the heading, not a
        // word on the line. Reading only a three-line window flagged the rule statement itself.
        let findings = lint_with(
            &[
                HarnessFile {
                    path: "AGENTS.md".to_string(),
                    content: "## Never\n\n- Keep a git commit as Scout, Assay, or Inspector.\n"
                        .to_string(),
                },
            ],
            &KNOWN_SKILLS,
            &[],
        );
        assert!(!findings
            .iter()
            .any(|finding| finding.rule == "non-builder-commit-instruction"));
    }

    #[test]
    fn the_skills_directory_drift_is_a_warning_never_a_failure() {
        let findings = lint_with(
            &[
                HarnessFile {
                    path: "docs/agent/skills/neon.md".to_string(),
                    content: "# neon\n".to_string(),
                },
                HarnessFile {
                    path: "docs/agent/skills/semgrep.md".to_string(),
                    content: "# semgrep\n".to_string(),
                },
            ],
            &["neon", "ui"],
            &[],
        );
        let mut warns = rules(&findings, Level::Warn);
        warns.sort_unstable();
        assert_eq!(
            warns,
            vec![
                "known-skill-has-no-file",
                "skill-file-not-in-known",
                "skill-not-anchored",
                "skill-not-anchored",
            ]
        );
        assert!(rules(&findings, Level::Fail).is_empty());
    }

    #[test]
    fn a_baselined_failure_is_reported_not_blocked() {
        let files = [packet("## Skills\n\nneon, quantum-neural-wiki\n")];
        let failing = lint_with(&files, &["neon"], &[]);
        assert_eq!(
            failing
                .iter()
                .filter(|finding| finding.level == Level::Fail)
                .count(),
            1
        );

        let baselined = lint_with(&files, &["neon"], &["docs/agent/packets/TEST-01.md::skills-unknown"]);
        assert!(rules(&baselined, Level::Fail).is_empty());
        let hit = baselined
            .iter()
            .find(|finding| finding.rule == "skills-unknown")
            .expect("the baselined finding is still reported");
        assert!(hit.baselined);
        assert!(hit.message.starts_with("pre-existing (baselined):"));
    }

    #[test]
    fn a_map_page_citing_a_missing_file_fails_and_real_paths_pass() {
        let root = fixture_root("map");
        write_file(&root, "legacy/services/property/property-service.ts", "// real\n");
        let resolver = Resolver::new(root.clone());
        let baseline = HashSet::new();

        let missing = lint_harness(
            &[HarnessFile {
                path: "docs/agent/MAP-test.md".to_string(),
                content: "Open `legacy/services/ghost/ghost-service.ts` first.\n".to_string(),
            }],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        let hit = missing
            .iter()
            .find(|finding| finding.rule == "map-cites-missing-path")
            .expect("expected the map rule to catch a path that does not exist");
        assert!(hit.message.contains("services/ghost"), "{}", hit.message);

        let real = lint_harness(
            &[HarnessFile {
                path: "docs/agent/MAP-test.md".to_string(),
                content: "Open `legacy/services/property/property-service.ts`.\n".to_string(),
            }],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        assert!(!real
            .iter()
            .any(|finding| finding.rule == "map-cites-missing-path"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_manifest_row_naming_a_path_that_is_not_on_disk_fails() {
        let root = fixture_root("manifest");
        write_file(&root, "rust/cli/src/forge/lint.rs", "// real\n");
        let findings = lint_harness(
            &[HarnessFile {
                path: "docs/agent/manifest/TEST-01.md".to_string(),
                content: "- `rust/cli/src/forge/lint.rs` — cited · cited by TEST-01\n\
                          - `legacy/services/ghost/ghost.ts` — cited · cited by TEST-01\n"
                    .to_string(),
            }],
            &KNOWN_SKILLS,
            &HashSet::new(),
            &Resolver::new(root.clone()),
        );
        let hits: Vec<&Finding> = findings
            .iter()
            .filter(|finding| finding.rule == "manifest-cites-missing-path")
            .collect();
        assert_eq!(hits.len(), 1, "{findings:?}");
        assert!(hits[0].message.contains("services/ghost/ghost.ts"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_vendor_block_that_drifted_fails_and_a_fresh_one_does_not() {
        let root = fixture_root("vendor-block");
        let block = render_block();
        let fresh = upsert_block(Some("# Adapter\n"), &block);
        let agents = HarnessFile {
            path: "AGENTS.md".to_string(),
            content: fixture_handbook(),
        };
        let resolver = Resolver::new(root.clone());
        let baseline = HashSet::new();

        let ok = lint_harness(
            &[
                agents.clone(),
                HarnessFile {
                    path: "CLAUDE.md".to_string(),
                    content: fresh.clone(),
                },
            ],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        assert!(!ok.iter().any(|finding| finding.rule == "vendor-block-drift"));

        let drifted = lint_harness(
            &[
                agents,
                HarnessFile {
                    path: "CLAUDE.md".to_string(),
                    content: fresh.replace("Only the Builder role commits", "Anyone may commit"),
                },
            ],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        assert_eq!(
            drifted
                .iter()
                .filter(|finding| finding.rule == "vendor-block-drift")
                .count(),
            1
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_guardrail_whose_handbook_sentence_is_gone_fails() {
        let root = fixture_root("orphan");
        let handbook = fixture_handbook().replace("Commit secrets or `.env.local`", "Commit anything");
        let findings = lint_harness(
            &[HarnessFile {
                path: "AGENTS.md".to_string(),
                content: handbook,
            }],
            &KNOWN_SKILLS,
            &HashSet::new(),
            &Resolver::new(root.clone()),
        );
        let hit = findings
            .iter()
            .find(|finding| finding.rule == "guardrail-anchor-missing")
            .expect("expected the anchor rule to catch a removed handbook sentence");
        assert!(hit.message.contains("Commit secrets"), "{}", hit.message);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_citation_past_the_end_of_a_file_fails_and_a_real_range_passes() {
        let root = fixture_root("citation");
        write_file(&root, "rust/cli/src/forge/citations.rs", "one\ntwo\nthree\n");
        let resolver = Resolver::new(root.clone());
        let baseline = HashSet::new();

        let past = lint_harness(
            &[packet("## Context refs\n\nsee `rust/cli/src/forge/citations.rs:99999`\n")],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        assert!(past
            .iter()
            .any(|finding| finding.rule == "evidence-line-past-eof"));

        let real = lint_harness(
            &[packet("## Context refs\n\nsee `rust/cli/src/forge/citations.rs:1-3`\n")],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        assert!(!real
            .iter()
            .any(|finding| finding.rule == "evidence-line-past-eof"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_citation_to_a_file_that_exists_nowhere_fails_and_a_bare_name_shorthand_does_not() {
        let root = fixture_root("shorthand");
        write_file(&root, "rust/cli/src/forge/citations.rs", "one\ntwo\nthree\n");
        let resolver = Resolver::new(root.clone());
        let baseline = HashSet::new();

        let gone = lint_harness(
            &[packet("## Context refs\n\nsee `engine-that-never-was.ts:1966`\n")],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        assert!(gone
            .iter()
            .any(|finding| finding.rule == "evidence-cites-missing-file"));

        // Shorthand convention: 15 of the 16 hits on the first live run were this, which is why the
        // resolver exists. `citations.rs` exists once here, so its range is verifiable.
        let shorthand = lint_harness(
            &[packet("## Context refs\n\nsee `citations.rs:1-3`\n")],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        assert!(rules(&shorthand, Level::Fail).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_hand_written_mirror_is_a_warning_not_a_failure() {
        let findings = lint_with(
            &[HarnessFile {
                path: "docs/agent/decisions/x.md".to_string(),
                content: "# x\n\n- status: active\n\nOne sentence is true.\n".to_string(),
            }],
            &KNOWN_SKILLS,
            &[],
        );
        let hit = findings
            .iter()
            .find(|finding| finding.rule == "decision-file-not-generated")
            .expect("expected the generated-by warning");
        assert_eq!(hit.level, Level::Warn);
    }

    #[test]
    fn the_baseline_reader_separates_absent_from_unreadable() {
        let root = fixture_root("baseline");
        assert_eq!(
            load_baseline(&root).expect("absent is not an error"),
            Vec::<String>::new()
        );

        write_file(
            &root,
            BASELINE_PATH,
            "{\n  \"note\": \"debt\",\n  \"findings\": [\"a.md::some-rule\"]\n}\n",
        );
        assert_eq!(
            load_baseline(&root).expect("valid JSON"),
            vec!["a.md::some-rule".to_string()]
        );

        // The real file was found in exactly this state on 2026-09-27: two JSON objects concatenated,
        // which the TypeScript read as "no debt at all" and silently un-baselined recorded debt.
        write_file(
            &root,
            BASELINE_PATH,
            "{\n  \"findings\": []\n}\n{\n  \"findings\": [\"a.md::some-rule\"]\n}\n",
        );
        let error = load_baseline(&root).expect_err("malformed JSON must be reported");
        assert!(error.contains("not valid JSON"), "{error}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_harness_files_scans_the_harness_and_only_the_harness() {
        let root = fixture_root("load");
        write_file(&root, "AGENTS.md", "# handbook\n");
        write_file(&root, "docs/agent/MEMORY.md", "# memory\n");
        write_file(&root, "docs/agent/packets/TEST-01.md", "# packet\n");
        write_file(&root, "docs/agent/MAP-code.md", "# map\n");
        write_file(&root, "docs/agent/ORIENTATION.md", "# orientation\n");
        write_file(&root, "docs/agent/skills/neon.md", "# neon\n");
        write_file(&root, "docs/agent/manifest/TEST-01.md", "# manifest\n");
        write_file(&root, "docs/agent/decisions/x.md", "# x\n");
        write_file(&root, "agent-runtime/skills.ts", "export {}\n");
        write_file(&root, "agent-runtime/skills.test.ts", "// not scanned\n");
        write_file(&root, "rust/cli/src/main.rs", "// not the harness\n");
        write_file(&root, "app/page.tsx", "// not the harness\n");
        let files = load_harness_files(&root);
        let mut paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            vec![
                "AGENTS.md",
                "agent-runtime/skills.ts",
                "docs/agent/MAP-code.md",
                "docs/agent/MEMORY.md",
                "docs/agent/ORIENTATION.md",
                "docs/agent/decisions/x.md",
                "docs/agent/manifest/TEST-01.md",
                "docs/agent/packets/TEST-01.md",
                "docs/agent/skills/neon.md",
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_resolver_reads_a_doc_relative_citation_and_stays_quiet_on_an_ambiguous_one() {
        let root = fixture_root("resolve");
        write_file(&root, "docs/agent/packets/README.md", "# packets\n");
        write_file(&root, "rust/a/thing.rs", "// a\n");
        write_file(&root, "rust/b/thing.rs", "// b\n");
        let resolver = Resolver::new(root.clone());

        assert!(matches!(
            resolver.resolve("packets/README.md"),
            Resolution::File(path) if path == "docs/agent/packets/README.md"
        ));
        assert!(matches!(
            resolver.resolve("rust/a/thing.rs"),
            Resolution::File(path) if path == "rust/a/thing.rs"
        ));
        assert!(matches!(resolver.resolve("thing.rs"), Resolution::Ambiguous));
        assert!(matches!(resolver.resolve("nothing-here.rs"), Resolution::None));
        assert!(resolver.path_exists("rust/a/*.rs"));
        assert!(!resolver.path_exists("rust/a/*.ts"));
        let _ = fs::remove_dir_all(&root);
    }
}
