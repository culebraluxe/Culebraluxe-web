//! ARCH-SEAM-006 — the vendor contract is verified before anything is paid for.
//!
//! CONTRACT (the C1 incident, 2026-10-03: the lane resolved the npm `opencode-ai` 1.18.26 instead of the v2 build,
//! opened a run, dispatched `architect`, and died on the vendor's help text). A lane whose vendor CLI does not speak
//! the contract Forge is written against must end with
//!
//! ```text
//! zero model turns · zero candidate · zero role result · a clear engine/configuration failure
//! ```
//!
//! This reuses the production gate, `opencode_client::verify_vendor_contract`; it does not invent a second probe.
//!
//!   * EXECUTABLE: against a vendor that logs every invocation, the gate refuses the 1.18.26 contract by name and
//!     accepts the v2 contract — and on both, the vendor saw only `--help` / `--version`. The gate itself never
//!     starts a turn.
//!   * STRUCTURAL, over the lane's composition root (`bin/forge.rs` `main`, which a test cannot call): the gate runs
//!     BEFORE the run is opened, before the harness is built and before the generation is driven; its failure arm
//!     records a configuration rejection and exits; and the gate probes the same binary the harness will run.
//!
//! Level: L1 executable (a local fake vendor, no network, no model); L0 structural.

#[path = "support/forge_arch_chain.rs"]
mod arch;

use std::path::{Path, PathBuf};

use arch::{production_code, workspace_root};
use forge::engine::opencode_client::verify_vendor_contract;

/// The v2 build's help — every option Forge emits is named (`tests/tests/opencode_v2_contract.rs` holds the same).
const V2_HELP: &str = "opencode v2.0.21
USAGE
  opencode run [flags] [<message...>]
FLAGS
  --standalone            Run with a private server instead of the background service
  --format choice         Output format (choices: default, json)
  --model, -m string      Model to use in the format provider/model#variant
  --agent string          Agent to use
  --session, -s string    Session ID to continue, or to create if it does not exist
  --continue, -c          Continue the last session
  --auto                  Auto-approve permissions that are not explicitly denied";

/// The npm `opencode-ai` 1.18.26 help, as C1 resolved it: no `--standalone`.
const V1_HELP: &str = "1.18.26
opencode run [message..]
Options:
  -c, --continue     continue the last session
  -s, --session      session id to continue
  -m, --model        model to use in the format of provider/model
      --agent        agent to use
      --format       format: default (formatted) or json (raw JSON events)
      --auto         auto-approve permissions that are not explicitly denied (dangerous!)";

/// A vendor that appends every invocation to `<name>.calls`, prints its help for `--help`/`--version`, and — if it is
/// ever asked for anything else — records the paid turn it would have been.
fn logging_vendor(dir: &Path, name: &str, help: &str) -> (String, PathBuf) {
    let path = dir.join(name);
    let log = dir.join(format!("{name}.calls"));
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$*\" in\n  *--help*|*--version*) cat <<'VENDOR_EOF'\n{help}\nVENDOR_EOF\n    ;;\n  *) echo MODEL_TURN >> '{log}'; exit 3 ;;\nesac\n",
            log = log.display()
        ),
    )
    .expect("vendor script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    (path.to_string_lossy().to_string(), log)
}

fn calls(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn the_gate_refuses_the_wrong_contract_by_name_and_never_starts_a_turn() {
    let dir = std::env::temp_dir().join(format!("forge-arch-seam-006-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let (v1, v1_log) = logging_vendor(&dir, "opencode-ai", V1_HELP);
    let (v2, v2_log) = logging_vendor(&dir, "opencode-v2", V2_HELP);

    let refused = verify_vendor_contract(&v1).expect_err("the 1.18.26 contract is refused");
    assert!(
        refused.contains("--standalone") && refused.contains("no model turn was attempted"),
        "the refusal names the missing option and says nothing was spent: {refused}"
    );
    verify_vendor_contract(&v2).expect("the v2 contract is accepted");
    let absent = verify_vendor_contract(&dir.join("missing").to_string_lossy())
        .expect_err("a vendor that cannot be run is refused");
    assert!(absent.contains("could not be run"), "{absent}");

    for (name, log) in [("1.18.26", &v1_log), ("v2", &v2_log)] {
        let seen = calls(log);
        assert!(!seen.is_empty(), "the {name} vendor was actually probed");
        assert!(
            seen.iter()
                .all(|call| call.contains("--help") || call.contains("--version")),
            "the gate asked the {name} vendor for more than its contract: {seen:?}"
        );
        assert!(
            !seen.iter().any(|call| call == "MODEL_TURN"),
            "a model turn was started"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// What `main` does after the gate, in order: each is a step that spends something.
const PAID_AFTER_THE_GATE: [&str; 4] = [
    "agent_work::begin_agent_work_run(",
    "OpenCodeHarness::from_env_for_policy(",
    "ProductionRoleRunner::new(",
    "drive_forge_story_with_jobs(",
];

#[test]
fn the_lane_verifies_its_vendor_before_it_opens_a_run_builds_a_harness_or_drives_a_turn() {
    let main = production_code(&workspace_root().join("forge/src/bin/forge.rs"));
    let gate = main
        .find("verify_vendor_contract(")
        .expect("the lane verifies its vendor contract");
    for step in PAID_AFTER_THE_GATE {
        let at = main
            .find(step)
            .unwrap_or_else(|| panic!("the lane no longer calls `{step}`"));
        assert!(
            gate < at,
            "`{step}` runs before the vendor contract is verified"
        );
    }

    // The failure arm: a configuration rejection, then the process ends — nothing after the gate is reached.
    let arm_start = main[gate..]
        .find("Err(e) =>")
        .map(|offset| gate + offset)
        .expect("the gate has an Err arm");
    let exit = main[arm_start..]
        .find("std::process::exit(")
        .map(|offset| arm_start + offset)
        .expect("the gate's failure arm exits");
    let arm = &main[arm_start..exit];
    assert!(
        arm.contains("reject_configuration(") && !arm["Err(e) =>".len()..].contains("=>"),
        "the gate's failure arm must reject the configuration and exit, within the one arm: {arm}"
    );
    assert!(
        PAID_AFTER_THE_GATE
            .iter()
            .all(|step| main.find(step).map(|at| exit < at).unwrap_or(false)),
        "the failure arm exits before any paid step is reached"
    );

    // The binary probed is the binary run.
    assert!(
        main[gate..].starts_with("verify_vendor_contract(")
            && main[gate..gate + 200].contains("default_cli_bin()"),
        "the gate probes `opencode::default_cli_bin()`"
    );
    let harness = production_code(&workspace_root().join("forge/src/engine/opencode.rs"));
    assert!(
        harness.contains("cli_bin: default_cli_bin()"),
        "the harness runs `default_cli_bin()` — the binary the gate verified"
    );
}
