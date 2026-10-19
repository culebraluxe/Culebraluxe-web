//! Preflight for the DSH harness: prove the CLI, the pinned **version** and the `headless` profile
//! before a claim is spent.
//!
//! The version is pinned deliberately: a harness contrast is evidence about the harness, so comparing
//! runs across an unpinned DSH upgrade would attribute vendor behaviour to the candidate revision. The
//! expected value comes from `DSH_EXPECTED_VERSION` and defaults to the version this adapter was
//! written and measured against ([`DEFAULT_DSH_VERSION`]). A mismatch is a hard refusal, never a
//! warning: downgrade/upgrade is an explicit, rehearsed operation (docs/agent/SOP-DSH-UPGRADE.md).
//!
//! The actual version is read from the vendor's own `--version`, and the model-turn sandbox records it
//! in its receipt and probe evidence so an experiment can always name the vendor build it ran.

use std::process::{Command, Stdio};

use workflow::{Result, WorkflowError};

use super::DSH_PROFILE;
use super::args::DshCli;

/// The DSH version this adapter was written and measured against.
pub const DEFAULT_DSH_VERSION: &str = "0.1.1-rc.2";

/// Environment variable that re-pins the expected DSH version (an explicit, reviewed operation).
pub const DSH_EXPECTED_VERSION_ENV: &str = "DSH_EXPECTED_VERSION";

/// The pin the preflight enforces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DshVersion {
    /// The version the vendor reported, normalised (no leading `v`).
    pub actual: String,
    /// The version `DSH_EXPECTED_VERSION` (or the default) requires.
    pub expected: String,
}

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

/// The expected pin from an arbitrary lookup (testable without mutating the process environment).
///
/// An absent, empty or whitespace-only value means the default; a non-empty value is taken verbatim
/// (the pin is an exact version, not a range).
pub fn expected_version_from(lookup: impl Fn(&str) -> Option<String>) -> String {
    lookup(DSH_EXPECTED_VERSION_ENV)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_DSH_VERSION.to_string())
}

/// The expected pin for this process.
pub fn expected_version() -> String {
    expected_version_from(|key| std::env::var(key).ok())
}

/// Parse a version token out of a vendor `--version` line.
///
/// Vendors print different shapes (`0.1.1-rc.2`, `dsh 0.1.1-rc.2`, `@deepseek-ai/dsh v0.1.1-rc.2`);
/// this accepts the first token that is `major.minor.patch` with an optional pre-release suffix and
/// nothing else, so a banner line cannot be mistaken for a version.
pub fn parse_vendor_version(output: &str) -> Option<String> {
    for raw in output.split_whitespace() {
        let token = raw.trim_matches(|character: char| {
            !(character.is_ascii_alphanumeric()
                || character == '.'
                || character == '-'
                || character == '/'
                || character == '@'
                || character == 'v')
        });
        // Common vendor shapes embed the version after a tool/package name (`dsh/0.1.1-rc.2`,
        // `@deepseek-ai/dsh@0.1.1-rc.2`); try the whole token and its last path/package segment.
        let mut candidates = vec![token];
        candidates.push(token.rsplit('/').next().unwrap_or(token));
        candidates.push(token.rsplit('@').next().unwrap_or(token));
        for candidate in candidates {
            if let Some(version) = parse_version_token(candidate) {
                return Some(version);
            }
        }
    }
    None
}

