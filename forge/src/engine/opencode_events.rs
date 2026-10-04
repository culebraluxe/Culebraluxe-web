//! The OpenCode V2 structured run contract: `run --standalone --format json` writes newline-delimited JSON.
//!
//! Port note: the V1 adapter treated `opencode run` stdout as one human answer (`HarnessOutput.raw = stdout`).
//! On V2 that stdout is a machine transcript, so Forge reads it here instead: a typed layer over the vendor's
//! event stream that yields the three things a Forge turn needs — the session id, the assistant's text, and the
//! usage of every model step.
//!
//! Captured from the installed build (opencode v2.0.21, 2026-10-01). The events Forge consumes:
//!
//! ```text
//! {"type":"step_start",  "timestamp":…, "sessionID":"ses_…", "part":{"type":"step-start"}}
//! {"type":"tool_use",    "timestamp":…, "sessionID":"ses_…", "part":{"type":"tool","tool":"shell","state":…}}
//! {"type":"step_finish", "timestamp":…, "sessionID":"ses_…", "part":{"type":"step-finish","reason":"tool-calls",
//!                                                             "cost":9.6984e-05,
//!                                                             "tokens":{"input":224,"output":41,"reasoning":0}}}
//! {"type":"text",        "timestamp":…, "sessionID":"ses_…", "part":{"type":"text","text":"…"}}
//! ```
//!
//! Two vendor facts that shape the code below, both observed rather than assumed:
//!
//! 1. The session id key is `sessionID`, and it rides on EVERY event (top level, and again inside `part`).
//!    V1 never had this: it guessed the turn's session afterwards by scanning for the newest session in the
//!    lane's directory. Now the id the vendor actually used is simply reported (§4).
//! 2. A multi-step turn emits one `step_start` per step but does NOT reliably emit a `step_finish` for the
//!    TERMINAL step: a two-step turn (`step_start`, `tool_use`, `step_finish`, `step_start`, `text`) sums to
//!    LESS than the session's own exported total. So the sum below is a floor, and `engine::harness_usage`
//!    prefers the vendor's `session export` for the authoritative figure.
//!
//! Tolerance rules (§3): an unknown `type` is preserved and ignored — V2 will add event types and an unrelated
//! one must never fail a turn. Malformed JSON is the opposite: it means the structured contract broke, so the
//! turn FAILS CLOSED rather than being read as a successful turn that said nothing.

use serde_json::Value;

use crate::engine::harness::HarnessUsage;
use workflow::{Result, WorkflowError};

/// V2's own key for a session id, at the top level of an event and again inside `part`.
pub const SESSION_ID_KEY: &str = "sessionID";

/// Everything one `opencode run` turn told Forge, in the vendor's own words.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenCodeTurnEvents {
    /// The session this turn actually ran in. NOT the id Forge asked for: V2 mints the id, and reporting it
    /// here is what lets a FRESH turn be continued explicitly afterwards (§4).
    pub session_id: Option<String>,
    /// Every `text` event's text, concatenated in arrival order. This is the role output Forge logs and the
    /// evidence `forge_agent_collect` reads — never the raw NDJSON (§3).
    pub assistant_text: String,
    /// Usage summed from this run's `step_finish` events. `None` = unmeasured, never a fabricated zero (§6).
    ///
    /// A FLOOR, not necessarily the total: see the module note about the terminal step. `harness_usage`
    /// prefers the vendor's `session export` and falls back to this only when the export is unavailable.
    pub usage: Option<HarnessUsage>,
    /// How many `step_finish` events were seen. `0` means usage is absent, not zero.
    pub steps: usize,
    /// The vendor's structured error, when it reported one.
    pub error: Option<String>,
    /// Every parsed event, kept for diagnostics and tests. Not a source of `assistant_text`.
    pub events: Vec<Value>,
}

impl OpenCodeTurnEvents {
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Parse the NDJSON stream `--format json` produced.
///
/// Unknown event types are tolerated and preserved. A line that is not valid JSON is an error (§3).
pub fn parse_run_events(stdout: &str) -> Result<OpenCodeTurnEvents> {
    let mut scanner = RunEventScanner::default();
    for line in stdout.lines() {
        scanner.feed(line)?;
    }
    Ok(scanner.finish())
}

/// The same contract, read INCREMENTALLY: feed lines as they arrive, ask what the turn has done so far.
///
/// This exists because enforcement has to happen while the turn is running. A transcript read at the end can only
/// report what a turn already spent; a line-by-line reading is what lets `engine::opencode` stop a turn the moment
/// its measured spend passes the cap (and it is the same fold, so the live view and the recorded view cannot drift
/// apart — `parse_run_events` is this type run to completion).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunEventScanner {
    turn: OpenCodeTurnEvents,
    lines: usize,
}

