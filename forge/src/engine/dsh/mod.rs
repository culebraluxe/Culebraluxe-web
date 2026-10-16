//! DeepSeek Harness (DSH) execution backend — the original Forge transport, restored in Rust.
//!
//! This is the Rust counterpart of the deleted TypeScript adapter
//! (`agent-runtime/deepseek/deepseek-harness-adapter.ts` + `deepseek/dsh-client.ts`, removed with the
//! TypeScript engine in 2026-09). Forge remains the control plane; DSH is one execution transport
//! behind `FORGE_HARNESS=dsh`.
//!
//! What the vendor contract actually is (verified against the installed CLI, and re-verified by
//! [`preflight::verify_vendor_contract`]):
//!
//! - `dsh --profile headless "<task>"` runs one task to completion and prints the final assistant text
//!   to stdout; exit 0 = completed, non-zero = failed.
//! - There is no `--model` flag. The only per-run selection seam is a `--patch` overlay applied after
//!   the profile layer. [`args::model_patch`] renders the `agent-default-model` override for it.
//! - The session id (`session-<uuid>`) is generated internally and **not** printed. It is recovered
//!   after the run from `$DSH_HOME/sessions/<projectKey>/session-*` ([`args::discover_latest_session`]),
//!   using the harness's exact `projectKey` escaping.
//! - Headless has no native pause/resume-by-id: each invocation is a fresh session. Forge's lease and
//!   heartbeat remain the liveness truth; the session id is recorded as evidence, not resumption.
//! - The vendor reports no machine-readable usage on the headless path, so `HarnessUsage` is
//!   `None` (UNMEASURED, never zero). The wall-clock turn ceiling is the control that always holds.
//!
//! The module deliberately does not touch workflow state, does not publish, and does not read
//! production credentials.

use crate::engine::runtime::ActiveForgeRoleTask;
use workflow::{Result, WorkflowError};

pub mod args;
pub mod harness;
pub mod output;
pub mod preflight;

pub use harness::DshHarness;
pub use preflight::verify_vendor_contract;

/// The harness adapter id printed in execution receipts.
pub const DSH_HARNESS_ADAPTER_ID: &str = "dsh-harness";

/// The DSH profile a headless turn boots.
pub const DSH_PROFILE: &str = "headless";

/// Environment variable naming the DSH CLI (a command on PATH, or an absolute `lib/bin.js`).
pub const DSH_BIN_ENV: &str = "DSH_BIN";

/// Environment variable overriding the model (exact `provider/model`), highest precedence.
pub const DSH_MODEL_ENV: &str = "DSH_MODEL";
/// Policy-tier model overrides: the cheap and judgment intents translate here when `DSH_MODEL` is unset.
pub const DSH_CHEAP_MODEL_ENV: &str = "DSH_MODEL_CHEAP";
pub const DSH_JUDGMENT_MODEL_ENV: &str = "DSH_MODEL_JUDGMENT";

/// Environment variable for turn ceiling (wall-clock timeout).
pub const TURN_CEILING_ENV: &str = crate::engine::config::TURN_CEILING_ENV;

/// The vendor-session lane key this harness reads and writes.
pub const VENDOR_SESSION_LANE: &str = "dsh-v2";

/// Session marker filename for workspace-level observation.
pub const SESSION_MARKER_FILENAME: &str = ".forge-dsh-session";

/// Resolve the DSH CLI command: an explicit `DSH_BIN`, the installed profile's `lib/bin.js`, or `dsh`
/// on PATH. A `bin.js` is executed through `node` (the vendor's documented entry point).
pub fn default_cli() -> Result<args::DshCli> {
    let explicit = std::env::var(DSH_BIN_ENV).ok();
    let dsh_home = std::env::var_os("DSH_HOME");
    let home = std::env::var_os("HOME");
    args::resolve_cli(explicit.as_deref(), dsh_home.as_deref(), home.as_deref())
}

/// Resolve the model for one turn: `DSH_MODEL` wins, then the policy tier, else the profile default.
pub fn resolve_model(model_policy: Option<&str>) -> Result<Option<String>> {
    let clean = |value: Option<String>| {
        value
            .map(|raw| raw.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    if let Some(model) = clean(std::env::var(DSH_MODEL_ENV).ok()) {
        return Ok(Some(args::validate_model(&model)?));
    }
    let tier = match model_policy {
        Some("judgment") => clean(std::env::var(DSH_JUDGMENT_MODEL_ENV).ok()),
        _ => clean(std::env::var(DSH_CHEAP_MODEL_ENV).ok()),
    };
    match tier {
        Some(model) => Ok(Some(args::validate_model(&model)?)),
        None => Ok(None),
    }
}

/// The fully resolved transport for one turn.
#[derive(Debug, Clone)]
pub struct DshTransport {
    pub cli: args::DshCli,
    pub profile: String,
    pub model: Option<String>,
}
