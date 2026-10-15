use super::*;

#[test]
fn maestro_harness_construction() {
    let _guard = lock_env();
    std::env::set_var("FORGE_WORKTREE", ".");
    std::env::set_var("FORGE_STORY_ID", "TEST-STORY");
    let harness = MaestroHarness::from_env().expect("harness construction");
    assert_eq!(harness.cli_bin, "maestro-cli");
    assert!(harness.workspace.exists());
    assert!(
        harness.agent.is_empty(),
        "from_env builds transport without intent"
    );
}

#[test]
fn backend_selection_opencode_maestro_fail_closed() {
    let _guard = lock_env();
    std::env::remove_var("FORGE_HARNESS");
    assert_eq!(
        HarnessBackend::from_env().expect("unset is opencode"),
        HarnessBackend::OpenCode
    );
    std::env::set_var("FORGE_HARNESS", "opencode");
    assert_eq!(
        HarnessBackend::from_env().expect("opencode is opencode"),
        HarnessBackend::OpenCode
    );
    std::env::set_var("FORGE_HARNESS", "maestro");
    assert_eq!(
        HarnessBackend::from_env().expect("maestro is maestro"),
        HarnessBackend::Maestro
    );
    // A misspelled vendor must fail configuration, never silently run OpenCode.
    std::env::set_var("FORGE_HARNESS", "maetsro");
    assert!(
        HarnessBackend::from_env().is_err(),
        "unknown FORGE_HARNESS must fail closed"
    );
    std::env::remove_var("FORGE_HARNESS");
}

#[test]
fn maestro_preflight_pass_and_fail() {
    // `/usr/bin/true` answers --version with exit 0; `/usr/bin/false` refuses; a missing binary
    // never spawns. No network, no tokens, no agent touched.
    assert!(verify_vendor_contract("/usr/bin/true").is_ok());
    assert!(verify_vendor_contract("/usr/bin/false").is_err());
    assert!(verify_vendor_contract("/nonexistent/maestro-cli-xyz").is_err());
}

#[test]
fn maestro_cli_default_is_maestro_cli() {
    let _guard = lock_env();
    std::env::remove_var("MAESTRO_BIN");
    assert_eq!(
        MaestroHarness::default_cli_bin().expect("default resolves"),
        "maestro-cli"
    );
}

#[test]
fn maestro_bin_explicit_override_is_selected() {
    let _guard = lock_env();
    std::env::set_var("MAESTRO_BIN", "/usr/local/bin/maestro-cli");
    assert_eq!(
        MaestroHarness::default_cli_bin().expect("explicit wins"),
        "/usr/local/bin/maestro-cli",
        "a valid explicit MAESTRO_BIN is used as configured"
    );
    std::env::remove_var("MAESTRO_BIN");
}

#[test]
fn maestro_bin_invalid_explicit_fails_closed() {
    let _guard = lock_env();
    std::env::set_var("MAESTRO_BIN", "/nonexistent/maestro-cli-xyz");
    // Resolution stays lenient (a name is not proof), but the bad explicit value is never replaced
    // by a fallback and is refused at preflight — before a claim, before a token.
    let bin = MaestroHarness::default_cli_bin().expect("no silent substitution");
    assert_eq!(bin, "/nonexistent/maestro-cli-xyz");
    assert!(
        verify_vendor_contract(&bin).is_err(),
        "an invalid explicit MAESTRO_BIN must fail closed"
    );
    std::env::remove_var("MAESTRO_BIN");
}

#[test]
fn maestro_backend_never_runs_direct_opencode_preflight() {
    // Structural guard on the binary: backend selection must precede vendor verification, and the
    // OpenCode contract check must sit behind the OpenCode backend arm — selecting Maestro must not
    // require the direct Forge → OpenCode harness preflight to pass.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/bin/forge/main.rs")
            .as_path(),
    )
    .expect("forge.rs is readable");
    let selection = source
        .find("HarnessBackend::from_env()")
        .expect("backend is selected");
    let opencode_verify = source
        .find("opencode_client::verify_vendor_contract")
        .expect("opencode verifies its own contract");
    let maestro_verify = source
        .find("maestro::verify_vendor_contract")
        .expect("maestro verifies its own contract");
    assert!(
        selection < opencode_verify && selection < maestro_verify,
        "the vendor must be resolved (and invalid config rejected) before any preflight runs"
    );
    let opencode_arm = source
        .rfind("HarnessBackend::OpenCode =>")
        .expect("opencode arm exists");
    assert!(
        opencode_arm < opencode_verify,
        "the OpenCode preflight lives inside the OpenCode backend arm only"
    );
}

#[test]
fn maestro_target_selection_never_calls_opencode_resolver() {
    // Maestro must translate Forge's vendor-neutral intent on its own: no OpenCode model-resolution
    // helper (or OpenCode agent map) may appear in Maestro's selection path. comments are allowed
    // to NAME the rule, so scan code with `//` line comments stripped.
    let source = maestro_module_source();
    let code: String = source
        .lines()
        .map(|line| match line.split_once("//") {
            Some((code, _)) => code,
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");
    // The guard below names the forbidden symbols; exclude the guard itself from the scan.
    let code = code
        .split("fn maestro_target_selection_never_calls_opencode_resolver")
        .next()
        .unwrap_or(&code);
    for forbidden in [
        "resolve_model_for_policy",
        "resolve_opencode_model",
        "v2_agent_env",
        "opencode_agents::v2_agent_for_node",
    ] {
        assert!(
            !code.contains(forbidden),
            "Maestro target selection must not use OpenCode's {forbidden}"
        );
    }
}
