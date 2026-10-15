// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|value| value.to_string()).collect()
}

/// No environment at all: what the flags said is all there is.
fn no_env(_name: &str) -> Option<String> {
    None
}

#[test]
fn the_flags_are_read_into_the_launch() {
    let cli = Cli::from_args_with(
        &args(&[
            "forge",
            "--story",
            "STORY-1",
            "--work-type",
            "BUG",
            "--work-item",
            "item-1",
            "--claim-owner",
            "worker-a",
            "--claim-generation",
            "3",
            "--stop-after",
            "architect",
        ]),
        &no_env,
    )
    .expect("a complete launch parses");

    assert_eq!(cli.story, "STORY-1");
    assert_eq!(cli.work_type, "BUG");
    assert_eq!(cli.work_item.as_deref(), Some("item-1"));
    assert_eq!(
        cli.claim_fence,
        Some(db::ClaimFence::new("worker-a", 3)),
        "the claim's authority is (owner, generation), not the work item alone"
    );
    assert!(
        matches!(&cli.stop_after, Some(ForgeStopTarget::Role("architect"))),
        "the dispatch cap is a lane, not the whole chain: {:?}",
        cli.stop_after
    );
}

#[test]
fn the_environment_fills_in_what_the_flags_omitted() {
    let cli = Cli::from_args_with(&args(&["forge"]), &|name| match name {
        "FORGE_STORY_ID" => Some("STORY-2".into()),
        "FORGE_WORK_TYPE" => Some("FAST".into()),
        "FORGE_STOP_AFTER" => Some(" scout ".into()),
        _ => None,
    })
    .expect("a launch named only by the environment parses");

    assert_eq!(cli.story, "STORY-2");
    assert_eq!(cli.work_type, "FAST");
    assert_eq!(
        cli.work_item, None,
        "an operator invocation owns no queue row"
    );
    assert_eq!(cli.claim_fence, None, "and so it names no authority");
    assert!(
        matches!(&cli.stop_after, Some(ForgeStopTarget::Role("scout"))),
        "a cap is trimmed before it is read: {:?}",
        cli.stop_after
    );
}

#[test]
fn a_flag_beats_the_variable_it_mirrors() {
    let cli = Cli::from_args_with(
        &args(&["forge", "--story", "STORY-FLAG", "--work-type", "HOTFIX"]),
        &|name| match name {
            "FORGE_STORY_ID" => Some("STORY-ENV".into()),
            "FORGE_WORK_TYPE" => Some("FEATURE".into()),
            _ => None,
        },
    )
    .expect("the flags are the launch");

    assert_eq!(cli.story, "STORY-FLAG");
    assert_eq!(cli.work_type, "HOTFIX");
}

#[test]
fn the_work_type_defaults_and_the_six_allowed_words_are_the_only_ones() {
    let cli = Cli::from_args_with(&args(&["forge", "--story", "S"]), &no_env)
        .expect("a story is enough to launch");
    assert_eq!(cli.work_type, "FEATURE", "the default chain");

    let refusal = Cli::from_args_with(
        &args(&["forge", "--story", "S", "--work-type", "BOGUS"]),
        &no_env,
    )
    .expect_err("an unknown work type is refused, never widened to the default");
    assert_eq!(refusal.exit_code, 2);
    assert_eq!(refusal.message, "invalid --work-type BOGUS");
}

#[test]
fn a_story_is_required_and_its_absence_prints_the_invocation() {
    let refusal = Cli::from_args_with(&args(&["forge"]), &no_env).expect_err("no --story, no run");
    assert_eq!(refusal.exit_code, 2);
    assert!(
        refusal.message.starts_with("usage: forge --story <id>"),
        "a refused launch says how to launch: {}",
        refusal.message
    );
}

#[test]
fn a_work_item_without_a_claim_owner_cannot_be_fenced() {
    let refusal = Cli::from_args_with(
        &args(&["forge", "--story", "S", "--work-item", "item-1"]),
        &no_env,
    )
    .expect_err("migration 278: a claimed run must name the authority it executes under");
    assert_eq!(refusal.exit_code, 2);
    assert!(
        refusal
            .message
            .contains("--work-item item-1 without --claim-owner cannot be fenced"),
        "{}",
        refusal.message
    );
}

#[test]
fn an_unreadable_stop_after_is_refused_rather_than_widened() {
    let refusal = Cli::from_args_with(
        &args(&["forge", "--story", "S", "--stop-after", "nowhere"]),
        &no_env,
    )
    .expect_err("a cap that cannot be read must not become the whole chain");
    assert_eq!(refusal.exit_code, 2);
    assert_eq!(
        refusal.message,
        "invalid --stop-after nowhere: expected scout|architect|lead"
    );
}

#[test]
fn a_claim_generation_that_is_not_a_number_is_refused() {
    let refusal = Cli::from_args_with(
        &args(&[
            "forge",
            "--story",
            "S",
            "--work-item",
            "item-1",
            "--claim-owner",
            "worker-a",
            "--claim-generation",
            "three",
        ]),
        &no_env,
    )
    .expect_err("a generation that cannot be read is not generation 0");
    assert_eq!(refusal.exit_code, 2);
    assert!(
        refusal.message.contains("invalid --claim-generation three"),
        "{}",
        refusal.message
    );
}
