use super::*;

#[test]
fn maestro_session_resume_precedence() {
    // The pure rule, shared with OpenCode's: marker wins, then a lane session proven local, else fresh.
    assert_eq!(
        resume_session(Some("marker".into()), Some("lane".into()), true),
        Some("marker".to_string())
    );
    assert_eq!(
        resume_session(None, Some("lane".into()), true),
        Some("lane".to_string())
    );
    assert_eq!(resume_session(None, Some("lane".into()), false), None);
    assert_eq!(resume_session(None, None, false), None);
}

/// Opt-in integration: the real `maestro-cli` against a cheap test agent. NEVER runs by default —
/// it spends model tokens. Enable with `FORGE_MAESTRO_INTEGRATION=1` and name the agent explicitly:
/// `MAESTRO_TEST_AGENT=<cheap test agent>`.
#[test]
#[ignore = "spends model tokens: FORGE_MAESTRO_INTEGRATION=1 with MAESTRO_TEST_AGENT set"]
fn maestro_live_send_integration() {
    if std::env::var("FORGE_MAESTRO_INTEGRATION").ok().as_deref() != Some("1") {
        eprintln!("skipped: FORGE_MAESTRO_INTEGRATION != 1");
        return;
    }
    let agent = std::env::var("MAESTRO_TEST_AGENT").expect(
        "FORGE_MAESTRO_INTEGRATION=1 requires MAESTRO_TEST_AGENT naming a cheap test agent",
    );
    let bin = MaestroHarness::default_cli_bin().expect("maestro-cli resolves");
    let args = build_maestro_send_args(&agent, "Reply with exactly: integration-ok", None);
    let output = std::process::Command::new(&bin)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("maestro-cli runs");
    assert!(
        output.status.success(),
        "send failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed = parse_maestro_output(&String::from_utf8_lossy(&output.stdout))
        .expect("the live response parses");
    assert!(
        !parsed.assistant_text.trim().is_empty(),
        "the live turn answered"
    );
    assert!(
        parsed.session_id.is_some(),
        "the live turn minted a session"
    );
}

/// WO-120: the Maestro marker obeys the same contract as the OpenCode one — an unwritable
/// marker is an error, never a silence.
#[test]
fn maestro_session_marker_write_failure_is_an_error() {
    let missing = std::path::Path::new("/nonexistent-forge-workspace-dir-wo120").join("x");
    write_session_id(missing.to_str().unwrap(), Some("ses_123"))
        .expect_err("an unwritable marker must be an error");
}

/// WO-120: clearing a Maestro session that was never marked is not a failure.
#[test]
fn maestro_session_marker_clear_is_idempotent() {
    let dir =
        std::env::temp_dir().join(format!("forge-maestro-marker-clear-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let workspace = dir.to_string_lossy().to_string();
    write_session_id(&workspace, None).expect("clearing a missing marker is not a failure");
    write_session_id(&workspace, Some("ses_abc")).expect("write marker");
    assert_eq!(read_session_id(&workspace).as_deref(), Some("ses_abc"));
    write_session_id(&workspace, None).expect("clear");
    assert_eq!(read_session_id(&workspace), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// WO-120: the Maestro vendor-session persist failure reuses the shared renderer, carrying the
/// Maestro harness name so the propagated error names its own source.
#[test]
fn maestro_vendor_session_failure_names_harness_node_story_and_cause() {
    let error = crate::engine::opencode::vendor_session_persist_error(
        "maestro-harness",
        "architect",
        "T-STORY",
        "db down",
    );
    let text = format!("{error}");
    for part in ["maestro-harness", "architect", "T-STORY", "db down"] {
        assert!(text.contains(part), "the error must name {part}: {text}");
    }
}

#[test]
fn maestro_turn_timeout_carries_code_and_seconds() {
    // WO-202: the ceiling error carries the TURN_TIMEOUT code and formats the ceiling as
    // whole seconds (7200s), never as a Duration debug string.
    let message = turn_timeout_message(Duration::from_secs(7200));
    assert!(message.contains("TURN_TIMEOUT"), "{message}");
    assert!(message.contains("7200s"), "{message}");
    assert!(!message.contains("7200s, 0ns"), "{message}");

    // And the code agrees with the OpenCode turn's vocabulary: one code, not two.
    assert_eq!(
        message.split(':').next().unwrap_or(""),
        crate::engine::opencode_client::TURN_TIMEOUT_CODE
    );
}

#[test]
fn maestro_session_never_crosses_worktrees() {
    // A lane session proven to belong to a different workspace is refused; resuming falls through to fresh.
    assert_eq!(
        resume_session(None, Some("other-worktree-session".into()), false),
        None,
        "a session from another worktree must never be resumed here"
    );
}

#[test]
fn maestro_child_env_does_not_inherit_prod_database_url() {
    // WO-201 (red first): a sentinel DATABASE_URL_PROD in the parent environment must be
    // absent in the spawned child. The allowlist (`sanitized_model_env`) already strips it
    // from the map, but `Command` inherits the parent environment unless `env_clear()` runs —
    // so the child sees the sentinel until the spawn clears first and applies second.
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let _guard = lock_env();
    let sentinel = "wo201-sentinel-prod-url";
    std::env::set_var("DATABASE_URL_PROD", sentinel);
    let dir = std::env::temp_dir().join(format!("forge-maestro-env-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let script = dir.join("print-prod.sh");
    {
        let mut f = std::fs::File::create(&script).expect("script");
        writeln!(
            f,
            "#!/bin/sh\necho \"PROD_URL=[${{DATABASE_URL_PROD-unset}}]\""
        )
        .expect("write");
    }
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    // Built AFTER the sentinel is set, so the allowlist itself is proven to exclude it.
    let env = sanitized_model_env();
    assert!(
        !env.contains_key("DATABASE_URL_PROD"),
        "the allowlist must strip DATABASE_URL_PROD"
    );
    let result = run_maestro_streaming(
        script.to_str().unwrap(),
        dir.to_str().unwrap(),
        &[],
        Some(&env),
        None,
        Some(Duration::from_secs(60)),
    )
    .expect("the probe script runs");
    std::env::remove_var("DATABASE_URL_PROD");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !result.stdout.contains(sentinel),
        "the child inherited DATABASE_URL_PROD from the parent: {}",
        result.stdout.trim()
    );
}

#[test]
fn maestro_task_text_preserves_forge_context() {
    // The task text Maestro receives is Forge's canonical builder output: role node, goal, brief,
    // acceptance, and execution workspace context survive the adapter unchanged.
    let _guard = lock_env();
    std::env::set_var("FORGE_WORKTREE", ".");
    std::env::set_var("FORGE_STORY_ID", "T-STORY");
    let mut harness = MaestroHarness::from_env().expect("harness");
    harness.packet.goal = Some("GOAL-TEXT".into());
    harness.packet.architect_brief = Some("BRIEF-TEXT".into());
    harness.packet.acceptance_criteria = Some("ACCEPT-TEXT".into());
    let task = crate::engine::runtime::ActiveForgeRoleTask {
        task_id: "task-1".into(),
        process_instance_id: "proc-1".into(),
        story_id: "T-STORY".into(),
        token_id: None,
        node_id: Some("architect".into()),
        status: workflow::TaskStatus::InProgress,
        assignee: None,
        candidates: vec![],
        write_surface: None,
    };
    let text = harness.task_text("architect", &task, None);
    let text = text.as_str();
    assert!(text.contains("GOAL-TEXT"), "{text}");
    assert!(text.contains("BRIEF-TEXT"), "{text}");
    assert!(text.contains("ACCEPT-TEXT"), "{text}");
    assert!(text.contains("architect"), "{text}");
    // The isolation posture travels with the text. This harness was handed a workspace, so the turn is told it
    // is in one — the adapter does not leave the model to guess which checkout it is editing.
    assert!(text.contains("Execution isolation"), "{text}");
    // Architect grants no planning, so no scaffold is composed for it: the policy is the NODE's own lane
    // (`packet::TaskText::for_node`), not an absence the adapter filled in.
    assert!(!text.contains("Planning policy"), "{text}");
}
