//! FORGE.ASSAY-001 — assay commands come only from the approved packet.
//!
//! CONTRACT. The worker executes exactly the assay commands the approved story packet carries —
//! nothing added, nothing substituted, nothing invented. The chain is
//! `StoryPacket` (parsed from the `storyboard_story` row) → `TstStoryView::from_packet`
//! (carries the commands verbatim) → `run_assay_commands` (executes that list) → QA
//! adjudication, where `adjudicate_qa` refuses drift (`ASSAY_COMMAND_DRIFT`) and substitution
//! (`ASSAY_COMMAND_SUBSTITUTED`). A packet with no commands yields no commands: an empty plan
//! adjudicates `NO_ASSAY_COMMANDS` rather than inventing work.
//!
//! Level: L3 Composition — the production packet/view/adjudication boundary, commands faked.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__001__assay_commands_come_only_from_approved_packet

use db::StoryPacketRow;
use forge::engine::assay::CommandResult;
use forge::engine::packet::StoryPacket;
use forge::engine::qa_adjudicate::{adjudicate_qa, AssayPlan};
use forge::pianola::worker::TstStoryView;

fn approved_packet() -> StoryPacket {
    StoryPacket {
        id: "TST-PROBE".to_string(),
        title: "Probe story".to_string(),
        goal: Some("prove the packet owns the assay plan".to_string()),
        special_instructions: None,
        architect_brief: None,
        acceptance_criteria: Some("the packet's commands run, unaltered".to_string()),
        test_mode: Some("RUST_CONTRACT".to_string()),
        assay_commands: vec![
            "cargo test -p test-harness --test probe_one".to_string(),
            "cargo check --manifest-path Cargo.toml --workspace --all-targets".to_string(),
        ],
        branch_name: None,
        base_ref: None,
        base_commit: None,
    }
}

fn planned(commands: &[String]) -> AssayPlan {
    AssayPlan {
        commands: commands.to_vec(),
        conditions: vec![],
        negative_control: None,
    }
}

fn measured(command: &str, passed: bool) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: if passed { 0 } else { 101 },
        passed,
        excerpt: "probe excerpt".into(),
        unmeasurable: false,
        output: "probe output".into(),
    }
}

#[test]
fn forge_assay_001__assay_commands_come_only_from_approved_packet() {
    // ── 1. THE VIEW CARRIES THE PACKET'S COMMANDS VERBATIM. ──────────────────
    let packet = approved_packet();
    let view = TstStoryView::from_packet(&packet);
    assert_eq!(
        view.assay_commands, packet.assay_commands,
        "the worker's assay plan must be the packet's assay plan, in packet order, with nothing added"
    );

    // The row path parses the same way: blank lines are dropped, commands survive trimmed.
    let row = StoryPacketRow {
        id: "TST-PROBE".to_string(),
        title: "Probe story".to_string(),
        goal: None,
        architect_brief: None,
        acceptance_criteria: None,
        test_mode: None,
        assay_commands: Some(
            "  cargo test -p probe --test one  \n\ncargo check --workspace\n".to_string(),
        ),
    };
    assert_eq!(
        TstStoryView::from_row(&row).assay_commands,
        vec![
            "cargo test -p probe --test one".to_string(),
            "cargo check --workspace".to_string()
        ],
        "row parsing must trim and drop blanks without adding or reordering commands"
    );

    // ── 2. MATCHING EXECUTION ADJUDICATES CLEAN. ─────────────────────────────
    let plan = planned(&packet.assay_commands);
    let results: Vec<CommandResult> = packet
        .assay_commands
        .iter()
        .map(|command| measured(command, true))
        .collect();
    let report = adjudicate_qa(&plan, &results, None, None, None);
    assert!(
        !report
            .blockers
            .iter()
            .any(|blocker| blocker.starts_with("ASSAY_COMMAND")),
        "executing exactly the approved commands must not read as drift: {:?}",
        report.blockers
    );

    // ── 3. NEGATIVE: A SUBSTITUTED COMMAND IS REFUSED. ───────────────────────
    let mut substituted = results.clone();
    substituted[0] = measured("cargo test -p probe --test something_else", true);
    let drifted = adjudicate_qa(&plan, &substituted, None, None, None);
    assert!(
        drifted
            .blockers
            .iter()
            .any(|blocker| blocker.starts_with("ASSAY_COMMAND_SUBSTITUTED")),
        "running a command the packet did not approve must be refused: {:?}",
        drifted.blockers
    );
    assert!(
        drifted.blockers.iter().any(|blocker| blocker
            .contains("cargo test -p probe --test something_else")),
        "the refusal must name the off-plan command: {:?}",
        drifted.blockers
    );

    // ── 4. NEGATIVE: A LONGER/SHORTER RESULT LIST IS DRIFT. ──────────────────
    let mut extra = results.clone();
    extra.push(measured("echo invented", true));
    let drift = adjudicate_qa(&plan, &extra, None, None, None);
    assert!(
        drift.blockers.iter().any(|blocker| blocker == "ASSAY_COMMAND_DRIFT"),
        "executing more commands than the packet approved is drift: {:?}",
        drift.blockers
    );
    let short = adjudicate_qa(&plan, &results[..1], None, None, None);
    assert!(
        short.blockers.iter().any(|blocker| blocker == "ASSAY_COMMAND_DRIFT"),
        "executing fewer commands than the packet approved is drift: {:?}",
        short.blockers
    );

    // ── 5. NEGATIVE: AN EMPTY PACKET INVENTS NOTHING. ────────────────────────
    let empty_packet = StoryPacket {
        assay_commands: vec![],
        ..approved_packet()
    };
    let empty_view = TstStoryView::from_packet(&empty_packet);
    assert!(
        empty_view.assay_commands.is_empty(),
        "a packet with no assay commands must yield no assay commands, never a default"
    );
    let empty_plan = planned(&[]);
    let no_plan = adjudicate_qa(&empty_plan, &[], None, None, None);
    assert!(
        no_plan.blockers.iter().any(|blocker| blocker == "NO_ASSAY_COMMANDS"),
        "an empty approved plan must surface as missing, not as a pass: {:?}",
        no_plan.blockers
    );
}
