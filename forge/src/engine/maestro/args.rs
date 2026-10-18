//! Args half of the Maestro harness.

use super::*;

/// Environment variable naming the Maestro agent this turn runs on. Wins outright: when the operator
/// names an agent, no policy translates it.
pub const MAESTRO_AGENT_ENV: &str = "MAESTRO_AGENT";
/// Policy-specific Maestro agents: the cheap and judgment tiers resolve here when `MAESTRO_AGENT` is unset.
pub const MAESTRO_CHEAP_AGENT_ENV: &str = "MAESTRO_CHEAP_AGENT";
pub const MAESTRO_JUDGMENT_AGENT_ENV: &str = "MAESTRO_JUDGMENT_AGENT";
/// Configured default Maestro agent: the last resort before failing closed.
pub const MAESTRO_DEFAULT_AGENT_ENV: &str = "MAESTRO_DEFAULT_AGENT";

/// Resolve the Maestro agent for one turn — a PURE function of its inputs, so the precedence is testable
/// without mutating the process environment.
///
/// Precedence, and the reason: an explicitly named agent is an attended decision and wins; otherwise the
/// policy names its tier; otherwise the configured default; otherwise FAIL CLOSED before model execution.
/// There is deliberately no silent fallback to an OpenCode model and no arbitrary pick: spending tokens on a
/// vendor nobody chose is the failure this order exists to remove.
///
/// `explicit` is `MAESTRO_AGENT`; `selection` is Forge's vendor-neutral intent. An `Explicit` selection
/// carries a vendor selection directly (a Maestro agent id on this side, an OpenCode model id on the other)
/// and never passes through the OpenCode resolver.
pub fn resolve_maestro_agent(
    selection: &crate::engine::harness::ModelSelection,
    explicit: Option<&str>,
    cheap: Option<&str>,
    judgment: Option<&str>,
    default: Option<&str>,
) -> Result<String> {
    use crate::engine::harness::ModelSelection;
    let clean = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| match agent_alias(value) {
                Some(real) => real.to_string(),
                None => value.to_string(),
            })
    };
    if let Some(agent) = clean(explicit) {
        return Ok(agent);
    }
    if let ModelSelection::Explicit(agent) = selection {
        let agent = agent.trim();
        if !agent.is_empty() {
            return Ok(agent_alias(agent).unwrap_or(agent).to_string());
        }
    }
    let tiered = match selection {
        ModelSelection::Judgment => clean(judgment),
        ModelSelection::Cheap => clean(cheap),
        ModelSelection::Explicit(_) => None,
    };
    if let Some(agent) = tiered.or_else(|| clean(default)) {
        return Ok(agent);
    }
    Err(WorkflowError::generic(
        "Maestro harness has no agent: set MAESTRO_AGENT (or MAESTRO_CHEAP_AGENT / MAESTRO_JUDGMENT_AGENT \
         for the turn's policy, or MAESTRO_DEFAULT_AGENT). Refusing to guess a vendor.",
    ))
}

/// Sanitize environment for the Maestro subprocess: same security boundary as OpenCode (no production
/// database authority crosses into a model turn, no git publication out of one), without any OpenCode
/// model configuration — a Maestro agent owns its provider, model and working directory itself.
pub(super) fn sanitized_model_env() -> std::collections::HashMap<String, String> {
    crate::engine::opencode::sanitized_model_env()
}

/// Build the Maestro `send` invocation — the vendor's headless execution contract.
///
/// `maestro-cli send <agent-id> <message>`, plus exactly one `--session <id>` when resuming. Measured
/// against `maestro-cli send --help`: there is no `--model`, `--auto`, `--continue` or `--agent` on this
/// command, and passing OpenCode flags would fail the turn against the real CLI.
pub(super) fn build_maestro_send_args(
    agent: &str,
    task: &str,
    session: Option<&str>,
    read_only: bool,
) -> Vec<String> {
    let mut args = vec!["send".to_string(), agent.to_string(), task.to_string()];
    // Forge's role authority, enforced by the vendor: a read-only lane cannot modify files even
    // though the roster agent itself is write-capable.
    if read_only {
        args.push("--read-only".to_string());
    }
    if let Some(id) = session.map(str::trim).filter(|id| !id.is_empty()) {
        args.push("--session".to_string());
        args.push(id.to_string());
    }
    args
}
