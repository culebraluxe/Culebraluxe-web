//! Pure output parsing for the DSH harness. Headless prints the final assistant text to stdout; the
//! vendor session id is recovered from the session directory, and usage is not machine-readable.

use std::path::{Path, PathBuf};

use super::args;

/// What one headless run produced.
#[derive(Debug, Clone, Default)]
pub struct ParsedDshOutput {
    pub assistant_text: String,
    pub session_id: Option<String>,
    pub session_dir: Option<PathBuf>,
}

/// Normalize one completed headless run.
///
/// `stdout` is the assistant's final message (the vendor prints nothing else there); `stderr` belongs to
/// the error path and is surfaced by the caller. The session directory is the newest `session-*` under
/// the workspace's project root — the same evidence id the original adapter persisted.
pub fn parse(stdout: &str, session_root: &Path) -> ParsedDshOutput {
    let session_id = args::discover_latest_session(session_root);
    let session_dir = session_id.as_ref().map(|id| session_root.join(id));
    ParsedDshOutput {
        assistant_text: stdout.trim_end().to_string(),
        session_id,
        session_dir,
    }
}
