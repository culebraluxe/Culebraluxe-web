//! Moved from `lint.rs` (move only): lint_harness.

#[allow(unused_imports)]
use super::*;

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
