//! GUARD-PATH LINT — every `guard:` line in the handbook is checked against the tree, not assumed.
//!
//! WHY THIS EXISTS. `docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md` finding A measured that every guardrail
//! path `AGENTS.md` named was dead: eight `guard: workflow_app/tests/*.test.ts` lines survived the deletion
//! of the whole TypeScript application because nothing read them. The handbook claimed nine rules were
//! guarded and not one of the files existed; the same absence let finding B (nine deleted TypeScript paths
//! still called canonical) survive as long as it did. This is the guard-PATH half of the idea the vendor
//! block already had for guardrail SENTENCES (`orphaned_guardrails`, `rust/cli/src/forge/vendor_block.rs`).
//!
//! WHAT IT IS NOT. It is not a test of the rules; it is a test of the CLAIM that the rules are tested. That
//! distinction is the whole value: a path that exists but holds no test still passes a naive check, so a
//! declared path must resolve to a file that actually carries a test (an empty file is not a guard).
//!
//! FOUR FINDINGS, all `fail`:
//!   * `guard-missing` — a rule in the `Never` list declares no guard at all (a silent hole). The
//!     handbook's own `guard: NONE — <reason>` form is how a genuinely un-automatable rule is written down;
//!     silence is not.
//!   * `guard-path-missing` — a `guard: <path>` whose path is not a file on disk.
//!   * `guard-path-not-a-test` — a path that exists but holds no test.
//!   * `guard-none-without-reason` — a bare `guard: NONE` with no reason. The `NONE` form IS the escape
//!     hatch, so it must be a WRITTEN decision: "cannot be automated because X", not one word that
//!     silences the gate. Risk 1 of the story: `NONE` is never for a test that was merely not written yet.
//!
//! A wrong path is worse than a missing one, because it reads as enforced — so a rule whose Rust guard has
//! not landed says `guard: NONE — <why>` rather than pointing at a file that looks plausible.
//!
//! Usage:
//!   cargo run -p cli -- forge guard-lint [--format json]

use std::fs;
use std::path::Path;

use super::Failure;

pub const AGENTS_MD: &str = "AGENTS.md";

/// One `guard:` clause: the line it sits on and the first whitespace-delimited token after the colon.
/// `NONE` is the handbook's written-decision form; anything else is a repo-relative path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardDeclaration {
    pub line: usize,
    pub value: String,
    /// The text after the value token. For `guard: NONE — <reason>` this is `<reason>`, and it is what makes
    /// the escape hatch a written decision rather than a one-word silence.
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardFinding {
    pub rule: &'static str,
    pub line: usize,
    pub reference: String,
    pub message: String,
}

/// Byte offsets of the real `guard:` clauses in a line. A clause is the WORD `guard:`: `safeguard:` and
/// `vanguard:` merely contain those bytes and are prose, so the character before the colon must not be
/// alphanumeric. Without this, "safeguard: against drift" would scrape the token after the colon as a
/// guard PATH and fail the gate on a sentence, and a rule whose only `guard:` sat inside another word
/// would read as guarded when it is not.
fn guard_clause_offsets(line: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for (at, _) in line.match_indices("guard:") {
        let preceded_by_word = line[..at]
            .chars()
            .next_back()
            .map(|c| c.is_alphanumeric())
            .unwrap_or(false);
        if !preceded_by_word {
            out.push(at);
        }
    }
    out
}

/// Every `guard:` clause in the document, with the line it sits on. A `guard: NONE — <reason>` clause
/// yields `NONE`; the reason stays in the document, where a reader needs it and this gate does not.
pub fn guard_declarations(agents_md: &str) -> Vec<GuardDeclaration> {
    let mut out = Vec::new();
    for (index, line) in agents_md.split('\n').enumerate() {
        for at in guard_clause_offsets(line) {
            let rest = line[at + "guard:".len()..].trim();
            let value = rest.split_whitespace().next().unwrap_or("").to_string();
            if value.is_empty() {
                continue;
            }
            let reason = rest[value.len()..].trim().to_string();
            out.push(GuardDeclaration {
                line: index + 1,
                value,
                reason,
            });
        }
    }
    out
}

