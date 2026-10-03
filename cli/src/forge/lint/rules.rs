//! Moved from `lint.rs` (move only): rules_over_the_corpus.

#[allow(unused_imports)]
use super::*;

/// Rules that read the corpus as a whole rather than one file at a time: the skill-directory drift, the
/// generated artefacts (manifests, vendor blocks), the evidence ranges and the decision mirrors.
pub(super) fn rules_over_the_corpus(
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
        // The guard-PATH half of the same idea: a `guard:` line that names a test which is not there.
        // `docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md` finding A is why this exists; `forge guard-lint`
        // runs the same check as a blocking gate, and this keeps `pnpm forge:packet-lint` honest too.
        for guard in crate::forge::guard_paths::check(resolver.root(), &agents.content) {
            findings.push(finding(
                Level::Fail,
                guard.rule,
                &agents.path,
                Some(guard.line),
                guard.message,
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
