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

use crate::forge::citations::{
    cited_repo_paths, count_lines, line_citations, Resolution, Resolver,
};
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
pattern!(
    roles_that_may_not_commit,
    r"(?i)\b(scout|assay|inspector)\b"
);
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
pattern!(
    prohibition_heading_pattern,
    r"(?i)^#*\s*(never|do not|don't|must not|forbidden)\b"
);
pattern!(skills_heading_pattern, r"(?i)^skills$");

mod harness;
mod helpers;
mod load;
mod rules;
#[allow(unused_imports)]
pub use harness::*;
#[allow(unused_imports)]
pub use helpers::*;
#[allow(unused_imports)]
pub use load::*;
#[allow(unused_imports)]
pub use rules::*;

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
        // ONE process, MANY parallel test threads: `std::process::id()` alone is not unique, and the tests that share
        // a fixture name (`no-paths`, via `lint_with`) raced on the same directory - `create_dir_all` came back
        // `AlreadyExists` on a loaded machine and `cargo test --workspace` failed (2026-09-28, on this line in that
        // run). A per-call counter gives every fixture its own directory.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "forge-harness-lint-{}-{n}-{name}",
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
        assert!(
            hit.message.contains("quantum-neural-wiki"),
            "{}",
            hit.message
        );
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
                content:
                    "- **2026-09-15 (a dated fact):** something we learned and can now find again\n"
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
            &[packet(
                "## Loop\n\nInspector: verify the diff, then git commit the fixes.\n",
            )],
            &KNOWN_SKILLS,
            &[],
        );
        assert!(instruction
            .iter()
            .any(|finding| finding.rule == "non-builder-commit-instruction"));

        let prohibition = lint_with(
            &[packet(
                "## Loop\n\nNever create a git commit as Scout, Assay or Inspector.\n",
            )],
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
            &[HarnessFile {
                path: "AGENTS.md".to_string(),
                content: "## Never\n\n- Keep a git commit as Scout, Assay, or Inspector.\n"
                    .to_string(),
            }],
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

        let baselined = lint_with(
            &files,
            &["neon"],
            &["docs/agent/packets/TEST-01.md::skills-unknown"],
        );
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
        write_file(
            &root,
            "legacy/services/property/property-service.ts",
            "// real\n",
        );
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
        write_file(&root, "cli/src/forge/lint.rs", "// real\n");
        let findings = lint_harness(
            &[HarnessFile {
                path: "docs/agent/manifest/TEST-01.md".to_string(),
                content: "- `cli/src/forge/lint.rs` — cited · cited by TEST-01\n\
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
        assert!(!ok
            .iter()
            .any(|finding| finding.rule == "vendor-block-drift"));

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
        let handbook =
            fixture_handbook().replace("Commit secrets or `.env.local`", "Commit anything");
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
        write_file(&root, "cli/src/forge/citations.rs", "one\ntwo\nthree\n");
        let resolver = Resolver::new(root.clone());
        let baseline = HashSet::new();

        let past = lint_harness(
            &[packet(
                "## Context refs\n\nsee `cli/src/forge/citations.rs:99999`\n",
            )],
            &KNOWN_SKILLS,
            &baseline,
            &resolver,
        );
        assert!(past
            .iter()
            .any(|finding| finding.rule == "evidence-line-past-eof"));

        let real = lint_harness(
            &[packet(
                "## Context refs\n\nsee `cli/src/forge/citations.rs:1-3`\n",
            )],
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
        write_file(&root, "cli/src/forge/citations.rs", "one\ntwo\nthree\n");
        let resolver = Resolver::new(root.clone());
        let baseline = HashSet::new();

        let gone = lint_harness(
            &[packet(
                "## Context refs\n\nsee `engine-that-never-was.ts:1966`\n",
            )],
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
        write_file(&root, "cli/src/main.rs", "// not the harness\n");
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
        write_file(&root, "web/a/thing.rs", "// a\n");
        write_file(&root, "web/b/thing.rs", "// b\n");
        let resolver = Resolver::new(root.clone());

        assert!(matches!(
            resolver.resolve("packets/README.md"),
            Resolution::File(path) if path == "docs/agent/packets/README.md"
        ));
        assert!(matches!(
            resolver.resolve("web/a/thing.rs"),
            Resolution::File(path) if path == "web/a/thing.rs"
        ));
        assert!(matches!(
            resolver.resolve("thing.rs"),
            Resolution::Ambiguous
        ));
        assert!(matches!(
            resolver.resolve("nothing-here.rs"),
            Resolution::None
        ));
        assert!(resolver.path_exists("web/a/*.rs"));
        assert!(!resolver.path_exists("web/a/*.ts"));
        let _ = fs::remove_dir_all(&root);
    }
}