/// True for the handbook's `Never` heading, however it is spelled: bare (`Never`) or as a heading
/// (`## Never`). The heading is the cue the whole list sits under; the items themselves read as rules.
fn is_never_heading(line: &str) -> bool {
    line.trim()
        .trim_start_matches('#')
        .trim()
        .eq_ignore_ascii_case("never")
}

/// Rules in the `Never` list that declare no guard at all, as `(line number, the rule's first line)`.
/// A rule that names both a path and `NONE` (it cannot happen in one clause) is not this finding's call;
/// a rule with any `guard:` clause is left to the path check.
pub fn rules_without_guard(agents_md: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = agents_md.split('\n').collect();
    let Some(start) = lines.iter().position(|line| is_never_heading(line)) else {
        return Vec::new();
    };

    // The section ends at the next markdown heading (`## Project`, `## Error Capture Obligation`, ...).
    let mut end = lines.len();
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("## ") || trimmed.starts_with("# ") {
            end = index;
            break;
        }
    }

    let mut out: Vec<(usize, String)> = Vec::new();
    let mut item: Option<(usize, String)> = None;
    let flush = |item: &mut Option<(usize, String)>, out: &mut Vec<(usize, String)>| {
        if let Some((at, text)) = item.take() {
            // A real clause, not the bytes inside `safeguard:` — the same boundary the path scan uses.
            if guard_clause_offsets(&text).is_empty() {
                out.push((at, text.trim().to_string()));
            }
        }
    };
    for (index, line) in lines.iter().enumerate().take(end).skip(start) {
        if line.starts_with("- ") {
            flush(&mut item, &mut out);
            item = Some((index + 1, (*line).to_string()));
        } else if let Some((_, text)) = item.as_mut() {
            text.push(' ');
            text.push_str(line);
        }
    }
    flush(&mut item, &mut out);
    out
}

/// A file is a guard when its BODY carries a test. Location and name are not enough: an empty file under
/// `tests/` is a Cargo integration-test target that runs zero tests, and a `*.test.ts` with no `it`/`test`/
/// `describe` body asserts nothing — neither guards a rule (the acceptance criterion is explicit: an empty
/// file is not a guard). So the body is read, whatever the path calls itself.
fn holds_a_test(path: &str, content: &str) -> bool {
    if content.trim().is_empty() {
        return false;
    }
    let normalized = path.replace('\\', "/");
    if normalized.ends_with(".rs") {
        // `#[test]` and `#[cfg(test)]` are the plain markers; `::test]` covers the async/DB wrappers
        // (`#[tokio::test]`, `#[sqlx::test]`), `test_case` the parameterised form.
        return content.contains("#[test]")
            || content.contains("#[cfg(test)]")
            || content.contains("mod tests")
            || content.contains("::test]")
            || content.contains("test_case");
    }
    content.contains("describe(")
        || content.contains("it(")
        || content.contains("test(")
        || content.contains("it.each")
        || content.contains("test.each")
}

