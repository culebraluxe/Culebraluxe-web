//! Preflight half of the Maestro harness.

use super::*;

/// Verify the Maestro CLI contract: the binary spawns and answers `--version`.
///
/// This is the Maestro half of vendor preflight — run when (and only when) the Maestro backend is selected,
/// so a Maestro turn never depends on the OpenCode harness passing. A Maestro turn that cannot prove its
/// transport never starts: the failure names the binary, not the story.
///
/// `Ok` carries the version line the lane logs; `Err` is the diagnostic. Same shape as OpenCode's
/// `verify_vendor_contract`, so the binary can verify either backend through one call shape.
pub fn verify_vendor_contract(cli_bin: &str) -> std::result::Result<String, String> {
    let mut cmd = Command::new(cli_bin);
    cmd.arg("--version");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let output = cmd.output().map_err(|error| {
        format!(
            "maestro preflight failed to spawn `{cli_bin}`: {error}. Check MAESTRO_BIN and PATH."
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "maestro preflight failed (exit={}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(format!("{version} @ {cli_bin}"))
}

pub(super) fn maestro_preflight(cli_bin: &str) -> Result<()> {
    verify_vendor_contract(cli_bin)
        .map(|vendor| eprintln!("maestro preflight: version={vendor}"))
        .map_err(WorkflowError::generic)
}