impl RunEventScanner {
    /// Fold one line. Line numbering is the scanner's own, so the error a live read produces names the same line
    /// the recorded read would.
    pub fn feed(&mut self, line: &str) -> Result<()> {
        let index = self.lines;
        self.lines += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let event: Value = serde_json::from_str(trimmed).map_err(|error| {
            WorkflowError::generic(format!(
                "opencode v2 structured output is not valid JSON at line {}: {error} (excerpt: {})",
                index + 1,
                excerpt(trimmed)
            ))
        })?;
        fold_event(&mut self.turn, event);
        Ok(())
    }

    /// What the turn has done so far. A live reading, not a verdict: an unmeasured spend stays `None`.
    pub fn turn(&self) -> &OpenCodeTurnEvents {
        &self.turn
    }

    pub fn lines(&self) -> usize {
        self.lines
    }

    /// The finished turn, with the session id applied to the usage the same way the whole-stream read does.
    pub fn finish(mut self) -> OpenCodeTurnEvents {
        if let (Some(usage), Some(id)) = (self.turn.usage.as_mut(), self.turn.session_id.as_deref())
        {
            usage.session_id = id.to_string();
        }
        self.turn
    }
}

/// Fold one parsed event into the turn. The single place the vendor's event vocabulary is read, so a new event type
/// is taught once.
fn fold_event(turn: &mut OpenCodeTurnEvents, event: Value) {
    if turn.session_id.is_none() {
        turn.session_id = session_id_of(&event).map(str::to_string);
    }
    match event_type(&event).as_deref() {
        Some("text") => {
            if let Some(text) = event
                .get("part")
                .and_then(|part| part.get("text"))
                .and_then(Value::as_str)
            {
                turn.assistant_text.push_str(text);
            }
        }
        Some("step_finish") => {
            turn.steps += 1;
            if let Some(part) = event.get("part") {
                let tokens = part.get("tokens");
                let cost = part.get("cost").and_then(Value::as_f64);
                let input = tokens
                    .and_then(|tokens| tokens.get("input"))
                    .and_then(Value::as_i64);
                let output = tokens
                    .and_then(|tokens| tokens.get("output"))
                    .and_then(Value::as_i64);
                // Present-but-zero is a measurement; absent is not. Only the latter stays unmeasured.
                if cost.is_some() || input.is_some() || output.is_some() {
                    let totals = turn.usage.get_or_insert_with(|| HarnessUsage {
                        session_id: String::new(),
                        tokens_input: 0,
                        tokens_output: 0,
                        cost_usd: 0.0,
                    });
                    totals.tokens_input += input.unwrap_or(0);
                    totals.tokens_output += output.unwrap_or(0);
                    totals.cost_usd += cost.unwrap_or(0.0);
                }
            }
        }
        Some("error") => {
            if turn.error.is_none() {
                turn.error = error_message(&event);
            }
        }
        Some(_) => {}
        // No `type` at all. The one shape V2 has actually produced here is a bare provider error —
        // `{"name":"UnknownError","message":"Unexpected server error"}` — which is a failure Forge must see
        // rather than an event it should ignore.
        None => {
            if turn.error.is_none() {
                turn.error = error_message(&event);
            }
        }
    }
    turn.events.push(event);
}

/// The event's kind, from the top level (`type`) or, failing that, the part's own type with V2's hyphen
/// spelling normalised (`step-finish` → `step_finish`).
fn event_type(event: &Value) -> Option<String> {
    event
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .get("part")
                .and_then(|part| part.get("type"))
                .and_then(Value::as_str)
        })
        .map(|kind| kind.replace('-', "_"))
}

fn session_id_of(event: &Value) -> Option<&str> {
    event
        .get(SESSION_ID_KEY)
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .get("part")
                .and_then(|part| part.get(SESSION_ID_KEY))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

/// `{"name":"…","message":"…"}` and `{"error":{"message":"…"}}` are the two error shapes seen from V2.
fn error_message(event: &Value) -> Option<String> {
    let name = event.get("name").and_then(Value::as_str);
    let message = event
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
        })
        .or_else(|| event.get("error").and_then(Value::as_str));
    match (name, message) {
        (Some(name), Some(message)) => Some(format!("{name}: {message}")),
        (Some(name), None) => Some(name.to_string()),
        (None, Some(message)) => Some(message.to_string()),
        (None, None) => None,
    }
}