/// Check the handbook against the tree rooted at `root`. Deterministic: sorted by the line the finding
/// is about, so the same tree prints the same output on two machines.
pub fn check(root: &Path, agents_md: &str) -> Vec<GuardFinding> {
    let mut findings: Vec<GuardFinding> = Vec::new();

    for (line, text) in rules_without_guard(agents_md) {
        findings.push(GuardFinding {
            rule: "guard-missing",
            line,
            reference: text,
            message: "the rule declares no guard; point it at a real test, or write `guard: NONE — <reason>` \
                      if it genuinely cannot be automated"
                .to_string(),
        });
    }

    for declaration in guard_declarations(agents_md) {
        if declaration.value.eq_ignore_ascii_case("NONE") {
            // `None` is the written decision for a rule no test can hold — but only when it says WHY. A bare
            // `guard: NONE` is a one-word silence, indistinguishable from a test nobody wrote yet, so it fails
            // the same way a missing path does.
            if declaration.reason.is_empty() {
                findings.push(GuardFinding {
                    rule: "guard-none-without-reason",
                    line: declaration.line,
                    reference: declaration.value.clone(),
                    message:
                        "`guard: NONE` carries no reason; write why the rule cannot be automated \
                              (`guard: NONE — <reason>`), or point it at a real test"
                            .to_string(),
                });
            }
            continue;
        }
        let full = root.join(&declaration.value);
        if !full.is_file() {
            findings.push(GuardFinding {
                rule: "guard-path-missing",
                line: declaration.line,
                reference: declaration.value.clone(),
                message: format!(
                    "`guard: {}` names a path that does not exist",
                    declaration.value
                ),
            });
            continue;
        }
        let content = match fs::read_to_string(&full) {
            Ok(content) => content,
            Err(error) => {
                findings.push(GuardFinding {
                    rule: "guard-path-not-a-test",
                    line: declaration.line,
                    reference: declaration.value.clone(),
                    message: format!("`{}` could not be read: {error}", declaration.value),
                });
                continue;
            }
        };
        if !holds_a_test(&declaration.value, &content) {
            findings.push(GuardFinding {
                rule: "guard-path-not-a-test",
                line: declaration.line,
                reference: declaration.value.clone(),
                message: format!(
                    "`{}` exists but holds no test — an empty file is not a guard",
                    declaration.value
                ),
            });
        }
    }

    findings.sort_by_key(|finding| finding.line);
    findings
}

