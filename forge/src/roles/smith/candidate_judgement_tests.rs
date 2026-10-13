use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::runtime::ActiveForgeRoleTask;
use std::collections::HashMap;

const BASE: &str = "1111111111111111111111111111111111111111";
const AFTER: &str = "2222222222222222222222222222222222222222";

/// A repository described by its answers: each git invocation (args joined by spaces) maps to its stdout, and
/// an invocation with no answer is a git that failed.
struct Repo {
    answers: HashMap<String, String>,
    test_mode: Option<&'static str>,
}

impl Repo {
    /// A clean, descending candidate that changes one test file.
    fn healthy() -> Self {
        let mut answers = HashMap::new();
        answers.insert(
            format!("merge-base --is-ancestor {BASE} {AFTER}"),
            String::new(),
        );
        answers.insert("status --porcelain".into(), String::new());
        answers.insert(
            format!("diff --name-only --no-renames {BASE}..{AFTER}"),
            "tests/tests/new_contract.rs".into(),
        );
        Self {
            answers,
            test_mode: None,
        }
    }

    fn answer(mut self, args: &str, stdout: Option<&str>) -> Self {
        match stdout {
            Some(out) => self.answers.insert(args.into(), out.into()),
            None => self.answers.remove(args),
        };
        self
    }
}

impl CandidateProbe for Repo {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.answers.get(&args.join(" ")).cloned()
    }
    fn declared_test_mode(&self) -> Option<&str> {
        self.test_mode
    }
}

impl RoleHarness for Repo {
    fn run_role(
        &self,
        _: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> Result<HarnessOutput> {
        unreachable!("the judgement never runs a turn")
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_command(&self, _: &str) -> CommandResult {
        unreachable!("the judgement never runs a command")
    }
    fn candidate_probe(&self) -> Option<&dyn CandidateProbe> {
        Some(self)
    }
}

fn reject(repo: &Repo, base: Option<&str>, after: Option<&str>) -> Option<String> {
    candidate_rejection_in_surface("smith", base, after, repo, None)
}

fn reject_in_surface(
    repo: &Repo,
    base: Option<&str>,
    after: Option<&str>,
    surface: &[String],
) -> Option<String> {
    candidate_rejection_in_surface("smith", base, after, repo, Some(surface))
}

fn output(after: Option<&str>) -> HarnessOutput {
    HarnessOutput {
        trajectory: None,
        raw: "smith answered".into(),
        candidate_sha: after.map(str::to_string),
        assay_commands: vec![],
        acceptance_mapped: false,
        refusal: None,
        execution_base: Some(BASE.into()),
        usage: None,
    }
}

#[test]
fn a_clean_descending_commit_that_changes_files_is_accepted() {
    assert_eq!(reject(&Repo::healthy(), Some(BASE), Some(AFTER)), None);
}

#[test]
fn candidate_changes_must_stay_inside_the_declared_surface() {
    let allowed_tree = vec!["tests/".to_string()];
    assert_eq!(
        reject_in_surface(&Repo::healthy(), Some(BASE), Some(AFTER), &allowed_tree),
        None,
        "an explicit tree contains its descendants"
    );

    let wrong_file = vec!["tests/other.rs".to_string()];
    let reason = reject_in_surface(&Repo::healthy(), Some(BASE), Some(AFTER), &wrong_file)
        .expect("a changed file outside its exact declared path is refused");
    assert!(reason.contains("outside its declared surface"), "{reason}");
    assert!(reason.contains("tests/tests/new_contract.rs"), "{reason}");
}

#[test]
fn every_refusal_names_its_reason() {
    let healthy = Repo::healthy;
    let cases: Vec<(Repo, Option<&str>, Option<&str>, &str)> = vec![
        (healthy(), None, Some(AFTER), "execution base is unreadable"),
        (healthy(), Some(BASE), None, "HEAD was unreadable"),
        (
            healthy(),
            Some(BASE),
            Some(BASE),
            "no new commit was created",
        ),
        (
            healthy(),
            Some(BASE),
            Some("abc123"),
            "is not a full commit SHA",
        ),
        (
            healthy().answer(&format!("merge-base --is-ancestor {BASE} {AFTER}"), None),
            Some(BASE),
            Some(AFTER),
            "is not a descendant of execution base",
        ),
        (
            healthy().answer("status --porcelain", None),
            Some(BASE),
            Some(AFTER),
            "git status is unreadable",
        ),
        (
            healthy().answer("status --porcelain", Some(" M web/src/lib.rs")),
            Some(BASE),
            Some(AFTER),
            "uncommitted work remains",
        ),
        (
            healthy().answer(
                &format!("diff --name-only --no-renames {BASE}..{AFTER}"),
                None,
            ),
            Some(BASE),
            Some(AFTER),
            "changed paths are unreadable",
        ),
        (
            healthy().answer(
                &format!("diff --name-only --no-renames {BASE}..{AFTER}"),
                Some(""),
            ),
            Some(BASE),
            Some(AFTER),
            "changes no files",
        ),
    ];
    for (repo, base, after, expected) in cases {
        let reason = reject(&repo, base, after).unwrap_or_default();
        assert!(
            reason.contains(expected),
            "expected {expected:?}, got {reason:?}"
        );
        assert!(
            reason.starts_with("Smith candidate refused for smith: "),
            "{reason}"
        );
    }
}

#[test]
fn rust_contract_refuses_a_production_touch_and_only_under_that_mode() {
    let touches_web = || {
        Repo::healthy().answer(
            &format!("diff --name-only --no-renames {BASE}..{AFTER}"),
            Some("tests/tests/new_contract.rs\nweb/src/api/error.rs"),
        )
    };
    assert_eq!(
        reject(&touches_web(), Some(BASE), Some(AFTER)),
        None,
        "no mode, no contract rule"
    );
    let mut contract = touches_web();
    contract.test_mode = Some("RUST_CONTRACT");
    let reason = reject(&contract, Some(BASE), Some(AFTER)).unwrap_or_default();
    assert!(
        reason.contains("RUST_CONTRACT candidate modified production code"),
        "{reason}"
    );
}

#[test]
fn a_refused_candidate_is_cleared_and_named_in_the_reply() {
    let repo = Repo::healthy().answer("status --porcelain", Some("?? scratch.txt"));
    let mut out = output(Some(AFTER));
    judge_delivered_candidate(&repo, "fast_repair_smith", &mut out);
    assert_eq!(
        out.candidate_sha, None,
        "a refused candidate is not the run's candidate"
    );
    let refusal = out.refusal.clone().expect("the refusal is recorded");
    assert!(
        out.raw
            .contains(&format!("SMITH_CANDIDATE_REJECTED: {refusal}")),
        "{}",
        out.raw
    );
}

#[test]
fn only_code_delivering_nodes_on_a_real_repository_are_judged() {
    let dirty = Repo::healthy().answer("status --porcelain", Some(" M x.rs"));
    let mut not_code = output(Some(AFTER));
    judge_delivered_candidate(&dirty, "architect", &mut not_code);
    assert_eq!(not_code.candidate_sha.as_deref(), Some(AFTER));
    assert_eq!(not_code.refusal, None);

    let mut accepted = output(Some(AFTER));
    judge_delivered_candidate(&Repo::healthy(), "lead_solo_implement", &mut accepted);
    assert_eq!(accepted.candidate_sha.as_deref(), Some(AFTER));
    assert_eq!(accepted.refusal, None);
}