/// One token that is exactly `major.minor.patch` plus an optional pre-release suffix.
fn parse_version_token(token: &str) -> Option<String> {
    let token = token.strip_prefix('v').unwrap_or(token);
    // `+build` metadata is not part of identity; the pre-release suffix keeps its dots
    // (`0.1.1-rc.2` is one token, not four dotted components).
    let token = token.split_once('+').map_or(token, |(base, _)| base);
    let (core, pre_release) = match token.split_once('-') {
        Some((core, suffix)) => (core, Some(suffix)),
        None => (token, None),
    };
    let mut parts = core.split('.');
    let (Some(major), Some(minor), Some(patch), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if major.is_empty()
        || minor.is_empty()
        || patch.is_empty()
        || !major.bytes().all(|byte| byte.is_ascii_digit())
        || !minor.bytes().all(|byte| byte.is_ascii_digit())
        || !patch.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    if let Some(suffix) = pre_release {
        if suffix.is_empty()
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
        {
            return None;
        }
    }
    Some(token.to_string())
}

/// Compare the vendor's version output with the pin. A missing/unparseable version is a refusal too:
/// "we could not tell" must never read as "it matched".
pub fn verify_version_output(
    output: &str,
    expected: &str,
) -> std::result::Result<DshVersion, String> {
    let actual = parse_vendor_version(output).ok_or_else(|| {
        format!(
            "DSH `--version` output is not a version (`{}`); cannot verify the {expected} pin",
            output.trim()
        )
    })?;
    if actual != expected {
        return Err(format!(
            "DSH version mismatch: expected {expected} (from {DSH_EXPECTED_VERSION_ENV} or the adapter default), vendor reports {actual}. Refusing before a claim is spent; see docs/agent/SOP-DSH-UPGRADE.md."
        ));
    }
    Ok(DshVersion {
        actual,
        expected: expected.to_string(),
    })
}

/// Verify the CLI answers `--version` with the pinned version. Same shape as the OpenCode/Maestro
/// preflights so the binary can verify whichever backend was selected, before opening a claim.
pub fn verify_vendor_version(cli: &DshCli) -> std::result::Result<DshVersion, String> {
    let output = run(cli, &["--version"])?;
    if !output.status.success() {
        return Err(format!(
            "DSH preflight failed (exit={}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    verify_version_output(&stdout, &expected_version())
}

/// The compatibility display string old callers used (`<version> @ <cli>`). The version is checked.
pub fn verify_vendor_contract(cli: &DshCli) -> std::result::Result<String, String> {
    let version = verify_vendor_version(cli)?;
    Ok(format!("{} @ {}", version.actual, cli.display()))
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

/// Fail-closed preflight before any turn token is spent. Returns the verified version so the caller
/// can record it in evidence/receipts.
pub fn preflight_with_version(cli: &DshCli) -> std::result::Result<DshVersion, WorkflowError> {
    let version = verify_vendor_version(cli).map_err(WorkflowError::generic)?;
    eprintln!(
        "dsh preflight: version={} (expected {}, pinned)",
        version.actual, version.expected
    );
    verify_profile(cli, DSH_PROFILE)?;
    Ok(version)
}

/// Fail-closed preflight before any turn token is spent.
pub fn preflight(cli: &DshCli) -> Result<()> {
    preflight_with_version(cli).map(|_| ())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn parses_the_version_shapes_the_vendor_prints() {
        assert_eq!(
            parse_vendor_version("0.1.1-rc.2\n").as_deref(),
            Some("0.1.1-rc.2")
        );
        assert_eq!(
            parse_vendor_version("dsh 0.1.1-rc.2\n").as_deref(),
            Some("0.1.1-rc.2")
        );
        assert_eq!(
            parse_vendor_version("@deepseek-ai/dsh v0.1.1-rc.2\n").as_deref(),
            Some("0.1.1-rc.2")
        );
        assert_eq!(
            parse_vendor_version("dsh/0.1.1-rc.2\n").as_deref(),
            Some("0.1.1-rc.2")
        );
        assert_eq!(
            parse_vendor_version("@deepseek-ai/dsh@0.1.1-rc.2\n").as_deref(),
            Some("0.1.1-rc.2")
        );
        assert_eq!(parse_vendor_version("ready\n").as_deref(), None);
        assert_eq!(parse_vendor_version("").as_deref(), None);
    }

    #[test]
    fn a_matching_version_is_accepted_and_a_mismatch_is_refused() {
        let pinned = verify_version_output("0.1.1-rc.2\n", "0.1.1-rc.2").expect("pinned version");
        assert_eq!(pinned.actual, "0.1.1-rc.2");
        assert_eq!(pinned.expected, "0.1.1-rc.2");

        let error = verify_version_output("0.1.2\n", "0.1.1-rc.2").expect_err("mismatch refuses");
        assert!(error.contains("version mismatch"), "{error}");
        assert!(error.contains("0.1.2"), "{error}");
        assert!(error.contains("0.1.1-rc.2"), "{error}");
        assert!(error.contains(DSH_EXPECTED_VERSION_ENV), "{error}");

        let unreadable = verify_version_output("not a version", "0.1.1-rc.2")
            .expect_err("unparseable version refuses");
        assert!(unreadable.contains("is not a version"), "{unreadable}");
    }

    #[test]
    fn the_pin_comes_from_the_environment_or_the_adapter_default() {
        assert_eq!(
            expected_version_from(|_| None),
            DEFAULT_DSH_VERSION,
            "no env override uses the adapter default"
        );
        assert_eq!(
            expected_version_from(|_| Some("  ".to_string())),
            DEFAULT_DSH_VERSION,
            "blank override is absent, not a pin"
        );
        assert_eq!(
            expected_version_from(|key| {
                (key == DSH_EXPECTED_VERSION_ENV).then(|| " 0.1.2 ".to_string())
            }),
            "0.1.2",
            "the operator pin is trimmed and used verbatim"
        );
    }
}