/// The CLI. Unlike the harness lint, this one BLOCKS: the whole point is that a dead guard path cannot
/// survive, and a gate that reports and exits 0 is a gate someone learns to scroll past.
pub fn run(args: &[String]) -> Result<u8, Failure> {
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--format" => {
                json = args.get(index + 1).map(String::as_str) == Some("json");
                index += 1;
            }
            other => {
                return Err(Failure::usage(format!(
                    "unknown argument `{other}`; usage: forge guard-lint [--format json]"
                )))
            }
        }
        index += 1;
    }

    let root = super::repo_root();
    let agents_md = fs::read_to_string(root.join(AGENTS_MD))
        .map_err(|error| Failure::failed(format!("{AGENTS_MD} could not be read: {error}")))?;
    let findings = check(&root, &agents_md);

    if json {
        let payload = serde_json::json!({
            "file": AGENTS_MD,
            "findings": findings
                .iter()
                .map(|finding| serde_json::json!({
                    "rule": finding.rule,
                    "line": finding.line,
                    "reference": finding.reference,
                    "message": finding.message,
                }))
                .collect::<Vec<_>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        for finding in &findings {
            eprintln!(
                "FAIL  {:<22} {}:{}  {}",
                finding.rule, AGENTS_MD, finding.line, finding.message
            );
        }
    }

    if findings.is_empty() {
        println!(
            "forge guard-lint — PASS: every `guard:` line in {AGENTS_MD} resolves to a test that exists, \
             or is a written `NONE`"
        );
        Ok(0)
    } else {
        Err(Failure::failed(format!(
            "forge guard-lint — {} finding(s): the handbook names a guard that is not there",
            findings.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn fixture_root(name: &str) -> PathBuf {
        // One process, many parallel test threads: a per-call counter so two fixtures never share a path.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "forge-guard-lint-{}-{n}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture root");
        root
    }

    fn write_file(root: &Path, path: &str, content: &str) {
        let full = root.join(path);
        fs::create_dir_all(full.parent().expect("fixture parent")).expect("fixture parent");
        fs::write(full, content).expect("fixture file");
    }

    fn handbook(items: &str) -> String {
        format!("# A handbook\n\nNever\n\n{items}\n\n## Project\n\n- a fact\n")
    }

    #[test]
    fn a_guard_path_that_does_not_exist_fails_with_its_line_and_path() {
        let root = fixture_root("missing");
        let findings = check(
            &root,
            &handbook(
                "- Commit secrets or `.env.local`. guard: workflow_app/tests/ghost.test.ts\n",
            ),
        );
        let hit = findings
            .iter()
            .find(|finding| finding.rule == "guard-path-missing")
            .expect("a missing guard path must fail");
        assert_eq!(hit.line, 5, "the guard clause is on line 5: {findings:?}");
        assert_eq!(hit.reference, "workflow_app/tests/ghost.test.ts");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_guard_path_that_holds_no_test_fails() {
        let root = fixture_root("empty");
        write_file(&root, "rust/empty_guard.rs", "// nothing runs here\n");
        let findings = check(&root, &handbook("- A rule. guard: rust/empty_guard.rs\n"));
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule == "guard-path-not-a-test"),
            "a file with no test is not a guard: {findings:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_empty_file_in_a_tests_directory_is_not_a_guard() {
        // Location is not a guard: `tests/` makes Cargo compile the file, but a body with no test runs
        // zero assertions. Criterion 2's "an empty file is not a guard" must hold there too.
        let root = fixture_root("empty-tests-dir");
        write_file(
            &root,
            "rust/core/db/tests/empty_case.rs",
            "// a test target that runs nothing\n",
        );
        let findings = check(
            &root,
            &handbook("- A rule. guard: rust/core/db/tests/empty_case.rs\n"),
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule == "guard-path-not-a-test"),
            "an empty file under tests/ is still not a guard: {findings:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_in_a_tests_directory_that_holds_a_test_passes() {
        let root = fixture_root("real-tests-dir");
        write_file(
            &root,
            "rust/core/db/tests/real_case.rs",
            "#[tokio::test]\nasync fn guarded() {}\n",
        );
        let findings = check(
            &root,
            &handbook("- A rule. guard: rust/core/db/tests/real_case.rs\n"),
        );
        assert!(findings.is_empty(), "{findings:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_guard_path_with_a_test_passes_and_the_none_form_is_a_written_decision() {
        let root = fixture_root("ok");
        write_file(
            &root,
            "rust/real_guard.rs",
            "#[cfg(test)]\nmod tests { #[test] fn guarded() {} }\n",
        );
        let findings = check(
            &root,
            &handbook(
                "- A rule with a real guard. guard: rust/real_guard.rs\n\
                 - A rule no test can hold. guard: NONE — a human decision, not a test\n",
            ),
        );
        assert!(findings.is_empty(), "{findings:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_bare_none_guard_without_a_reason_is_a_silent_hole() {
        // `NONE` is the escape hatch, so it must be a WRITTEN decision. One word is not: it is
        // indistinguishable from a test that was merely not written yet (story risk 1), which is the hole
        // the gate exists to report.
        let root = fixture_root("bare-none");
        let findings = check(&root, &handbook("- A rule. guard: NONE\n"));
        let hit = findings
            .iter()
            .find(|finding| finding.rule == "guard-none-without-reason")
            .expect("a bare `guard: NONE` must fail");
        assert_eq!(hit.line, 5, "{findings:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_word_ending_in_guard_is_not_a_guard_clause() {
        // `safeguard:` contains the bytes `guard:`, but it is prose. A rule whose only "guard:" sits
        // inside another word therefore has NO guard: it must be reported as a silent hole, and no path
        // may be scraped from the word after the colon (which would fail the gate on a sentence).
        let root = fixture_root("safeguard");
        let findings = check(&root, &handbook("- A safeguard: against drift.\n"));
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule == "guard-missing"),
            "prose ending in `guard:` is not a guard clause: {findings:?}"
        );
        assert!(
            !findings
                .iter()
                .any(|finding| finding.rule == "guard-path-missing"),
            "no path may be scraped from a word like `safeguard:`: {findings:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_never_rule_with_no_guard_at_all_is_a_silent_hole() {
        let findings = check(
            &std::env::temp_dir(),
            &handbook("- A rule nobody guarded.\n"),
        );
        let hit = findings
            .iter()
            .find(|finding| finding.rule == "guard-missing")
            .expect("an unannotated Never rule must fail");
        assert_eq!(hit.line, 5, "{findings:?}");
    }

    #[test]
    fn the_real_handbook_guard_paths_resolve_to_tests_on_disk() {
        // This is the acceptance check, run by `cargo test` in CI: the committed AGENTS.md must pass its
        // own gate, or the gate has caught the handbook drifting from the tree it describes.
        let root = super::super::repo_root();
        let agents_md = fs::read_to_string(root.join(AGENTS_MD)).expect("AGENTS.md is committed");
        let findings = check(&root, &agents_md);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
