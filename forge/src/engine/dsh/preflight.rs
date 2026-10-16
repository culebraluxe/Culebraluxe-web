//! Preflight for the DSH harness: prove the CLI and the `headless` profile before a claim is spent.

use std::process::{Command, Stdio};

use workflow::{Result, WorkflowError};

use super::args::DshCli;
use super::DSH_PROFILE;

fn run(cli: &DshCli, extra: &[&str]) -> std::result::Result<std::process::Output, String> {
    let mut cmd = Command::new(&cli.program);
    cmd.args(&cli.prefix);
    cmd.args(extra);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.output().map_err(|error| {
        format!(
            "DSH preflight failed to spawn `{}`: {error}. Check DSH_BIN and PATH.",
            cli.display()
        )
    })
}

/// Verify the CLI answers `--version`. Same shape as the OpenCode/Maestro preflights so the binary can
/// verify whichever backend was selected, before opening a claim.
pub fn verify_vendor_contract(cli: &DshCli) -> std::result::Result<String, String> {
    let output = run(cli, &["--version"])?;
    if !output.status.success() {
        return Err(format!(
            "DSH preflight failed (exit={}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(format!(
        "{} @ {}",
        String::from_utf8_lossy(&output.stdout).trim(),
        cli.display()
    ))
}

/// Verify the selected profile exists by asking the vendor for its own help. This is a local probe: no
/// task is run and no model is called.
pub fn verify_profile(cli: &DshCli, profile: &str) -> Result<()> {
    let output = run(cli, &["--profile", profile, "--help"]).map_err(WorkflowError::generic)?;
    if !output.status.success() {
        return Err(WorkflowError::generic(format!(
            "DSH profile {profile:?} is not bootable (`{} --profile {profile} --help` exit={}): {}",
            cli.display(),
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// Fail-closed preflight before any turn token is spent.
pub fn preflight(cli: &DshCli) -> Result<()> {
    let version = verify_vendor_contract(cli).map_err(WorkflowError::generic)?;
    eprintln!("dsh preflight: version={version}");
    verify_profile(cli, DSH_PROFILE)
}
