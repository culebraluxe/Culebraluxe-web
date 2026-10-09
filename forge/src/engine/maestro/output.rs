//! Pure parsing for Maestro process output; no process, session, or persistence behavior lives here.

use crate::engine::harness::HarnessUsage;
use workflow::{Result, WorkflowError};

/// Parse usage from a single Maestro streaming output line (if available).
/// Returns None if the line doesn't contain usage info.
pub(super) fn parse_usage_from_line(line: &str) -> Option<HarnessUsage> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
        if let Some(usage_obj) = json.get("usage").or_else(|| json.get("usage_info")) {
            if let (Some(input), Some(output), Some(cost)) = (
                usage_obj
                    .get("input_tokens")
                    .or_else(|| usage_obj.get("tokens_input"))
                    .and_then(|v| v.as_i64()),
                usage_obj
                    .get("output_tokens")
                    .or_else(|| usage_obj.get("tokens_output"))
                    .and_then(|v| v.as_i64()),
                usage_obj
                    .get("cost_usd")
                    .or_else(|| usage_obj.get("cost"))
                    .and_then(|v| v.as_f64()),
            ) {
                return Some(HarnessUsage {
                    session_id: String::new(), // Will be filled by parse_maestro_output
                    tokens_input: input,
                    tokens_output: output,
                    cost_usd: cost,
                });
            }
        }
    }
    None
}

/// Maestro run result.
#[derive(Debug, Clone)]
pub(super) struct MaestroRunResult {
    pub(super) exit_code: Option<i32>,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

/// Parsed Maestro output with structured fields.
#[derive(Debug, Clone, Default)]
pub(super) struct ParsedMaestroOutput {
    pub(super) assistant_text: String,
    pub(super) session_id: Option<String>,
    pub(super) usage: Option<HarnessUsage>,
}

/// Parse the `maestro-cli send` JSON response into Forge's turn output.
///
/// The vendor envelope (`maestro-cli send` prints one JSON response):
/// `response` → the turn text, `sessionId` → vendor-session persistence, `usage.*` → spend accounting, and
/// `success: false` → a harness execution failure carrying Maestro's error. A failed response is never
/// reinterpreted as successful plain text, and an unparseable response is a malformed-response failure —
/// never an empty success.
pub(super) fn parse_maestro_output(stdout: &str) -> Result<ParsedMaestroOutput> {
    // The vendor prints one JSON object; incidental lines around it are ignored by taking the LAST line
    // that parses as an object carrying the envelope. Nothing JSON-shaped at all is malformed output.
    let mut envelope: Option<serde_json::Value> = None;
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        if json.is_object() {
            envelope = Some(json);
        }
    }
    if envelope.is_none() {
        for line in stdout.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if json.is_object()
                    && (json.get("success").is_some()
                        || json.get("response").is_some()
                        || json.get("sessionId").is_some())
                {
                    envelope = Some(json);
                    break;
                }
            }
        }
    }
    let Some(json) = envelope else {
        return Err(WorkflowError::generic(format!(
            "maestro-harness: malformed response (no JSON envelope in {} bytes of stdout)",
            stdout.len()
        )));
    };

    if json.get("success").and_then(|value| value.as_bool()) == Some(false) {
        let error = json
            .get("error")
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string())
            })
            .filter(|text| !text.trim().is_empty() && text != "null")
            .unwrap_or_else(|| "unknown Maestro failure".to_string());
        return Err(WorkflowError::generic(format!(
            "maestro-harness: turn failed: {error}"
        )));
    }

    let mut result = ParsedMaestroOutput::default();
    // Maestro's field is `sessionId`. `session_id`/`sessionID` stay as tolerated aliases; nothing else does.
    if let Some(id) = json
        .get("sessionId")
        .or_else(|| json.get("session_id"))
        .or_else(|| json.get("sessionID"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        result.session_id = Some(id.to_string());
    }
    result.assistant_text = json
        .get("response")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_default();
    if let Some(usage_obj) = json.get("usage") {
        // Cost is authoritative-or-nothing: without `totalCostUsd` the turn's spend is unmeasured and
        // `usage` stays `None` (the field's documented meaning — "unmeasured, never zero"). Recording
        // tokens beside a fake $0 would let a spend cap believe an expensive turn was free.
        if let Some(cost) = usage_obj
            .get("totalCostUsd")
            .and_then(|value| value.as_f64())
        {
            result.usage = Some(HarnessUsage {
                session_id: result.session_id.clone().unwrap_or_default(),
                tokens_input: usage_obj
                    .get("inputTokens")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(0),
                tokens_output: usage_obj
                    .get("outputTokens")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(0),
                cost_usd: cost,
            });
        }
    }
    if result.assistant_text.trim().is_empty() {
        return Err(WorkflowError::generic(
            "maestro-harness: the response envelope carries no response text",
        ));
    }
    Ok(result)
}
