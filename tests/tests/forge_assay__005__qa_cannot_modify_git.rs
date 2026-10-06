//! FORGE.ASSAY-005 — QA cannot modify Git.
//!
//! CONTRACT. The QA lane (Assay/Inspector) measures and reports; it never mutates the
//! repository. Two facts pin this at the production boundary:
//!
//!   1. THE LANE MODULE OWNS NO GIT DOOR. `forge/src/roles/qa.rs` names no mutation verb
//!      (`push`/`merge`/`rebase`/`commit`/`checkout`/`worktree` as git arguments), spawns no
//!      process of its own (`Command::new`), and holds no release handle. Its one git read
//!      (`git rev-parse HEAD`) goes through the engine's command port
//!      (`ctx.harness.run_command`), the same port every lane uses — a read through the port
//!      is measurement, not a door.
//!   2. QA ROUTING NEVER ROUTES TO A MUTATION. `route_qa_result` answers every QA outcome
//!      with Pass, Smith repair, Architect replan, or Hold — none of which is a git mutation,
//!      and no Hold reason instructs one.
//!
//! Level: L3 Composition — the production lane source plus the production repair router.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__005__qa_cannot_modify_git

use forge::engine::qa_repair::{
    route_qa_result, RepairAttemptState, RepairBudget, RepairRouting, QaDisposition, QaVerdict,
};
use test_harness::source;

/// The QA lane module: the only file that may turn a QA node into action.
const QA_LANE: &str = "forge/src/roles/qa.rs";

/// Git mutation verbs as argument literals, the process spawn a lane would need to run git
/// itself, and the release surface. A QA module naming any of these owns a git door.
const GIT_DOOR_TOKENS: [&str; 10] = [
    "\"push\"",
    "\"merge\"",
    "\"rebase\"",
    "\"commit\"",
    "\"checkout\"",
    "\"worktree\"",
    "Command::new",
    "HostReleaseExecutor",
    "GitReleaseOps",
    "FORGE_ALLOW_PUBLISH",
];

/// Code with `//` line comments removed. `qa.rs` carries no URLs and no block comments, so a
/// `//` always opens a comment — and prose about commits must never read as a git commit.
fn code_of(text: &str) -> String {
    text.lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// English verbs no QA routing outcome may instruct. Checked as whole words so "replan" or
/// "repair cycle" can never trip them — and so a real instruction cannot hide behind prose.
fn instructs_git_mutation(reason: &str) -> bool {
    reason
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|word| matches!(word, "push" | "merge" | "rebase" | "commit" | "checkout"))
}

#[test]
fn forge_assay_005__qa_cannot_modify_git() {
    // ── 0. THE DETECTOR, AGAINST PLANTED SAMPLES. A scan that cannot fail is not a check. ──
    assert!(
        code_of("let args = [\"push\", \"origin\"];").contains("\"push\""),
        "a real git argument literal must be seen"
    );
    assert!(
        !code_of("outcomes.push(result); // never runs `git push`").contains("\"push\""),
        "`Vec::push` is not a push and a comment is not code"
    );
    assert!(
        instructs_git_mutation("now push the lane branch"),
        "a plain-language push instruction must be seen"
    );
    assert!(
        !instructs_git_mutation("do not auto-launch another repair cycle"),
        "repair prose is not a mutation instruction"
    );

    // ── 1. THE QA LANE MODULE OWNS NO GIT DOOR. ─────────────────────────────────
    let lane_path = source::repo_root().join(QA_LANE);
    assert!(lane_path.is_file(), "the QA lane module moved: {QA_LANE}");
    let lane_code = code_of(&source::read(&lane_path));
    assert!(
        lane_code.contains("run_rust_contract_qa"),
        "the scan must read the real lane module, not an empty file"
    );
    for token in GIT_DOOR_TOKENS {
        assert!(
            !lane_code.contains(token),
            "{QA_LANE} names `{token}`: the QA lane owns no git door — it measures through the \
             engine's command port and routes through policy, it never pushes, merges, rebases, \
             commits, spawns git, or holds the release handle"
        );
    }

    // ── 2. QA ROUTING NEVER ROUTES TO A MUTATION. ──────────────────────────────
    let dispositions = [
        None,
        Some(QaDisposition::Repair),
        Some(QaDisposition::Replan),
        Some(QaDisposition::Escalate),
    ];
    for verdict in [QaVerdict::Pass, QaVerdict::Fail] {
        for disposition in dispositions {
            for verification_gap in [false, true] {
                for no_progress in [false, true] {
                    let routing = route_qa_result(
                        verdict,
                        disposition,
                        RepairAttemptState {
                            repair_attempts: 0,
                            replan_attempts: 0,
                        },
                        RepairBudget::default(),
                        no_progress,
                        verification_gap,
                    );
                    match routing {
                        RepairRouting::Pass
                        | RepairRouting::Smith { .. }
                        | RepairRouting::Architect { .. } => {}
                        RepairRouting::Hold { reason } => assert!(
                            !instructs_git_mutation(&reason),
                            "a QA hold must never instruct a git mutation \
                             ({verdict:?}/{disposition:?}): {reason}"
                        ),
                    }
                }
            }
        }
    }

    // ── 3. NEGATIVE: EXHAUSTED BUDGETS HOLD FOR A HUMAN, NOT FOR GIT. ─────────
    let spent = RepairAttemptState {
        repair_attempts: 99,
        replan_attempts: 99,
    };
    for disposition in [Some(QaDisposition::Repair), Some(QaDisposition::Replan)] {
        let routing = route_qa_result(
            QaVerdict::Fail,
            disposition,
            spent,
            RepairBudget::default(),
            false,
            false,
        );
        match routing {
            RepairRouting::Hold { reason } => {
                assert!(
                    reason.contains("operator/Lead"),
                    "an exhausted budget escalates to a human: {reason}"
                );
                assert!(
                    !instructs_git_mutation(&reason),
                    "even the escalation never reaches for git: {reason}"
                );
            }
            other => panic!("an exhausted budget must hold: {other:?}"),
        }
    }
}
