//! Moved from `lint.rs` (move only): is_packet, is_map_page, Heading, headings_in, prohibition_for, is_commit_directive, finding, baseline_key.

#[allow(unused_imports)]
use super::*;

pub(super) fn is_packet(path: &str) -> bool {
    packet_path_pattern().is_match(path) && !path.ends_with("README.md")
}

pub(super) fn is_map_page(path: &str) -> bool {
    map_page_pattern().is_match(path)
}

/// `## <name>` headings with their line number and the lines under them, up to the next `##`.
pub(super) struct Heading {
    pub(super) name: String,
    pub(super) line: usize,
    pub(super) body: String,
}

pub(super) fn headings_in(content: &str) -> Vec<Heading> {
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
pub(super) fn prohibition_for(lines: &[&str], index: usize) -> bool {
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
pub(super) fn is_commit_directive(window: &str) -> bool {
    if !roles_that_may_not_commit().is_match(window) {
        return false;
    }
    if prohibition().is_match(window) {
        return false;
    }
    let trimmed = window.trim();
    role_opens_instruction().is_match(trimmed) || role_is_handed_the_act().is_match(window)
}

pub(super) fn finding(
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

/// `path::rule`, without line numbers, so editing a file above a known finding does not "un-baseline" it.
pub fn baseline_key(finding: &Finding) -> String {
    format!("{}::{}", finding.file, finding.rule)
}
