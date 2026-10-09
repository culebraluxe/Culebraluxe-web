//! FORGE.ASSAY-007 — a legacy view cannot authorize PASS.
//!
//! CONTRACT. A packet view and boolean mapping hint are legacy inputs. They cannot authorize
//! PASS without the run-frozen typed plan and its approved checks. The generic legacy helper is
//! retained for compatibility but is not the production measurement boundary.
//!
//! Level: L3 Composition — the production adjudicators over a packet-derived view.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__007__pass_requires_acceptance_mapped

use forge::engine::assay::{adjudicate_assay, AssayVerdict, CommandResult};
use forge::engine::packet::StoryPacket;
use forge::pianola::worker::TstStoryView;
use forge::pianola::worker_exec::adjudicate_for_view;

fn passing(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: 0,
        cancelled: false,
        passed: true,
        excerpt: "ok".into(),
        unmeasurable: false,
        output: "ok".into(),
    }
}

fn mapped_packet() -> StoryPacket {
    StoryPacket {
        id: "TST-PROBE".to_string(),
        title: "Probe story".to_string(),
        goal: Some("prove green means something".to_string()),
        special_instructions: None,
        architect_brief: None,
        acceptance_criteria: Some("every clause maps to an assay command".to_string()),
        test_mode: Some("RUST_CONTRACT".to_string()),
        assay_commands: vec!["cargo test -p test-harness --test probe".to_string()],
        branch_name: None,
        base_ref: None,
        base_commit: None,
    }
}

#[test]
fn forge_assay_007__pass_requires_acceptance_mapped() {
    let command = "cargo test -p test-harness --test probe".to_string();

    // The legacy helper remains a compatibility utility, not a production gate.
    let pass = adjudicate_assay(&[command.clone()], &[passing(&command)], true);
    assert_eq!(pass.verdict, AssayVerdict::Pass);
    assert!(
        pass.blockers.is_empty(),
        "a mapped green run carries no blockers: {:?}",
        pass.blockers
    );

    // A false legacy mapping hint remains unproven.
    let unproven = adjudicate_assay(&[command.clone()], &[passing(&command)], false);
    assert_eq!(
        unproven.verdict,
        AssayVerdict::Unproven,
        "green commands with no acceptance mapping must not pass"
    );
    assert!(
        unproven.blockers.contains(&"ACCEPTANCE_MAP_MISSING"),
        "the reading must name the missing mapping: {:?}",
        unproven.blockers
    );

    // Even acceptance prose and matching command text cannot authorize PASS.
    let view = TstStoryView::from_packet(&mapped_packet());
    let via_view = adjudicate_for_view(&view, &[passing(&command)]);
    assert_eq!(via_view.verdict, AssayVerdict::Unproven);
    assert!(via_view.blockers.contains(&"ASSAY_PLAN_REQUIRED"));
    let unmapped_packet = StoryPacket {
        acceptance_criteria: None,
        ..mapped_packet()
    };
    let unmapped_view = TstStoryView::from_packet(&unmapped_packet);
    let via_unmapped = adjudicate_for_view(&unmapped_view, &[passing(&command)]);
    assert!(
        via_unmapped.blockers.contains(&"ASSAY_PLAN_REQUIRED"),
        "a story view without a frozen plan cannot prove its green run: {:?}",
        via_unmapped.blockers
    );
    let blank_criteria = StoryPacket {
        acceptance_criteria: Some("   ".to_string()),
        ..mapped_packet()
    };
    let blank_view = adjudicate_for_view(
        &TstStoryView::from_packet(&blank_criteria),
        &[passing(&command)],
    );
    assert!(
        blank_view.blockers.contains(&"ASSAY_PLAN_REQUIRED"),
        "blank prose cannot replace a frozen plan: {:?}",
        blank_view.blockers
    );
}