fn excerpt(line: &str) -> String {
    line.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real 2.0.21 shapes. Ids and text neutralised; every key, nesting and number is as captured.
    const MULTI_STEP: &str = concat!(
        r#"{"type":"step_start","timestamp":1790912098119,"sessionID":"ses_fixture_turn_0001","part":{"id":"prt_a1","sessionID":"ses_fixture_turn_0001","messageID":"msg_m1","type":"step-start"}}"#,
        "\n",
        r#"{"type":"tool_use","timestamp":1790912098574,"sessionID":"ses_fixture_turn_0001","part":{"partID":"prt_a2","sessionID":"ses_fixture_turn_0001","messageID":"msg_m1","type":"tool","id":"call_1","tool":"shell","state":{"status":"completed","input":{"command":"echo hello"},"output":"hello\n","metadata":{"metadata":{"status":"completed","truncated":false,"exit":0}}}}}"#,
        "\n",
        r#"{"type":"step_finish","timestamp":1790912098575,"sessionID":"ses_fixture_turn_0001","part":{"id":"prt_a3","sessionID":"ses_fixture_turn_0001","messageID":"msg_m1","type":"step-finish","reason":"tool-calls","cost":0.000096984,"tokens":{"input":224,"output":41,"reasoning":0,"cache":{"read":12928,"write":0}}}}"#,
        "\n",
        r#"{"type":"step_start","timestamp":1790912098700,"sessionID":"ses_fixture_turn_0001","part":{"id":"prt_a4","sessionID":"ses_fixture_turn_0001","messageID":"msg_m2","type":"step-start"}}"#,
        "\n",
        r#"{"type":"step_finish","timestamp":1790912098900,"sessionID":"ses_fixture_turn_0001","part":{"id":"prt_a5","sessionID":"ses_fixture_turn_0001","messageID":"msg_m2","type":"step-finish","reason":"stop","cost":0.00026622,"tokens":{"input":426,"output":16,"reasoning":0,"cache":{"read":13440,"write":0}}}}"#,
        "\n",
        r#"{"type":"text","timestamp":1790912099000,"sessionID":"ses_fixture_turn_0001","part":{"id":"prt_a6","sessionID":"ses_fixture_turn_0001","messageID":"msg_m2","type":"text","text":"DONE","time":{"start":1790912098800,"end":1790912099000}}}"#,
    );

    /// The literal shape the 2.0.21 build emitted for a two-step turn: two `step_start`s, ONE `step_finish`.
    const TERMINAL_STEP_UNREPORTED: &str = concat!(
        r#"{"type":"step_start","timestamp":1,"sessionID":"ses_fixture_turn_0002","part":{"type":"step-start"}}"#,
        "\n",
        r#"{"type":"tool_use","timestamp":2,"sessionID":"ses_fixture_turn_0002","part":{"type":"tool","tool":"shell","state":{"status":"completed"}}}"#,
        "\n",
        r#"{"type":"step_finish","timestamp":3,"sessionID":"ses_fixture_turn_0002","part":{"type":"step-finish","reason":"tool-calls","cost":0.000096984,"tokens":{"input":224,"output":41,"reasoning":0,"cache":{"read":12928,"write":0}}}}"#,
        "\n",
        r#"{"type":"step_start","timestamp":4,"sessionID":"ses_fixture_turn_0002","part":{"type":"step-start"}}"#,
        "\n",
        r#"{"type":"text","timestamp":5,"sessionID":"ses_fixture_turn_0002","part":{"type":"text","text":"smoke-ok"}}"#,
    );

    #[test]
    fn a_fresh_turn_reports_the_session_the_vendor_actually_used() {
        let turn = parse_run_events(MULTI_STEP).expect("a real stream parses");
        assert_eq!(turn.session_id.as_deref(), Some("ses_fixture_turn_0001"));
        assert_eq!(
            turn.assistant_text, "DONE",
            "text events are the role output"
        );
        assert_eq!(turn.steps, 2);
        assert_eq!(turn.events.len(), 6, "every event is preserved");
        assert!(turn.error.is_none());
    }

    #[test]
    fn every_step_finish_is_aggregated_not_just_the_last() {
        let turn = parse_run_events(MULTI_STEP).expect("parses");
        let usage = turn.usage.expect("measured");
        assert_eq!(usage.tokens_input, 650, "224 + 426");
        assert_eq!(usage.tokens_output, 57, "41 + 16");
        assert!(
            (usage.cost_usd - 0.000363204).abs() < 1e-12,
            "9.6984e-05 + 2.6622e-04"
        );
        assert_eq!(
            usage.session_id, "ses_fixture_turn_0001",
            "usage is attributed to the session it was measured in"
        );
    }

    #[test]
    fn the_sum_is_a_floor_when_the_terminal_step_reports_no_finish() {
        // The live 2.0.21 behaviour. This test exists to stop anyone reading `usage` as the total: the
        // session's own export for this exact turn read 650/57/$0.0003632, while the stream yields one step.
        let turn = parse_run_events(TERMINAL_STEP_UNREPORTED).expect("parses");
        let usage = turn.usage.expect("the one reported step is measured");
        assert_eq!((usage.tokens_input, usage.tokens_output), (224, 41));
        assert_eq!(
            turn.steps, 1,
            "one step_finish for two steps: the terminal step is unreported, so this sum is a lower bound"
        );
        assert_eq!(turn.assistant_text, "smoke-ok");
    }

    #[test]
    fn text_events_concatenate_in_arrival_order() {
        let stream = concat!(
            r#"{"type":"text","sessionID":"ses_t","part":{"type":"text","text":"<FORGE_DECISION>"}}"#,
            "\n",
            r#"{"type":"text","sessionID":"ses_t","part":{"type":"text","text":"proceed</FORGE_DECISION>"}}"#,
        );
        let turn = parse_run_events(stream).expect("parses");
        assert_eq!(
            turn.assistant_text,
            "<FORGE_DECISION>proceed</FORGE_DECISION>"
        );
    }

    #[test]
    fn an_unknown_event_type_is_tolerated_rather_than_fatal() {
        let stream = concat!(
            r#"{"type":"text","sessionID":"ses_t","part":{"type":"text","text":"ok"}}"#,
            "\n",
            r#"{"type":"some_future_v3_thing","sessionID":"ses_t","payload":{"anything":true}}"#,
            "\n",
            r#"{"type":"text","sessionID":"ses_t","part":{"type":"text","text":"!"}}"#,
        );
        let turn = parse_run_events(stream).expect("an unknown type must not fail the turn");
        assert_eq!(turn.assistant_text, "ok!");
        assert_eq!(turn.events.len(), 3, "the unknown event is preserved");
    }

    #[test]
    fn malformed_json_fails_closed() {
        let err = parse_run_events(r#"{"type":"text","sessionID":"ses_t""#)
            .expect_err("a broken structured contract must not read as success");
        assert!(err.to_string().contains("not valid JSON"), "{err}");
        // A stream that is half good must fail too: the bad line is not skipped.
        let mixed = concat!(
            r#"{"type":"text","sessionID":"ses_t","part":{"type":"text","text":"ok"}}"#,
            "\n",
            "not json at all",
        );
        assert!(parse_run_events(mixed).is_err());
    }

    #[test]
    fn a_provider_shaped_error_is_captured_even_without_a_type_field() {
        let stream = r#"{"name":"UnknownError","message":"Unexpected server error"}"#;
        let turn = parse_run_events(stream).expect("valid JSON parses");
        assert_eq!(
            turn.error.as_deref(),
            Some("UnknownError: Unexpected server error"),
            "the shape V2 returns for an unknown model id must not be silent"
        );
        assert!(
            turn.usage.is_none(),
            "a failed turn with no step_finish is unmeasured"
        );
        assert_eq!(turn.session_id, None);
    }

    #[test]
    fn a_typed_error_event_is_captured() {
        let stream = concat!(
            r#"{"type":"error","sessionID":"ses_t","error":{"name":"ProviderError","message":"provider refused"}}"#,
        );
        let turn = parse_run_events(stream).expect("parses");
        assert_eq!(
            turn.error.as_deref(),
            Some("provider refused"),
            "an error event is surfaced, not swallowed"
        );
    }

    #[test]
    fn a_turn_with_no_step_finish_is_unmeasured_not_zero_cost() {
        let stream = concat!(
            r#"{"type":"step_start","sessionID":"ses_t","part":{"type":"step-start"}}"#,
            "\n",
            r#"{"type":"text","sessionID":"ses_t","part":{"type":"text","text":"hi"}}"#,
        );
        let turn = parse_run_events(stream).expect("parses");
        assert!(turn.usage.is_none(), "missing measurement is not zero cost");
        assert_eq!(turn.steps, 0);
        assert_eq!(turn.assistant_text, "hi");
    }

    #[test]
    fn the_part_level_session_key_is_read_when_the_top_level_one_is_absent() {
        let stream =
            r#"{"type":"text","part":{"sessionID":"ses_only_in_part","type":"text","text":"x"}}"#;
        let turn = parse_run_events(stream).expect("parses");
        assert_eq!(turn.session_id.as_deref(), Some("ses_only_in_part"));
    }
}
