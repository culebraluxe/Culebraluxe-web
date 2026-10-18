//! One read-only advisory turn through the DSH harness.
//!
//! Phase 2 panels are questions, not stories: each seat gets the same bounded packet, runs in its
//! own fresh headless session with `DSH_PERMISSION_MODE=read-only`, and reports a verdict. This
//! module deliberately does not know about Forge workflow state — the panel runner above it owns
//! routing and synthesis.

use std::path::Path;
use std::sync::{Arc, Mutex};

use super::harness::run_headless;
use super::usage::{self, TokenTotals};
use super::{args, default_cli, output, preflight, resolve_model, DSH_PROFILE};

/// What one advisory seat produced.
#[derive(Debug, Clone, Default)]
pub struct AdvisoryTurn {
    pub text: String,
    /// Token totals from the session log; `None` means unmeasured.
    pub tokens: Option<TokenTotals>,
}

/// Run one read-only advisory prompt in `workspace`.
///
/// The seat is read-only by sandbox mode; the model is the operator's configured DSH model (the
/// `--patch` seam is used when one is configured). No Forge role task is synthesized.
pub fn run_read_only_prompt(
    workspace: &Path,
    prompt: &str,
    model: Option<&str>,
) -> workflow::Result<AdvisoryTurn> {
    if !workspace.exists() {
        return Err(workflow::WorkflowError::generic(format!(
            "DSH advisory workspace does not exist: {}",
            workspace.display()
        )));
    }
    let cli = default_cli()?;
    preflight::preflight(&cli)?;
    let patch = match model {
        Some(model) => Some(args::ModelPatchFile::create(model)?),
        None => None,
    };
    let argv = args::headless_args(
        DSH_PROFILE,
        patch.as_ref().map(args::ModelPatchFile::path),
        prompt,
    );
    let mut env = crate::engine::opencode::sanitized_model_env();
    env.insert("DSH_PERMISSION_MODE".to_string(), "read-only".to_string());
    let child = Arc::new(Mutex::new(None));
    let max_turn =
        crate::engine::config::turn_ceiling(std::env::var(super::TURN_CEILING_ENV).ok().as_deref());
    let result = run_headless(
        &cli,
        &workspace.to_string_lossy(),
        &argv,
        Some(&env),
        max_turn,
        &child,
    )?;
    drop(patch);
    if result.exit_code != Some(0) {
        return Err(workflow::WorkflowError::generic(format!(
            "dsh advisory turn failed: exit={:?}, stderr={}",
            result.exit_code,
            result.stderr.trim()
        )));
    }
    let dsh_home = args::dsh_home(
        std::env::var_os("DSH_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    );
    let session_root = args::session_root(workspace, &dsh_home);
    let parsed = output::parse(&result.stdout, &session_root);
    let tokens = parsed
        .session_dir
        .as_deref()
        .and_then(usage::read_token_totals);
    Ok(AdvisoryTurn {
        text: parsed.assistant_text,
        tokens,
    })
}

/// The model an advisory panel should use: the operator's `DSH_MODEL`, else the profile default.
pub fn advisory_model() -> workflow::Result<Option<String>> {
    resolve_model(None)
}
